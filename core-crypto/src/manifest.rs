use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

const FORMAT_VERSION: u16 = 1;
const MAX_ENTRIES: usize = 1_000_000;
const MAX_CHUNK_BYTES: u64 = 16 * 1024 * 1024;
const AEAD_TAG_BYTES: u64 = 16;

#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("unsupported manifest format version")]
    UnsupportedVersion,
    #[error("manifest has too many entries")]
    TooManyEntries,
    #[error("manifest entries are not in canonical order or contain duplicates")]
    NonCanonicalEntries,
    #[error("manifest contains invalid chunk sizes")]
    InvalidChunkSize,
    #[error("manifest serialization failed")]
    Serialization,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataClass {
    Contacts,
    Photos,
    Videos,
    SmsMms,
    CallLog,
    Documents,
    AppData,
    Notes,
    Calendars,
    AuthenticatorVault,
    WhatsApp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ObjectId(pub [u8; 32]);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub data_class: DataClass,
    /// Opaque stable item identifier; names and paths belong inside encrypted metadata.
    pub item_id: [u8; 16],
    pub chunk_index: u32,
    pub object_id: ObjectId,
    pub plaintext_size: u64,
    pub ciphertext_size: u64,
    pub source_modified_unix_ms: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub format_version: u16,
    pub snapshot_id: [u8; 16],
    pub device_id: [u8; 16],
    pub created_at_unix_ms: u64,
    /// Entries must be sorted by (data_class, item_id, chunk_index).
    pub entries: Vec<ManifestEntry>,
}

impl Manifest {
    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.format_version != FORMAT_VERSION {
            return Err(ManifestError::UnsupportedVersion);
        }
        if self.entries.len() > MAX_ENTRIES {
            return Err(ManifestError::TooManyEntries);
        }

        let mut seen = BTreeSet::new();
        let mut previous: Option<(DataClass, [u8; 16], u32)> = None;

        for entry in &self.entries {
            let key = (entry.data_class, entry.item_id, entry.chunk_index);
            if previous.is_some_and(|prior| prior >= key) || !seen.insert(key) {
                return Err(ManifestError::NonCanonicalEntries);
            }
            previous = Some(key);

            if entry.plaintext_size > MAX_CHUNK_BYTES
                || entry.plaintext_size.checked_add(AEAD_TAG_BYTES) != Some(entry.ciphertext_size)
            {
                return Err(ManifestError::InvalidChunkSize);
            }
        }
        Ok(())
    }

    /// Postcard encodes this fixed-schema structure deterministically after
    /// validation; no unordered maps or floating-point fields are permitted.
    pub fn encode(&self) -> Result<Vec<u8>, ManifestError> {
        self.validate()?;
        postcard::to_allocvec(self).map_err(|_| ManifestError::Serialization)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, ManifestError> {
        let manifest: Self =
            postcard::from_bytes(bytes).map_err(|_| ManifestError::Serialization)?;
        manifest.validate()?;
        Ok(manifest)
    }
}
