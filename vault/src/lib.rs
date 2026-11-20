//! Local immutable storage primitives for opaque ciphertext.
//! This is not yet an authenticated network service or a production backup vault.

use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("invalid opaque identifier")]
    InvalidId,
    #[error("object exceeds configured size limit")]
    TooLarge,
    #[error("immutable object already exists")]
    AlreadyExists,
    #[error("stored object integrity check failed")]
    Integrity,
    #[error("storage I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("secure random number generation failed")]
    Randomness,
    #[error("upload session was not found")]
    SessionNotFound,
    #[error("upload session metadata conflicts with this request")]
    SessionConflict,
    #[error("upload session is incomplete")]
    Incomplete,
    #[error("upload chunk or completed object digest does not match")]
    DigestMismatch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UploadReceipt {
    object_id: String,
    digest: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UploadSession {
    object_id: String,
    total_size: u64,
    chunk_size: u32,
    expected_digest: [u8; 32],
    created_at_unix: u64,
    chunks: Vec<Option<[u8; 32]>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UploadSessionInfo {
    pub upload_id: String,
    pub object_id: String,
    pub total_size: u64,
    pub chunk_size: u32,
    pub received_bytes: u64,
    pub chunk_count: usize,
}

pub struct VaultStore {
    root: PathBuf,
    max_object_bytes: u64,
}

impl VaultStore {
    pub fn open(root: impl AsRef<Path>, max_object_bytes: u64) -> Result<Self, StoreError> {
        if max_object_bytes == 0 {
            return Err(StoreError::TooLarge);
        }
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(root.join("objects"))?;
        fs::create_dir_all(root.join("manifests"))?;
        fs::create_dir_all(root.join("uploads"))?;
        Ok(Self {
            root,
            max_object_bytes,
        })
    }

    /// Starts a persistent fixed-size-chunk upload session. Chunk size is 64 KiB–1 MiB.
    pub fn create_upload_session(
        &self,
        object_id: &str,
        total_size: u64,
        chunk_size: u32,
        expected_digest: [u8; 32],
    ) -> Result<UploadSessionInfo, StoreError> {
        validate_id(object_id, 64)?;
        if total_size == 0 || total_size > self.max_object_bytes
            || !(64 * 1024..=1024 * 1024).contains(&chunk_size)
        {
            return Err(StoreError::TooLarge);
        }
        let chunk_count = total_size.div_ceil(chunk_size as u64) as usize;
        if chunk_count > 256 {
            return Err(StoreError::TooLarge);
        }
        let mut random = [0u8; 16];
        getrandom::fill(&mut random).map_err(|_| StoreError::Randomness)?;
        let upload_id = hex_encode(&random);
        let session = UploadSession {
            object_id: object_id.to_owned(),
            total_size,
            chunk_size,
            expected_digest,
            created_at_unix: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            chunks: vec![None; chunk_count],
        };
        self.prune_expired_uploads()?;
        let active_sessions = fs::read_dir(self.root.join("uploads"))?
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
            .count();
        if active_sessions >= 32 {
            return Err(StoreError::TooLarge);
        }
        let part_path = self.upload_part_path(&upload_id);
        let mut part = OpenOptions::new().write(true).create_new(true).open(&part_path)?;
        part.set_len(total_size)?;
        part.sync_all()?;
        self.save_upload_session(&upload_id, &session)?;
        Ok(upload_info(&upload_id, &session))
    }

    /// Writes one aligned chunk. Repeating a completed chunk is idempotent only when its digest matches.
    pub fn put_upload_chunk(
        &self,
        upload_id: &str,
        chunk_index: usize,
        bytes: &[u8],
        expected_chunk_digest: [u8; 32],
    ) -> Result<UploadSessionInfo, StoreError> {
        validate_id(upload_id, 32)?;
        let mut session = self.load_upload_session(upload_id)?;
        let slot = session.chunks.get_mut(chunk_index).ok_or(StoreError::SessionConflict)?;
        let offset = chunk_index as u64 * session.chunk_size as u64;
        let expected_len = (session.total_size - offset).min(session.chunk_size as u64) as usize;
        if bytes.len() != expected_len || blake3::hash(bytes).as_bytes() != &expected_chunk_digest {
            return Err(StoreError::DigestMismatch);
        }
        if let Some(existing) = slot {
            if existing != &expected_chunk_digest {
                return Err(StoreError::SessionConflict);
            }
            return Ok(upload_info(upload_id, &session));
        }

        let mut part = OpenOptions::new().write(true).open(self.upload_part_path(upload_id))?;
        part.seek(SeekFrom::Start(offset))?;
        part.write_all(bytes)?;
        part.sync_all()?;
        session.chunks[chunk_index] = Some(expected_chunk_digest);
        self.save_upload_session(upload_id, &session)?;
        Ok(upload_info(upload_id, &session))
    }

    pub fn upload_session_info(&self, upload_id: &str) -> Result<UploadSessionInfo, StoreError> {
        validate_id(upload_id, 32)?;
        let session = self.load_upload_session(upload_id)?;
        Ok(upload_info(upload_id, &session))
    }

    /// Verifies all chunk receipts and the full-object digest before immutable commit.
    /// Repeating a successful completion is safe and returns the same digest.
    pub fn complete_upload(&self, upload_id: &str) -> Result<[u8; 32], StoreError> {
        validate_id(upload_id, 32)?;
        let session_path = self.upload_session_path(upload_id);
        if !session_path.exists() {
            let receipt_path = self.upload_receipt_path(upload_id);
            let receipt_bytes = fs::read(receipt_path).map_err(|error| {
                if error.kind() == std::io::ErrorKind::NotFound {
                    StoreError::SessionNotFound
                } else {
                    StoreError::Io(error)
                }
            })?;
            let receipt: UploadReceipt =
                serde_json::from_slice(&receipt_bytes).map_err(|_| StoreError::Integrity)?;
            self.get_object(&receipt.object_id, receipt.digest)?;
            return Ok(receipt.digest);
        }
        let session = self.load_upload_session(upload_id)?;
        if session.chunks.iter().any(Option::is_none) {
            return Err(StoreError::Incomplete);
        }
        let mut bytes = Vec::with_capacity(session.total_size as usize);
        File::open(self.upload_part_path(upload_id))?
            .take(self.max_object_bytes.saturating_add(1))
            .read_to_end(&mut bytes)?;
        let digest = *blake3::hash(&bytes).as_bytes();
        if bytes.len() as u64 != session.total_size || digest != session.expected_digest {
            return Err(StoreError::DigestMismatch);
        }
        match self.put_object(&session.object_id, &bytes) {
            Ok(_) => {}
            Err(StoreError::AlreadyExists) => {
                self.get_object(&session.object_id, digest)?;
            }
            Err(error) => return Err(error),
        }
        self.save_upload_receipt(
            upload_id,
            &UploadReceipt {
                object_id: session.object_id,
                digest,
            },
        )?;
        let _ = fs::remove_file(self.upload_part_path(upload_id));
        let _ = fs::remove_file(session_path);
        Ok(digest)
    }

    fn upload_session_path(&self, upload_id: &str) -> PathBuf {
        self.root.join("uploads").join(format!("{upload_id}.json"))
    }

    fn upload_part_path(&self, upload_id: &str) -> PathBuf {
        self.root.join("uploads").join(format!("{upload_id}.part"))
    }

    fn upload_receipt_path(&self, upload_id: &str) -> PathBuf {
        self.root.join("uploads").join(format!("{upload_id}.done"))
    }

    fn save_upload_receipt(&self, upload_id: &str, receipt: &UploadReceipt) -> Result<(), StoreError> {
        let mut random = [0u8; 8];
        getrandom::fill(&mut random).map_err(|_| StoreError::Randomness)?;
        let temp_path = self.root.join("uploads").join(format!(".{upload_id}-{}.receipt.tmp", hex_encode(&random)));
        let bytes = serde_json::to_vec(receipt).map_err(|_| StoreError::Integrity)?;
        let mut file = OpenOptions::new().write(true).create_new(true).open(&temp_path)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp_path, self.upload_receipt_path(upload_id))?;
        File::open(self.root.join("uploads"))?.sync_all()?;
        Ok(())
    }

    fn prune_expired_uploads(&self) -> Result<(), StoreError> {
        const RETENTION_SECONDS: u64 = 7 * 24 * 60 * 60;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        for entry in fs::read_dir(self.root.join("uploads"))? {
            let entry = entry?;
            if entry.path().extension().is_some_and(|ext| ext == "json") {
                let session: UploadSession = match fs::read(entry.path())
                    .ok()
                    .and_then(|bytes| serde_json::from_slice(&bytes).ok())
                {
                    Some(session) => session,
                    None => continue,
                };
                if now.saturating_sub(session.created_at_unix) > RETENTION_SECONDS {
                    let stem = entry.path().file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_owned();
                    let _ = fs::remove_file(self.upload_part_path(&stem));
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
        Ok(())
    }

    fn load_upload_session(&self, upload_id: &str) -> Result<UploadSession, StoreError> {
        let bytes = fs::read(self.upload_session_path(upload_id))
            .map_err(|error| if error.kind() == std::io::ErrorKind::NotFound {
                StoreError::SessionNotFound
            } else {
                StoreError::Io(error)
            })?;
        let session: UploadSession =
            serde_json::from_slice(&bytes).map_err(|_| StoreError::Integrity)?;
        let expected_chunks = session.total_size.div_ceil(session.chunk_size.max(1) as u64);
        if session.total_size == 0
            || session.total_size > self.max_object_bytes
            || !(64 * 1024..=1024 * 1024).contains(&session.chunk_size)
            || expected_chunks > 256
            || session.chunks.len() != expected_chunks as usize
            || validate_id(&session.object_id, 64).is_err()
        {
            return Err(StoreError::Integrity);
        }
        Ok(session)
    }

    fn save_upload_session(&self, upload_id: &str, session: &UploadSession) -> Result<(), StoreError> {
        let mut random = [0u8; 8];
        getrandom::fill(&mut random).map_err(|_| StoreError::Randomness)?;
        let temp_path = self.root.join("uploads").join(format!(".{upload_id}-{}.tmp", hex_encode(&random)));
        let bytes = serde_json::to_vec(session).map_err(|_| StoreError::Integrity)?;
        let mut file = OpenOptions::new().write(true).create_new(true).open(&temp_path)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp_path, self.upload_session_path(upload_id))?;
        File::open(self.root.join("uploads"))?.sync_all()?;
        Ok(())
    }

    /// Returns the sum of regular object, manifest, and in-flight part file sizes for quota enforcement.
    pub fn total_bytes(&self) -> Result<u64, StoreError> {
        let mut total = 0u64;
        for area in ["objects", "manifests", "uploads"] {
            for entry in fs::read_dir(self.root.join(area))? {
                let entry = entry?;
                let metadata = entry.metadata()?;
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if metadata.is_file() && !name.starts_with(".") && (area != "uploads" || name.ends_with(".part")) {
                    total = total.saturating_add(metadata.len());
                }
            }
        }
        Ok(total)
    }

    pub fn put_object(&self, id: &str, ciphertext: &[u8]) -> Result<[u8; 32], StoreError> {
        self.put_immutable("objects", id, 64, ciphertext)
    }

    /// Reads and hashes an immutable object so clients can verify a deduplication hit.
    pub fn object_metadata(&self, id: &str) -> Result<(u64, [u8; 32]), StoreError> {
        validate_id(id, 64)?;
        let path = self.root.join("objects").join(id);
        let metadata = fs::metadata(&path)?;
        if metadata.len() > self.max_object_bytes {
            return Err(StoreError::TooLarge);
        }
        let mut bytes = Vec::with_capacity(
            usize::try_from(metadata.len()).map_err(|_| StoreError::TooLarge)?,
        );
        File::open(path)?
            .take(self.max_object_bytes.saturating_add(1))
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 != metadata.len() {
            return Err(StoreError::Integrity);
        }
        Ok((metadata.len(), *blake3::hash(&bytes).as_bytes()))
    }

    pub fn get_object(
        &self,
        id: &str,
        expected_blake3: [u8; 32],
    ) -> Result<Vec<u8>, StoreError> {
        self.get_verified("objects", id, 64, expected_blake3)
    }

    pub fn put_manifest(
        &self,
        snapshot_id: &str,
        encrypted_manifest: &[u8],
    ) -> Result<[u8; 32], StoreError> {
        self.put_immutable("manifests", snapshot_id, 32, encrypted_manifest)
    }

    pub fn get_manifest(
        &self,
        snapshot_id: &str,
        expected_blake3: [u8; 32],
    ) -> Result<Vec<u8>, StoreError> {
        self.get_verified("manifests", snapshot_id, 32, expected_blake3)
    }

    fn put_immutable(
        &self,
        area: &str,
        id: &str,
        id_len: usize,
        bytes: &[u8],
    ) -> Result<[u8; 32], StoreError> {
        validate_id(id, id_len)?;
        if bytes.len() as u64 > self.max_object_bytes {
            return Err(StoreError::TooLarge);
        }

        let dir = self.root.join(area);
        let destination = dir.join(id);
        let mut random = [0u8; 16];
        getrandom::fill(&mut random).map_err(|_| StoreError::Randomness)?;
        let temporary = dir.join(format!(".tmp-{}", hex_encode(&random)));

        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        let write_result = (|| -> Result<(), std::io::Error> {
            file.write_all(bytes)?;
            file.sync_all()?;
            Ok(())
        })();
        drop(file);

        if let Err(error) = write_result {
            let _ = fs::remove_file(&temporary);
            return Err(StoreError::Io(error));
        }

        // hard_link gives create-if-absent semantics; concurrent writers cannot overwrite.
        let link_result = fs::hard_link(&temporary, &destination);
        let _ = fs::remove_file(&temporary);
        match link_result {
            Ok(()) => Ok(*blake3::hash(bytes).as_bytes()),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                Err(StoreError::AlreadyExists)
            }
            Err(error) => Err(StoreError::Io(error)),
        }
    }

    fn get_verified(
        &self,
        area: &str,
        id: &str,
        id_len: usize,
        expected: [u8; 32],
    ) -> Result<Vec<u8>, StoreError> {
        validate_id(id, id_len)?;
        let path = self.root.join(area).join(id);
        let metadata = fs::metadata(&path)?;
        if metadata.len() > self.max_object_bytes {
            return Err(StoreError::TooLarge);
        }

        let file = File::open(path)?;
        let capacity = usize::try_from(metadata.len()).map_err(|_| StoreError::TooLarge)?;
        let mut bytes = Vec::with_capacity(capacity);
        file.take(self.max_object_bytes.saturating_add(1))
            .read_to_end(&mut bytes)?;

        if bytes.len() as u64 > self.max_object_bytes
            || blake3::hash(&bytes).as_bytes() != &expected
        {
            return Err(StoreError::Integrity);
        }
        Ok(bytes)
    }
}

fn upload_info(upload_id: &str, session: &UploadSession) -> UploadSessionInfo {
    let received_bytes = session.chunks.iter().enumerate().fold(0u64, |total, (index, chunk)| {
        if chunk.is_some() {
            total + (session.total_size - index as u64 * session.chunk_size as u64)
                .min(session.chunk_size as u64)
        } else {
            total
        }
    });
    UploadSessionInfo {
        upload_id: upload_id.to_owned(),
        object_id: session.object_id.clone(),
        total_size: session.total_size,
        chunk_size: session.chunk_size,
        received_bytes,
        chunk_count: session.chunks.len(),
    }
}

fn validate_id(id: &str, expected_len: usize) -> Result<(), StoreError> {
    if id.len() != expected_len
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(StoreError::InvalidId);
    }
    Ok(())
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resumable_upload_survives_store_reopen_and_completion_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let store = VaultStore::open(dir.path(), 2 * 1024 * 1024).unwrap();
        let payload = vec![7u8; 100_000];
        let digest = *blake3::hash(&payload).as_bytes();
        let object_id = "ab".repeat(32);
        let session = store.create_upload_session(&object_id, payload.len() as u64, 65_536, digest).unwrap();
        let first = &payload[..65_536];
        let second = &payload[65_536..];
        store.put_upload_chunk(&session.upload_id, 0, first, *blake3::hash(first).as_bytes()).unwrap();
        let reopened = VaultStore::open(dir.path(), 2 * 1024 * 1024).unwrap();
        assert_eq!(reopened.upload_session_info(&session.upload_id).unwrap().received_bytes, 65_536);
        assert!(matches!(reopened.complete_upload(&session.upload_id), Err(StoreError::Incomplete)));
        reopened.put_upload_chunk(&session.upload_id, 1, second, *blake3::hash(second).as_bytes()).unwrap();
        assert_eq!(reopened.complete_upload(&session.upload_id).unwrap(), digest);
        assert_eq!(reopened.complete_upload(&session.upload_id).unwrap(), digest);
        assert_eq!(reopened.get_object(&object_id, digest).unwrap(), payload);
    }

    #[test]
    fn upload_chunk_rejects_wrong_digest_and_conflicting_retry() {
        let dir = tempfile::tempdir().unwrap();
        let store = VaultStore::open(dir.path(), 2 * 1024 * 1024).unwrap();
        let payload = vec![3u8; 65_536];
        let session = store.create_upload_session(&"cd".repeat(32), payload.len() as u64, 65_536, *blake3::hash(&payload).as_bytes()).unwrap();
        assert!(matches!(store.put_upload_chunk(&session.upload_id, 0, &payload, [0u8; 32]), Err(StoreError::DigestMismatch)));
        let digest = *blake3::hash(&payload).as_bytes();
        store.put_upload_chunk(&session.upload_id, 0, &payload, digest).unwrap();
        assert!(matches!(store.put_upload_chunk(&session.upload_id, 0, &vec![4u8; 65_536], *blake3::hash(&vec![4u8; 65_536]).as_bytes()), Err(StoreError::SessionConflict)));
    }

    #[test]
    fn object_metadata_exposes_verified_digest_for_deduplication() {
        let dir = tempfile::tempdir().unwrap();
        let store = VaultStore::open(dir.path(), 1024).unwrap();
        let payload = b"already encrypted";
        let id = "ac".repeat(32);
        let expected = store.put_object(&id, payload).unwrap();
        assert_eq!(store.object_metadata(&id).unwrap(), (payload.len() as u64, expected));
    }

    #[test]
    fn total_bytes_counts_objects_and_manifests() {
        let dir = tempfile::tempdir().unwrap();
        let store = VaultStore::open(dir.path(), 1024).unwrap();
        store.put_object(&"aa".repeat(32), b"12345").unwrap();
        store.put_manifest(&"bb".repeat(16), b"123").unwrap();
        assert_eq!(store.total_bytes().unwrap(), 8);
    }

    #[test]
    fn opaque_object_round_trip_and_integrity_check() {
        let dir = tempfile::tempdir().unwrap();
        let store = VaultStore::open(dir.path(), 1024).unwrap();
        let id = "ab".repeat(32);
        let payload = b"ciphertext only";
        let digest = store.put_object(&id, payload).unwrap();
        assert_eq!(store.get_object(&id, digest).unwrap(), payload);
        assert!(matches!(
            store.get_object(&id, [0u8; 32]),
            Err(StoreError::Integrity)
        ));
    }

    #[test]
    fn encrypted_manifest_uses_128_bit_snapshot_identifier() {
        let dir = tempfile::tempdir().unwrap();
        let store = VaultStore::open(dir.path(), 1024).unwrap();
        let id = "12".repeat(16);
        let bytes = b"encrypted manifest";
        let digest = store.put_manifest(&id, bytes).unwrap();
        assert_eq!(store.get_manifest(&id, digest).unwrap(), bytes);
    }

    #[test]
    fn immutable_objects_cannot_be_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let store = VaultStore::open(dir.path(), 1024).unwrap();
        let id = "cd".repeat(32);
        store.put_object(&id, b"first").unwrap();
        assert!(matches!(
            store.put_object(&id, b"second"),
            Err(StoreError::AlreadyExists)
        ));
    }

    #[test]
    fn identifiers_cannot_escape_storage_root() {
        let dir = tempfile::tempdir().unwrap();
        let store = VaultStore::open(dir.path(), 1024).unwrap();
        assert!(matches!(
            store.put_object("../escape", b"x"),
            Err(StoreError::InvalidId)
        ));
    }

    #[test]
    fn configured_size_limit_is_enforced() {
        let dir = tempfile::tempdir().unwrap();
        let store = VaultStore::open(dir.path(), 3).unwrap();
        assert!(matches!(
            store.put_object(&"ef".repeat(32), b"four"),
            Err(StoreError::TooLarge)
        ));
    }
}
