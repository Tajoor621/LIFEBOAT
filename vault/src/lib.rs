//! Local immutable storage primitives for opaque ciphertext.
//! This is not yet an authenticated network service or a production backup vault.

use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

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
        Ok(Self {
            root,
            max_object_bytes,
        })
    }

    /// Returns the sum of regular object and manifest file sizes for quota enforcement.
    pub fn total_bytes(&self) -> Result<u64, StoreError> {
        let mut total = 0u64;
        for area in ["objects", "manifests"] {
            for entry in fs::read_dir(self.root.join(area))? {
                let entry = entry?;
                let metadata = entry.metadata()?;
                if metadata.is_file() && !entry.file_name().to_string_lossy().starts_with(".tmp-") {
                    total = total.saturating_add(metadata.len());
                }
            }
        }
        Ok(total)
    }

    pub fn put_object(&self, id: &str, ciphertext: &[u8]) -> Result<[u8; 32], StoreError> {
        self.put_immutable("objects", id, 64, ciphertext)
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
