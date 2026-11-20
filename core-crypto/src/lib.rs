//! LIFEBOAT client-side cryptographic building blocks.
//!
//! This crate is an implementation foundation, not a completed security review.
//! Callers must protect passphrases, keys, and plaintext at their boundaries.

mod crypto;
mod manifest;

pub use crypto::{
    create_key_wrapper, derive_backup_key, derive_object_id_key, generate_symmetric_key,
    hash_bytes, open, seal, sign, unlock_with_passphrase, unlock_with_recovery_code, verify,
    Argon2idConfig, CiphertextEnvelope, CreatedKeyWrapper, CryptoError, DedupKey,
    IdentityKeypair, KeyWrapper, RecoveryKit, SymmetricKey,
};
pub use manifest::{DataClass, Manifest, ManifestEntry, ManifestError, ObjectId};

#[cfg(test)]
mod tests {
    use super::*;

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
        damaged.pop();
        damaged.push('0');
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
