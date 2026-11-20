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
    #[error("invalid or unsupported key-wrapper format")]
    WrapperFormat,
    #[error("recovery code checksum or encoding is invalid")]
    RecoveryCode,
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


const WRAPPER_VERSION: u8 = 1;
const MAX_WRAPPER_BYTES: usize = 4096;
const RECOVERY_PREFIX: &str = "LB1-";

/// Client-side key wrapper. It contains only encrypted copies of the vault master key.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyWrapper {
    pub version: u8,
    pub vault_id: [u8; 16],
    pub kdf: Argon2idConfig,
    pub passphrase_salt: [u8; 16],
    pub passphrase_envelope: CiphertextEnvelope,
    pub recovery_envelope: CiphertextEnvelope,
}

/// The recovery code is deliberately not Debug/Serialize. Display it once, let the
/// owner print or store it offline, and never send it to the vault server.
pub struct RecoveryKit {
    recovery_code: String,
    vault_id: [u8; 16],
}

impl RecoveryKit {
    pub fn recovery_code(&self) -> &str {
        &self.recovery_code
    }

    pub fn vault_id(&self) -> [u8; 16] {
        self.vault_id
    }

    /// Plain-text print layout. Callers should show this only in an explicit,
    /// owner-controlled export flow and warn that anyone holding it can unlock the vault.
    pub fn printable_text(&self) -> String {
        format!(
            "LIFEBOAT RECOVERY KIT\\nVault ID: {}\\nRecovery code: {}\\n\\nStore offline in a secure place. Anyone with this code can unlock this vault.\\nDo not photograph, email, or upload this page. Test recovery before relying on it.",
            encode_hex(&self.vault_id),
            self.recovery_code
        )
    }
}

pub struct CreatedKeyWrapper {
    pub wrapper: KeyWrapper,
    pub recovery_kit: RecoveryKit,
}

fn wrapper_aad(vault_id: &[u8; 16], slot: &[u8]) -> Vec<u8> {
    let mut aad = b"LIFEBOAT/key-wrapper/v1/".to_vec();
    aad.extend_from_slice(vault_id);
    aad.push(b'/');
    aad.extend_from_slice(slot);
    aad
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn decode_hex<const N: usize>(value: &str) -> Result<[u8; N], CryptoError> {
    if value.len() != N * 2 || !value.is_ascii() {
        return Err(CryptoError::RecoveryCode);
    }
    let mut output = [0u8; N];
    for (index, byte) in output.iter_mut().enumerate() {
        let pair = &value[index * 2..index * 2 + 2];
        *byte = u8::from_str_radix(pair, 16).map_err(|_| CryptoError::RecoveryCode)?;
        if pair.bytes().any(|c| c.is_ascii_uppercase()) {
            return Err(CryptoError::RecoveryCode);
        }
    }
    Ok(output)
}

fn make_recovery_code(secret: &[u8; KEY_LEN]) -> String {
    let checksum = blake3::hash(secret);
    format!(
        "{RECOVERY_PREFIX}{}-{}",
        encode_hex(secret),
        encode_hex(&checksum.as_bytes()[..4])
    )
}

fn parse_recovery_code(code: &str) -> Result<zeroize::Zeroizing<[u8; KEY_LEN]>, CryptoError> {
    let remainder = code.strip_prefix(RECOVERY_PREFIX).ok_or(CryptoError::RecoveryCode)?;
    let (secret_hex, checksum_hex) = remainder.split_once('-').ok_or(CryptoError::RecoveryCode)?;
    if checksum_hex.len() != 8 {
        return Err(CryptoError::RecoveryCode);
    }
    let secret = zeroize::Zeroizing::new(decode_hex::<KEY_LEN>(secret_hex)?);
    let checksum = decode_hex::<4>(checksum_hex)?;
    if blake3::hash(&*secret).as_bytes()[..4] != checksum {
        return Err(CryptoError::RecoveryCode);
    }
    Ok(secret)
}

impl KeyWrapper {
    pub fn validate(&self) -> Result<(), CryptoError> {
        if self.version != WRAPPER_VERSION
            || self.kdf.validate().is_err()
            || self.passphrase_envelope.version != 1
            || self.recovery_envelope.version != 1
            || self.passphrase_envelope.ciphertext.len() != KEY_LEN + 16
            || self.recovery_envelope.ciphertext.len() != KEY_LEN + 16
        {
            return Err(CryptoError::WrapperFormat);
        }
        Ok(())
    }

    /// Bounded, versioned Postcard encoding for client-side storage/printing workflows.
    pub fn encode(&self) -> Result<Vec<u8>, CryptoError> {
        self.validate()?;
        let bytes = postcard::to_allocvec(self).map_err(|_| CryptoError::WrapperFormat)?;
        if bytes.len() > MAX_WRAPPER_BYTES {
            return Err(CryptoError::WrapperFormat);
        }
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, CryptoError> {
        if bytes.is_empty() || bytes.len() > MAX_WRAPPER_BYTES {
            return Err(CryptoError::WrapperFormat);
        }
        let wrapper: Self =
            postcard::from_bytes(bytes).map_err(|_| CryptoError::WrapperFormat)?;
        wrapper.validate()?;
        Ok(wrapper)
    }
}

/// Creates independent passphrase and recovery-code wraps of a random vault master key.
/// The recovery code is returned once and is never included in the serialized wrapper.
pub fn create_key_wrapper(
    master: &SymmetricKey,
    vault_id: [u8; 16],
    passphrase: &[u8],
    kdf: Argon2idConfig,
) -> Result<CreatedKeyWrapper, CryptoError> {
    if passphrase.is_empty() {
        return Err(CryptoError::Kdf);
    }
    let mut salt = [0u8; 16];
    getrandom::fill(&mut salt).map_err(|_| CryptoError::Randomness)?;
    let passphrase_key = derive_backup_key(passphrase, &salt, kdf)?;

    let mut recovery_secret = zeroize::Zeroizing::new([0u8; KEY_LEN]);
    getrandom::fill(&mut *recovery_secret).map_err(|_| CryptoError::Randomness)?;
    let recovery_code = make_recovery_code(&recovery_secret);
    let recovery_key = SymmetricKey::from_bytes(*recovery_secret);

    let wrapper = KeyWrapper {
        version: WRAPPER_VERSION,
        vault_id,
        kdf,
        passphrase_salt: salt,
        passphrase_envelope: seal(
            &passphrase_key,
            master.bytes(),
            &wrapper_aad(&vault_id, b"passphrase"),
        )?,
        recovery_envelope: seal(
            &recovery_key,
            master.bytes(),
            &wrapper_aad(&vault_id, b"recovery"),
        )?,
    };
    wrapper.validate()?;

    Ok(CreatedKeyWrapper {
        wrapper,
        recovery_kit: RecoveryKit {
            recovery_code,
            vault_id,
        },
    })
}

/// Unlocks the master key using the owner's passphrase.
pub fn unlock_with_passphrase(
    wrapper: &KeyWrapper,
    passphrase: &[u8],
) -> Result<SymmetricKey, CryptoError> {
    wrapper.validate()?;
    let key = derive_backup_key(passphrase, &wrapper.passphrase_salt, wrapper.kdf)?;
    let plaintext = zeroize::Zeroizing::new(open(
        &key,
        &wrapper.passphrase_envelope,
        &wrapper_aad(&wrapper.vault_id, b"passphrase"),
    )?);
    let master: [u8; KEY_LEN] = plaintext
        .as_slice()
        .try_into()
        .map_err(|_| CryptoError::WrapperFormat)?;
    Ok(SymmetricKey::from_bytes(master))
}

/// Unlocks the master key from the offline recovery code.
pub fn unlock_with_recovery_code(
    wrapper: &KeyWrapper,
    recovery_code: &str,
) -> Result<SymmetricKey, CryptoError> {
    wrapper.validate()?;
    let secret = parse_recovery_code(recovery_code)?;
    let key = SymmetricKey::from_bytes(*secret);
    let plaintext = zeroize::Zeroizing::new(open(
        &key,
        &wrapper.recovery_envelope,
        &wrapper_aad(&wrapper.vault_id, b"recovery"),
    )?);
    let master: [u8; KEY_LEN] = plaintext
        .as_slice()
        .try_into()
        .map_err(|_| CryptoError::WrapperFormat)?;
    Ok(SymmetricKey::from_bytes(master))
}
