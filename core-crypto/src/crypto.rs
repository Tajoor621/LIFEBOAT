use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::{
    aead::{Aead, Payload},
    KeyInit, XChaCha20Poly1305, XNonce,
};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::ObjectId;

const KEY_LEN: usize = 32;
const NONCE_LEN: usize = 24;
const OBJECT_ID_CONTEXT: &str = "LIFEBOAT v1 keyed object identifier";

#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("secure random number generation failed")]
    Randomness,
    #[error("invalid Argon2id parameters or input")]
    Kdf,
    #[error("invalid encryption key")]
    InvalidKey,
    #[error("authenticated encryption failed")]
    Encryption,
    #[error("authenticated decryption failed")]
    Authentication,
    #[error("invalid Ed25519 public key or signature")]
    Signature,
    #[error("invalid ciphertext envelope version")]
    EnvelopeVersion,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Argon2idConfig {
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
}

impl Default for Argon2idConfig {
    fn default() -> Self {
        // RFC 9106 lower-memory recommended profile; benchmark on target devices.
        Self {
            memory_kib: 65_536,
            iterations: 3,
            parallelism: 4,
        }
    }
}

impl Argon2idConfig {
    fn validate(self) -> Result<Params, CryptoError> {
        if self.memory_kib > 262_144
            || self.iterations == 0
            || self.iterations > 10
            || self.parallelism == 0
            || self.parallelism > 8
            || self.memory_kib < self.parallelism.saturating_mul(8)
        {
            return Err(CryptoError::Kdf);
        }
        Params::new(
            self.memory_kib,
            self.iterations,
            self.parallelism,
            Some(KEY_LEN),
        )
        .map_err(|_| CryptoError::Kdf)
    }
}

/// Secret key material is zeroized on drop where supported.
pub struct SymmetricKey(Zeroizing<[u8; KEY_LEN]>);

impl SymmetricKey {
    pub fn from_bytes(bytes: [u8; KEY_LEN]) -> Self {
        Self(Zeroizing::new(bytes))
    }

    pub(crate) fn bytes(&self) -> &[u8; KEY_LEN] {
        &self.0
    }
}

pub struct DedupKey(Zeroizing<[u8; KEY_LEN]>);

impl DedupKey {
    /// Keyed IDs avoid raw plaintext hashes and cross-vault correlation.
    /// Equality of repeated content remains visible within one vault.
    pub fn identify(&self, plaintext: &[u8]) -> ObjectId {
        ObjectId(*blake3::keyed_hash(&self.0, plaintext).as_bytes())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CiphertextEnvelope {
    pub version: u8,
    pub nonce: [u8; NONCE_LEN],
    pub ciphertext: Vec<u8>,
}

pub fn generate_symmetric_key() -> Result<SymmetricKey, CryptoError> {
    let mut key = [0u8; KEY_LEN];
    getrandom::fill(&mut key).map_err(|_| CryptoError::Randomness)?;
    Ok(SymmetricKey::from_bytes(key))
}

pub fn derive_backup_key(
    passphrase: &[u8],
    salt: &[u8],
    config: Argon2idConfig,
) -> Result<SymmetricKey, CryptoError> {
    if salt.len() < 16 || salt.len() > 64 || passphrase.is_empty() {
        return Err(CryptoError::Kdf);
    }
    let params = config.validate()?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut output = Zeroizing::new([0u8; KEY_LEN]);
    argon2
        .hash_password_into(passphrase, salt, &mut *output)
        .map_err(|_| CryptoError::Kdf)?;
    Ok(SymmetricKey(output))
}

pub fn derive_object_id_key(master: &SymmetricKey) -> DedupKey {
    DedupKey(Zeroizing::new(blake3::derive_key(
        OBJECT_ID_CONTEXT,
        master.bytes(),
    )))
}

pub fn hash_bytes(bytes: &[u8]) -> [u8; 32] {
    *blake3::hash(bytes).as_bytes()
}

pub fn seal(
    key: &SymmetricKey,
    plaintext: &[u8],
    associated_data: &[u8],
) -> Result<CiphertextEnvelope, CryptoError> {
    let mut nonce = [0u8; NONCE_LEN];
    getrandom::fill(&mut nonce).map_err(|_| CryptoError::Randomness)?;
    let cipher =
        XChaCha20Poly1305::new_from_slice(key.bytes()).map_err(|_| CryptoError::InvalidKey)?;
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: plaintext,
                aad: associated_data,
            },
        )
        .map_err(|_| CryptoError::Encryption)?;
    Ok(CiphertextEnvelope {
        version: 1,
        nonce,
        ciphertext,
    })
}

pub fn open(
    key: &SymmetricKey,
    envelope: &CiphertextEnvelope,
    associated_data: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    if envelope.version != 1 {
        return Err(CryptoError::EnvelopeVersion);
    }
    let cipher =
        XChaCha20Poly1305::new_from_slice(key.bytes()).map_err(|_| CryptoError::InvalidKey)?;
    cipher
        .decrypt(
            XNonce::from_slice(&envelope.nonce),
            Payload {
                msg: envelope.ciphertext.as_ref(),
                aad: associated_data,
            },
        )
        .map_err(|_| CryptoError::Authentication)
}

pub struct IdentityKeypair(SigningKey);

impl IdentityKeypair {
    pub fn generate() -> Result<Self, CryptoError> {
        let mut seed = Zeroizing::new([0u8; KEY_LEN]);
        getrandom::fill(&mut *seed).map_err(|_| CryptoError::Randomness)?;
        Ok(Self(SigningKey::from_bytes(&seed)))
    }

    pub fn public_key(&self) -> [u8; KEY_LEN] {
        self.0.verifying_key().to_bytes()
    }
}

pub fn sign(identity: &IdentityKeypair, message: &[u8]) -> [u8; 64] {
    identity.0.sign(message).to_bytes()
}

pub fn verify(
    public_key: &[u8; KEY_LEN],
    message: &[u8],
    signature: &[u8; 64],
) -> Result<(), CryptoError> {
    let key = VerifyingKey::from_bytes(public_key).map_err(|_| CryptoError::Signature)?;
    key.verify_strict(message, &Signature::from_bytes(signature))
        .map_err(|_| CryptoError::Signature)
}
