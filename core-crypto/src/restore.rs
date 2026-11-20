use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use crate::{DataClass, DedupKey, Manifest, ObjectId};

const MAX_REHEARSAL_ITEMS: usize = 100_000;
const MAX_REHEARSAL_ITEM_BYTES: usize = 512 * 1024 * 1024;
const MAX_REHEARSAL_TOTAL_BYTES: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum RestoreError {
    #[error("snapshot manifest is invalid")]
    InvalidManifest,
    #[error("selected restore set is empty")]
    EmptySelection,
    #[error("restore rehearsal exceeds configured resource limits")]
    ResourceLimit,
    #[error("restored chunk does not match the manifest")]
    ChunkMismatch,
    #[error("restore source failed: {0}")]
    Source(String),
    #[error("restored item failed its format/parser validation: {0}")]
    InvalidItem(String),
    #[error("restore rehearsal filesystem operation failed: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreRehearsalReport {
    pub verified_items: usize,
    pub verified_chunks: usize,
    pub verified_bytes: u64,
    pub contacts_present: bool,
    pub all_contacts_verified: bool,
    pub sampled_non_contact_items: usize,
    pub restored_files: Vec<PathBuf>,
}

fn class_name(class: DataClass) -> &'static str {
    match class {
        DataClass::Contacts => "contacts",
        DataClass::Photos => "photos",
        DataClass::Videos => "videos",
        DataClass::SmsMms => "sms_mms",
        DataClass::CallLog => "call_log",
        DataClass::Documents => "documents",
        DataClass::AppData => "app_data",
        DataClass::Notes => "notes",
        DataClass::Calendars => "calendars",
        DataClass::AuthenticatorVault => "authenticator_vault",
        DataClass::WhatsApp => "whatsapp",
    }
}

fn hex_item_id(id: &[u8; 16]) -> String {
    let mut output = String::with_capacity(32);
    for byte in id {
        use std::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

/// Performs a client-side restore rehearsal into a caller-provided scratch directory.
/// Every contact item is included; the requested number of other logical items is
/// selected deterministically. The resolver must decrypt each chunk and the validator
/// must parse the restored item (for example, VCF for contacts). A successful report
/// means selected items were decrypted, checked, written, synced, and parser-validated.
pub fn run_restore_rehearsal<R, V>(
    manifest: &Manifest,
    dedup_key: &DedupKey,
    scratch_dir: impl AsRef<Path>,
    sample_non_contact_items: usize,
    mut resolve_chunk: R,
    mut validate_item: V,
) -> Result<RestoreRehearsalReport, RestoreError>
where
    R: FnMut(ObjectId) -> Result<Vec<u8>, RestoreError>,
    V: FnMut(DataClass, &Path) -> Result<(), String>,
{
    manifest
        .validate()
        .map_err(|_| RestoreError::InvalidManifest)?;

    let mut grouped: BTreeMap<(DataClass, [u8; 16]), Vec<_>> = BTreeMap::new();
    for entry in &manifest.entries {
        grouped
            .entry((entry.data_class, entry.item_id))
            .or_default()
            .push(entry);
    }

    let contacts: Vec<_> = grouped
        .iter()
        .filter(|((class, _), _)| *class == DataClass::Contacts)
        .collect();
    let others: Vec<_> = grouped
        .iter()
        .filter(|((class, _), _)| *class != DataClass::Contacts)
        .take(sample_non_contact_items)
        .collect();

    if contacts.is_empty() && others.is_empty() {
        return Err(RestoreError::EmptySelection);
    }
    if contacts.len().saturating_add(others.len()) > MAX_REHEARSAL_ITEMS {
        return Err(RestoreError::ResourceLimit);
    }

    let mut selected = contacts;
    selected.extend(others);
    let contacts_present = selected
        .iter()
        .any(|((class, _), _)| *class == DataClass::Contacts);
    let sampled_non_contact_items = selected
        .iter()
        .filter(|((class, _), _)| *class != DataClass::Contacts)
        .count();

    let scratch_dir = scratch_dir.as_ref();
    fs::create_dir_all(scratch_dir)?;
    let mut report = RestoreRehearsalReport {
        verified_items: 0,
        verified_chunks: 0,
        verified_bytes: 0,
        contacts_present,
        all_contacts_verified: contacts_present,
        sampled_non_contact_items,
        restored_files: Vec::new(),
    };

    let result = (|| {
        for ((class, item_id), entries) in selected {
            let mut restored = Vec::new();
            for (expected_index, entry) in entries.iter().enumerate() {
                if entry.chunk_index as usize != expected_index {
                    return Err(RestoreError::ChunkMismatch);
                }
                let plaintext = resolve_chunk(entry.object_id)?;
                if plaintext.len() as u64 != entry.plaintext_size
                    || dedup_key.identify(&plaintext) != entry.object_id
                {
                    return Err(RestoreError::ChunkMismatch);
                }
                if restored.len().saturating_add(plaintext.len()) > MAX_REHEARSAL_ITEM_BYTES {
                    return Err(RestoreError::ResourceLimit);
                }
                if report.verified_bytes.saturating_add(plaintext.len() as u64)
                    > MAX_REHEARSAL_TOTAL_BYTES
                {
                    return Err(RestoreError::ResourceLimit);
                }
                restored.extend_from_slice(&plaintext);
                report.verified_chunks += 1;
                report.verified_bytes = report
                    .verified_bytes
                    .checked_add(plaintext.len() as u64)
                    .ok_or(RestoreError::ChunkMismatch)?;
            }

            let file_name = format!(
                "{}_{}.restore",
                class_name(*class),
                hex_item_id(item_id)
            );
            let path = scratch_dir.join(file_name);
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)?;
            report.restored_files.push(path.clone());
            file.write_all(&restored)?;
            file.sync_all()?;
            drop(file);

            if let Err(error) = validate_item(*class, &path) {
                let _ = fs::remove_file(&path);
                return Err(RestoreError::InvalidItem(error));
            }
            report.verified_items += 1;
        }
        Ok(())
    })();

    if let Err(error) = result {
        for path in &report.restored_files {
            let _ = fs::remove_file(path);
        }
        return Err(error);
    }

    Ok(report)
}
