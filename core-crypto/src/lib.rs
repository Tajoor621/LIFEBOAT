//! LIFEBOAT client-side cryptographic building blocks.
//!
//! This crate is an implementation foundation, not a completed security review.
//! Callers must protect passphrases, keys, and plaintext at their boundaries.

mod crypto;
mod manifest;
mod restore;

pub use crypto::{
    create_key_wrapper, derive_backup_key, derive_object_id_key, generate_symmetric_key,
    hash_bytes, open, seal, sign, unlock_with_passphrase, unlock_with_recovery_code, verify,
    Argon2idConfig, CiphertextEnvelope, CreatedKeyWrapper, CryptoError, DedupKey,
    IdentityKeypair, KeyWrapper, RecoveryKit, SymmetricKey,
};
pub use manifest::{DataClass, Manifest, ManifestEntry, ManifestError, ObjectId};
pub use restore::{run_restore_rehearsal, RestoreError, RestoreRehearsalReport};

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn encrypt_decrypt_round_trip_and_aad_binding() {
        let key = generate_symmetric_key().unwrap();
        let envelope = seal(&key, b"private payload", b"lifeboat/test/v1").unwrap();
        assert_eq!(
            open(&key, &envelope, b"lifeboat/test/v1").unwrap(),
            b"private payload"
        );
        assert!(open(&key, &envelope, b"wrong-context").is_err());
    }

    #[test]
    fn ciphertext_tampering_is_rejected() {
        let key = generate_symmetric_key().unwrap();
        let mut envelope = seal(&key, b"payload", b"aad").unwrap();
        envelope.ciphertext[0] ^= 1;
        assert!(open(&key, &envelope, b"aad").is_err());
    }

    #[test]
    fn password_derived_keys_are_salt_bound() {
        let config = Argon2idConfig {
            memory_kib: 8192,
            iterations: 1,
            parallelism: 1,
        };
        let a = derive_backup_key(b"correct horse battery staple", &[7u8; 16], config).unwrap();
        let b = derive_backup_key(b"correct horse battery staple", &[8u8; 16], config).unwrap();
        assert_ne!(a.bytes(), b.bytes());
    }

    #[test]
    fn invalid_kdf_parameters_are_rejected() {
        let config = Argon2idConfig {
            memory_kib: 1024,
            iterations: 1,
            parallelism: 1,
        };
        assert!(derive_backup_key(b"passphrase", &[1u8; 16], config).is_err());
        assert!(
            derive_backup_key(
                b"passphrase",
                &[1u8; 16],
                Argon2idConfig {
                    memory_kib: 262_144,
                    iterations: 10,
                    parallelism: 8,
                },
            )
            .is_err()
        );
        assert!(
            derive_backup_key(
                &vec![b'x'; 4097],
                &[1u8; 16],
                Argon2idConfig::default(),
            )
            .is_err()
        );
    }

    #[test]
    fn object_ids_are_keyed_and_deterministic() {
        let master = generate_symmetric_key().unwrap();
        let key = derive_object_id_key(&master);
        assert_eq!(key.identify(b"chunk"), key.identify(b"chunk"));
        let other = generate_symmetric_key().unwrap();
        assert_ne!(
            key.identify(b"chunk"),
            derive_object_id_key(&other).identify(b"chunk")
        );
    }

    #[test]
    fn identity_signatures_are_verified_strictly() {
        let identity = IdentityKeypair::generate().unwrap();
        let public = identity.public_key();
        let signature = sign(&identity, b"manifest bytes");
        assert!(verify(&public, b"manifest bytes", &signature).is_ok());
        assert!(verify(&public, b"altered bytes", &signature).is_err());
    }

    #[test]
    fn key_wrapper_supports_passphrase_and_offline_recovery_after_serialization() {
        let master = generate_symmetric_key().unwrap();
        let config = Argon2idConfig {
            memory_kib: 8192,
            iterations: 1,
            parallelism: 1,
        };
        let created = create_key_wrapper(
            &master,
            [0x42; 16],
            b"correct horse battery staple",
            config,
        )
        .unwrap();

        let encoded = created.wrapper.encode().unwrap();
        assert!(!String::from_utf8_lossy(&encoded).contains(created.recovery_kit.recovery_code()));
        let decoded = KeyWrapper::decode(&encoded).unwrap();

        let passphrase_unlocked =
            unlock_with_passphrase(&decoded, b"correct horse battery staple").unwrap();
        let recovery_unlocked = unlock_with_recovery_code(
            &decoded,
            created.recovery_kit.recovery_code(),
        )
        .unwrap();
        assert_eq!(master.bytes(), passphrase_unlocked.bytes());
        assert_eq!(master.bytes(), recovery_unlocked.bytes());

        let mut damaged_kdf = decoded.clone();
        damaged_kdf.kdf.memory_kib = 1;
        assert!(unlock_with_passphrase(&damaged_kdf, b"correct horse battery staple").is_err());
        let recovery_after_kdf_damage = unlock_with_recovery_code(
            &damaged_kdf,
            created.recovery_kit.recovery_code(),
        )
        .unwrap();
        assert_eq!(master.bytes(), recovery_after_kdf_damage.bytes());

        let mut wrong_vault = decoded.clone();
        wrong_vault.vault_id[0] ^= 1;
        assert!(unlock_with_passphrase(&wrong_vault, b"correct horse battery staple").is_err());
        assert!(unlock_with_recovery_code(
            &wrong_vault,
            created.recovery_kit.recovery_code()
        )
        .is_err());

        assert!(created.recovery_kit.printable_text().contains("Recovery code:"));
        assert!(created.recovery_kit.printable_text().contains('\n'));
    }

    #[test]
    fn recovery_code_checksum_and_wrong_passphrase_are_rejected() {
        let master = generate_symmetric_key().unwrap();
        let created = create_key_wrapper(
            &master,
            [0x24; 16],
            b"owner passphrase",
            Argon2idConfig {
                memory_kib: 8192,
                iterations: 1,
                parallelism: 1,
            },
        )
        .unwrap();

        assert!(unlock_with_passphrase(&created.wrapper, b"wrong passphrase").is_err());
        let mut damaged = created.recovery_kit.recovery_code().to_owned();
        let replacement = if damaged.ends_with('0') { '1' } else { '0' };
        damaged.pop();
        damaged.push(replacement);
        assert!(unlock_with_recovery_code(&created.wrapper, &damaged).is_err());
    }

    #[test]
    fn key_wrapper_rejects_truncated_and_oversized_encodings() {
        let master = generate_symmetric_key().unwrap();
        let created = create_key_wrapper(
            &master,
            [0x11; 16],
            b"passphrase",
            Argon2idConfig {
                memory_kib: 8192,
                iterations: 1,
                parallelism: 1,
            },
        )
        .unwrap();
        let encoded = created.wrapper.encode().unwrap();
        assert!(KeyWrapper::decode(&encoded[..encoded.len() - 1]).is_err());
        assert!(KeyWrapper::decode(&vec![0u8; 4097]).is_err());
    }

    #[test]
    fn restore_rehearsal_restores_all_contacts_and_validates_sampled_items() {
        use std::collections::HashMap;

        let master = generate_symmetric_key().unwrap();
        let dedup = derive_object_id_key(&master);
        let contacts = b"BEGIN:VCARD\nVERSION:4.0\nFN:Test Owner\nEND:VCARD\n".to_vec();
        let photo = b"sample image payload".to_vec();
        let contacts_id = dedup.identify(&contacts);
        let photo_id = dedup.identify(&photo);
        let mut chunks = HashMap::new();
        chunks.insert(contacts_id, contacts.clone());
        chunks.insert(photo_id, photo.clone());

        let manifest = Manifest {
            format_version: 1,
            snapshot_id: [1; 16],
            device_id: [2; 16],
            created_at_unix_ms: 1234,
            entries: vec![
                ManifestEntry {
                    data_class: DataClass::Contacts,
                    item_id: [3; 16],
                    chunk_index: 0,
                    object_id: contacts_id,
                    plaintext_size: contacts.len() as u64,
                    ciphertext_size: contacts.len() as u64 + 16,
                    source_modified_unix_ms: None,
                },
                ManifestEntry {
                    data_class: DataClass::Photos,
                    item_id: [4; 16],
                    chunk_index: 0,
                    object_id: photo_id,
                    plaintext_size: photo.len() as u64,
                    ciphertext_size: photo.len() as u64 + 16,
                    source_modified_unix_ms: None,
                },
            ],
        };

        let scratch = tempfile::tempdir().unwrap();
        let report = run_restore_rehearsal(
            &manifest,
            &dedup,
            scratch.path(),
            1,
            |id| {
                chunks
                    .get(&id)
                    .cloned()
                    .ok_or(RestoreError::Source("missing test object".into()))
            },
            |class, path| {
                let bytes = fs::read(path).map_err(|error| error.to_string())?;
                match class {
                    DataClass::Contacts
                        if bytes.starts_with(b"BEGIN:VCARD\n")
                            && bytes.ends_with(b"END:VCARD\n") =>
                    {
                        Ok(())
                    }
                    DataClass::Photos if bytes == photo => Ok(()),
                    _ => Err("test parser rejected restored item".into()),
                }
            },
        )
        .unwrap();

        assert_eq!(report.verified_items, 2);
        assert_eq!(report.verified_chunks, 2);
        assert!(report.contacts_present);
        assert!(report.all_contacts_verified);
        assert_eq!(report.sampled_non_contact_items, 1);
        assert_eq!(report.restored_files.len(), 2);
    }

    #[test]
    fn restore_rehearsal_assembles_multi_chunk_contact_in_manifest_order() {
        let master = generate_symmetric_key().unwrap();
        let dedup = derive_object_id_key(&master);
        let first = b"BEGIN:VCARD\nVERSION:4.0\n".to_vec();
        let second = b"FN:Multi Chunk\nEND:VCARD\n".to_vec();
        let expected = [first.clone(), second.clone()].concat();
        let first_id = dedup.identify(&first);
        let second_id = dedup.identify(&second);
        let chunks = HashMap::from([(first_id, first), (second_id, second)]);
        let manifest = Manifest {
            format_version: 1,
            snapshot_id: [7; 16],
            device_id: [8; 16],
            created_at_unix_ms: 5678,
            entries: vec![
                ManifestEntry {
                    data_class: DataClass::Contacts,
                    item_id: [9; 16],
                    chunk_index: 0,
                    object_id: first_id,
                    plaintext_size: 25,
                    ciphertext_size: 41,
                    source_modified_unix_ms: None,
                },
                ManifestEntry {
                    data_class: DataClass::Contacts,
                    item_id: [9; 16],
                    chunk_index: 1,
                    object_id: second_id,
                    plaintext_size: 26,
                    ciphertext_size: 42,
                    source_modified_unix_ms: None,
                },
            ],
        };
        let scratch = tempfile::tempdir().unwrap();
        let report = run_restore_rehearsal(
            &manifest,
            &dedup,
            scratch.path(),
            0,
            |id| chunks.get(&id).cloned().ok_or(RestoreError::Source("missing chunk".into())),
            |class, path| {
                let bytes = fs::read(path).map_err(|error| error.to_string())?;
                if class == DataClass::Contacts && bytes == expected {
                    Ok(())
                } else {
                    Err("multi-chunk VCF did not reassemble exactly".into())
                }
            },
        )
        .unwrap();
        assert_eq!(report.verified_chunks, 2);
        assert_eq!(report.verified_items, 1);
        assert!(report.all_contacts_verified);
    }

    #[test]
    fn restore_rehearsal_rejects_corrupt_plaintext_and_cleans_scratch_files() {
        let master = generate_symmetric_key().unwrap();
        let dedup = derive_object_id_key(&master);
        let payload = b"expected contact".to_vec();
        let id = dedup.identify(&payload);
        let manifest = Manifest {
            format_version: 1,
            snapshot_id: [1; 16],
            device_id: [2; 16],
            created_at_unix_ms: 1234,
            entries: vec![ManifestEntry {
                data_class: DataClass::Contacts,
                item_id: [3; 16],
                chunk_index: 0,
                object_id: id,
                plaintext_size: payload.len() as u64,
                ciphertext_size: payload.len() as u64 + 16,
                source_modified_unix_ms: None,
            }],
        };
        let scratch = tempfile::tempdir().unwrap();
        let result = run_restore_rehearsal(
            &manifest,
            &dedup,
            scratch.path(),
            0,
            |_| Ok(b"tampered".to_vec()),
            |_, _| Ok(()),
        );
        assert!(matches!(result, Err(RestoreError::ChunkMismatch)));
        assert_eq!(fs::read_dir(scratch.path()).unwrap().count(), 0);
    }

    #[test]
    fn restore_rehearsal_rejects_parser_failure_and_removes_scratch_file() {
        let master = generate_symmetric_key().unwrap();
        let dedup = derive_object_id_key(&master);
        let payload = b"valid hash but invalid contact format".to_vec();
        let id = dedup.identify(&payload);
        let manifest = Manifest {
            format_version: 1,
            snapshot_id: [1; 16],
            device_id: [2; 16],
            created_at_unix_ms: 1234,
            entries: vec![ManifestEntry {
                data_class: DataClass::Contacts,
                item_id: [3; 16],
                chunk_index: 0,
                object_id: id,
                plaintext_size: payload.len() as u64,
                ciphertext_size: payload.len() as u64 + 16,
                source_modified_unix_ms: None,
            }],
        };
        let scratch = tempfile::tempdir().unwrap();
        let result = run_restore_rehearsal(
            &manifest,
            &dedup,
            scratch.path(),
            0,
            |_| Ok(payload.clone()),
            |_, _| Err("VCF parser rejected item".into()),
        );
        assert!(matches!(result, Err(RestoreError::InvalidItem(_))));
        assert_eq!(fs::read_dir(scratch.path()).unwrap().count(), 0);
    }

    #[test]
    fn manifest_encoding_is_deterministic_and_validated() {
        let manifest = Manifest {
            format_version: 1,
            snapshot_id: [1; 16],
            device_id: [2; 16],
            created_at_unix_ms: 1234,
            entries: vec![ManifestEntry {
                data_class: DataClass::Photos,
                item_id: [3; 16],
                chunk_index: 0,
                object_id: ObjectId([4; 32]),
                plaintext_size: 5,
                ciphertext_size: 21,
                source_modified_unix_ms: None,
            }],
        };
        let bytes = manifest.encode().unwrap();
        assert_eq!(bytes, manifest.encode().unwrap());
        assert_eq!(Manifest::decode(&bytes).unwrap(), manifest);
    }

    #[test]
    fn manifest_rejects_noncanonical_duplicate_chunks() {
        let entry = ManifestEntry {
            data_class: DataClass::Photos,
            item_id: [3; 16],
            chunk_index: 0,
            object_id: ObjectId([4; 32]),
            plaintext_size: 5,
            ciphertext_size: 21,
            source_modified_unix_ms: None,
        };
        let manifest = Manifest {
            format_version: 1,
            snapshot_id: [1; 16],
            device_id: [2; 16],
            created_at_unix_ms: 1234,
            entries: vec![entry.clone(), entry],
        };
        assert!(manifest.encode().is_err());
    }
}
