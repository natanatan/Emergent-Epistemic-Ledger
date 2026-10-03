//! Identities, signatures and the local keystore.
//!
//! * Event ids are BLAKE3 of the canonical unsigned event (see `eel-core`).
//! * Signatures are Ed25519 over the 32 bytes of the event id.
//! * Identity ids are BLAKE3 of the 32-byte public key.
//! * Private keys live only in the local keystore and are never sent to a node.

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use eel_core::event::IdentityRegister;
use eel_core::{
    BranchId, EpistemicEvent, EventId, EventPayload, Hash, IdentityId, Timestamp, UnsignedEvent,
    SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("invalid public key")]
    InvalidPublicKey,
    #[error("invalid signature encoding")]
    InvalidSignatureEncoding,
    #[error("signature does not verify")]
    BadSignature,
    #[error("event type does not match payload")]
    TypeMismatch,
    #[error("unsupported schema version `{0}`")]
    SchemaVersion(String),
    #[error("randomness unavailable: {0}")]
    Random(String),
    #[error("keystore: {0}")]
    Keystore(String),
    #[error(transparent)]
    Core(#[from] eel_core::CoreError),
}

pub type Result<T> = std::result::Result<T, CryptoError>;

/// Identity id for a public key.
pub fn identity_id_for(public_key: &[u8; 32]) -> IdentityId {
    IdentityId(Hash::digest(public_key))
}

/// Parses a hex Ed25519 public key.
pub fn parse_public_key(hex_key: &str) -> Result<VerifyingKey> {
    let bytes = hex::decode(hex_key).map_err(|_| CryptoError::InvalidPublicKey)?;
    let arr: [u8; 32] = bytes
        .try_into()
        .map_err(|_| CryptoError::InvalidPublicKey)?;
    VerifyingKey::from_bytes(&arr).map_err(|_| CryptoError::InvalidPublicKey)
}

/// An identity's key pair, held only on the author's machine.
#[derive(Clone)]
pub struct KeyPair {
    signing: SigningKey,
}

impl KeyPair {
    pub fn generate() -> Result<Self> {
        let mut seed = [0u8; 32];
        getrandom::getrandom(&mut seed).map_err(|e| CryptoError::Random(e.to_string()))?;
        Ok(Self::from_seed(seed))
    }

    /// Deterministic key from a 32-byte seed (used by fixtures and tests).
    pub fn from_seed(seed: [u8; 32]) -> Self {
        KeyPair {
            signing: SigningKey::from_bytes(&seed),
        }
    }

    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.signing.verifying_key().to_bytes()
    }

    pub fn public_key_hex(&self) -> String {
        hex::encode(self.public_key_bytes())
    }

    pub fn identity_id(&self) -> IdentityId {
        identity_id_for(&self.public_key_bytes())
    }

    pub fn seed_hex(&self) -> String {
        hex::encode(self.signing.to_bytes())
    }

    pub fn sign_bytes(&self, msg: &[u8]) -> String {
        hex::encode(self.signing.sign(msg).to_bytes())
    }

    /// Signs an unsigned event, producing the event and its id.
    pub fn sign_event(&self, unsigned: UnsignedEvent) -> Result<(EventId, EpistemicEvent)> {
        if unsigned.payload.event_type() != unsigned.event_type {
            return Err(CryptoError::TypeMismatch);
        }
        let id = unsigned.event_id()?;
        let sig = self.sign_bytes(id.as_hash().as_bytes());
        Ok((id, EpistemicEvent::from_unsigned(unsigned, sig)))
    }

    /// Builds and signs an event authored by this key.
    pub fn author_event(
        &self,
        timestamp: Timestamp,
        branch: BranchId,
        parent_events: Vec<EventId>,
        payload: EventPayload,
    ) -> Result<(EventId, EpistemicEvent)> {
        let unsigned = UnsignedEvent {
            schema_version: SCHEMA_VERSION.to_string(),
            event_type: payload.event_type(),
            timestamp,
            author: self.identity_id(),
            branch,
            parent_events,
            artifact_refs: vec![],
            payload,
        };
        self.sign_event(unsigned)
    }

    /// The self-signed IDENTITY_REGISTER event for this key.
    pub fn identity_register_event(
        &self,
        timestamp: Timestamp,
    ) -> Result<(EventId, EpistemicEvent)> {
        self.author_event(
            timestamp,
            BranchId::main(),
            vec![],
            EventPayload::IdentityRegister(IdentityRegister {
                identity_id: self.identity_id(),
                public_key: self.public_key_hex(),
            }),
        )
    }
}

/// Verifies a hex signature over `msg` with a hex public key.
pub fn verify_bytes(public_key_hex: &str, msg: &[u8], signature_hex: &str) -> Result<()> {
    let key = parse_public_key(public_key_hex)?;
    let bytes = hex::decode(signature_hex).map_err(|_| CryptoError::InvalidSignatureEncoding)?;
    let arr: [u8; 64] = bytes
        .try_into()
        .map_err(|_| CryptoError::InvalidSignatureEncoding)?;
    key.verify(msg, &Signature::from_bytes(&arr))
        .map_err(|_| CryptoError::BadSignature)
}

/// Checks the structural and cryptographic validity of an event, given the
/// author's public key. Returns the event id.
pub fn verify_event(event: &EpistemicEvent, author_public_key_hex: &str) -> Result<EventId> {
    if event.schema_version != SCHEMA_VERSION {
        return Err(CryptoError::SchemaVersion(event.schema_version.clone()));
    }
    if event.payload.event_type() != event.event_type {
        return Err(CryptoError::TypeMismatch);
    }
    let key = parse_public_key(author_public_key_hex)?;
    if identity_id_for(&key.to_bytes()) != event.author {
        return Err(CryptoError::BadSignature);
    }
    let id = event.event_id()?;
    verify_bytes(
        author_public_key_hex,
        id.as_hash().as_bytes(),
        &event.signature,
    )?;
    Ok(id)
}

/// Optional SHA-256 for external artifact compatibility.
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest;
    hex::encode(sha2::Sha256::digest(bytes))
}

/// On-disk key file. The seed is stored unencrypted in the MVP (decisions D-004).
#[derive(Serialize, Deserialize)]
struct KeyFile {
    identity_id: IdentityId,
    public_key: String,
    seed: String,
}

/// A directory of key files: `<dir>/<identity_id>.key`, plus `default`.
pub struct Keystore {
    dir: PathBuf,
}

impl Keystore {
    pub fn open(dir: impl AsRef<Path>) -> Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&dir).map_err(|e| CryptoError::Keystore(e.to_string()))?;
        Ok(Keystore { dir })
    }

    pub fn save(&self, key: &KeyPair) -> Result<IdentityId> {
        let id = key.identity_id();
        let file = KeyFile {
            identity_id: id,
            public_key: key.public_key_hex(),
            seed: key.seed_hex(),
        };
        let path = self.dir.join(format!("{id}.key"));
        let body = serde_json::to_string_pretty(&file).unwrap();
        write_private(&path, body.as_bytes()).map_err(|e| CryptoError::Keystore(e.to_string()))?;
        Ok(id)
    }

    pub fn load(&self, id: &IdentityId) -> Result<KeyPair> {
        Self::load_file(&self.dir.join(format!("{id}.key")))
    }

    pub fn load_file(path: &Path) -> Result<KeyPair> {
        let body = std::fs::read_to_string(path)
            .map_err(|e| CryptoError::Keystore(format!("{}: {e}", path.display())))?;
        let file: KeyFile =
            serde_json::from_str(&body).map_err(|e| CryptoError::Keystore(e.to_string()))?;
        let seed: [u8; 32] = hex::decode(&file.seed)
            .ok()
            .and_then(|b| b.try_into().ok())
            .ok_or_else(|| CryptoError::Keystore("bad seed".into()))?;
        let key = KeyPair::from_seed(seed);
        if key.identity_id() != file.identity_id {
            return Err(CryptoError::Keystore("key file identity mismatch".into()));
        }
        Ok(key)
    }

    pub fn set_default(&self, id: &IdentityId) -> Result<()> {
        std::fs::write(self.dir.join("default"), id.to_string())
            .map_err(|e| CryptoError::Keystore(e.to_string()))
    }

    pub fn default_identity(&self) -> Result<IdentityId> {
        let s = std::fs::read_to_string(self.dir.join("default")).map_err(|_| {
            CryptoError::Keystore("no default identity; run `eel identity create`".into())
        })?;
        s.trim().parse().map_err(CryptoError::Core)
    }

    pub fn list(&self) -> Result<Vec<IdentityId>> {
        let mut out = vec![];
        for entry in
            std::fs::read_dir(&self.dir).map_err(|e| CryptoError::Keystore(e.to_string()))?
        {
            let name = entry
                .map_err(|e| CryptoError::Keystore(e.to_string()))?
                .file_name();
            if let Some(stem) = name.to_string_lossy().strip_suffix(".key") {
                if let Ok(id) = stem.parse() {
                    out.push(id);
                }
            }
        }
        out.sort();
        Ok(out)
    }
}

#[cfg(unix)]
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    f.write_all(bytes)
}

#[cfg(not(unix))]
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::write(path, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_signature_accepted_and_invalid_rejected() {
        let key = KeyPair::from_seed([7; 32]);
        let (id, event) = key.identity_register_event(Timestamp(1)).unwrap();
        assert_eq!(verify_event(&event, &key.public_key_hex()).unwrap(), id);

        let mut tampered = event.clone();
        tampered.timestamp = Timestamp(2);
        assert!(matches!(
            verify_event(&tampered, &key.public_key_hex()),
            Err(CryptoError::BadSignature)
        ));

        let other = KeyPair::from_seed([8; 32]);
        assert!(verify_event(&event, &other.public_key_hex()).is_err());
    }

    #[test]
    fn keystore_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let ks = Keystore::open(dir.path()).unwrap();
        let key = KeyPair::generate().unwrap();
        let id = ks.save(&key).unwrap();
        ks.set_default(&id).unwrap();
        assert_eq!(ks.default_identity().unwrap(), id);
        assert_eq!(ks.load(&id).unwrap().public_key_hex(), key.public_key_hex());
        assert_eq!(ks.list().unwrap(), vec![id]);
    }
}
