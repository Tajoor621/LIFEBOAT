//! LIFEBOAT client-side cryptographic building blocks.
//!
//! This crate is an implementation foundation, not a completed security review.
//! Callers must protect passphrases, keys, and plaintext at their boundaries.

mod crypto;
mod manifest;

pub use crypto::{
    derive_backup_key, derive_object_id_key, generate_symmetric_key, hash_bytes, open, seal,
    sign, verify, Argon2idConfig, CiphertextEnvelope, CryptoError, DedupKey, IdentityKeypair,
    SymmetricKey,
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
