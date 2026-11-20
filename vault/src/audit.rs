use std::{
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum AuditError {
    #[error("audit storage I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("audit ledger integrity verification failed")]
    Integrity,
    #[error("audit record serialization failed")]
    Serialization,
}

#[derive(Debug, Serialize)]
struct UnsignedRecord<'a> {
    sequence: u64,
    timestamp_unix: u64,
    device_id: &'a str,
    action: &'a str,
    resource_id: Option<&'a str>,
    outcome: &'a str,
    previous_hash: &'a str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Record {
    sequence: u64,
    timestamp_unix: u64,
    device_id: String,
    action: String,
    resource_id: Option<String>,
    outcome: String,
    previous_hash: String,
    hash: String,
}

pub struct AuditLedger {
    path: PathBuf,
    sequence: u64,
    previous_hash: String,
}

impl AuditLedger {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, AuditError> {
        fs::create_dir_all(root.as_ref())?;
        let path = root.as_ref().join("events.jsonl");
        if !path.exists() {
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)?
                .sync_all()?;
            File::open(root.as_ref())?.sync_all()?;
        }
        let (sequence, previous_hash) = verify_file(&path)?;
        Ok(Self {
            path,
            sequence,
            previous_hash,
        })
    }

    pub fn append(
        &mut self,
        device_id: &str,
        action: &str,
        resource_id: Option<&str>,
        outcome: &str,
    ) -> Result<(), AuditError> {
        let timestamp_unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let sequence = self.sequence.checked_add(1).ok_or(AuditError::Integrity)?;
        let unsigned = UnsignedRecord {
            sequence,
            timestamp_unix,
            device_id,
            action,
            resource_id,
            outcome,
            previous_hash: &self.previous_hash,
        };
        let encoded = serde_json::to_vec(&unsigned).map_err(|_| AuditError::Serialization)?;
        let hash = blake3::hash(&encoded).to_hex().to_string();
        let record = Record {
            sequence,
            timestamp_unix,
            device_id: device_id.to_owned(),
            action: action.to_owned(),
            resource_id: resource_id.map(str::to_owned),
            outcome: outcome.to_owned(),
            previous_hash: self.previous_hash.clone(),
            hash: hash.clone(),
        };
        let mut bytes = serde_json::to_vec(&record).map_err(|_| AuditError::Serialization)?;
        bytes.push(b'\n');
        let mut file = OpenOptions::new().append(true).open(&self.path)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        self.sequence = sequence;
        self.previous_hash = hash;
        Ok(())
    }

    #[cfg(test)]
    fn path(&self) -> &Path {
        &self.path
    }
}

fn verify_file(path: &Path) -> Result<(u64, String), AuditError> {
    let file = File::open(path)?;
    let mut sequence = 0u64;
    let mut previous_hash = "0".repeat(64);
    for line in BufReader::new(file).lines() {
        let line = line?;
        if line.is_empty() {
            return Err(AuditError::Integrity);
        }
        let record: Record = serde_json::from_str(&line).map_err(|_| AuditError::Integrity)?;
        let expected_sequence = sequence.checked_add(1).ok_or(AuditError::Integrity)?;
        if record.sequence != expected_sequence || record.previous_hash != previous_hash {
            return Err(AuditError::Integrity);
        }
        let unsigned = UnsignedRecord {
            sequence: record.sequence,
            timestamp_unix: record.timestamp_unix,
            device_id: &record.device_id,
            action: &record.action,
            resource_id: record.resource_id.as_deref(),
            outcome: &record.outcome,
            previous_hash: &record.previous_hash,
        };
        let encoded = serde_json::to_vec(&unsigned).map_err(|_| AuditError::Serialization)?;
        let calculated = blake3::hash(&encoded).to_hex().to_string();
        if calculated != record.hash {
            return Err(AuditError::Integrity);
        }
        sequence = record.sequence;
        previous_hash = record.hash;
    }
    Ok((sequence, previous_hash))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ledger_survives_restart_and_verifies_chain() {
        let dir = tempfile::tempdir().unwrap();
        {
            let mut ledger = AuditLedger::open(dir.path()).unwrap();
            ledger
                .append("device", "object_put", Some("object"), "success")
                .unwrap();
            ledger
                .append("device", "manifest_put", Some("snapshot"), "success")
                .unwrap();
        }
        let reopened = AuditLedger::open(dir.path()).unwrap();
        assert_eq!(reopened.sequence, 2);
    }

    #[test]
    fn ledger_fails_closed_after_tampering() {
        let dir = tempfile::tempdir().unwrap();
        let path;
        {
            let mut ledger = AuditLedger::open(dir.path()).unwrap();
            ledger
                .append("device", "object_put", None, "success")
                .unwrap();
            path = ledger.path().to_path_buf();
        }
        let original = fs::read_to_string(&path).unwrap();
        fs::write(&path, original.replace("object_put", "object_get")).unwrap();
        assert!(matches!(
            AuditLedger::open(dir.path()),
            Err(AuditError::Integrity)
        ));
    }
}
