//! The node's data directory:
//!
//! ```text
//! <data-dir>/
//!   genesis.yaml     genesis configuration (written on first use)
//!   sealer.key       development block-sealer key (decisions D-006)
//!   eel.db           SQLite ledger
//!   keys/            local identities (private keys never leave this machine)
//!   artifacts/       copies of registered artifact content, by artifact id
//! ```

use eel_core::genesis::*;
use eel_core::validation::{CanonicalizationRule, ValidatorClass};
use eel_core::*;
use eel_crypto::{KeyPair, Keystore};
use eel_storage::PersistentNode;
use std::path::{Path, PathBuf};

/// Deterministic development key for a named role. These keys are public and
/// worthless outside the development network.
pub fn dev_key(role: &str) -> KeyPair {
    KeyPair::from_seed(*Hash::digest(format!("EEL development key: {role}").as_bytes()).as_bytes())
}

pub const DEV_VALIDATORS: &[(&str, &[&str])] = &[
    ("VAL-A", &["general", "ontology", "physics"]),
    ("VAL-B", &["general", "ontology", "physics"]),
    ("VAL-C", &["general", "physics"]),
];

/// The development network's genesis: three genesis validators, VAL-A seals.
pub fn dev_genesis() -> GenesisConfig {
    GenesisConfig {
        network: NetworkConfig {
            name: "EEL Development Network".into(),
            protocol_version: "0.1".into(),
        },
        principles: REQUIRED_PRINCIPLES.iter().map(|s| s.to_string()).collect(),
        genesis_time: Timestamp(1_767_225_600_000), // 2026-01-01T00:00:00Z
        validators: DEV_VALIDATORS
            .iter()
            .map(|(id, domains)| GenesisValidator {
                validator_id: id.parse().unwrap(),
                public_key: dev_key(id).public_key_hex(),
                domains: domains.iter().map(|d| d.to_string()).collect(),
                validator_class: ValidatorClass::Genesis,
                activated_at: Timestamp(0),
                expires_at: None,
            })
            .collect(),
        canonicalization: CanonicalizationRule::default(),
        block_sealer: "VAL-A".parse().unwrap(),
    }
}

pub fn keystore(data_dir: &Path) -> Result<Keystore, eel_crypto::CryptoError> {
    Keystore::open(data_dir.join("keys"))
}

pub fn artifacts_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("artifacts")
}

/// Writes the development genesis, sealer key and validator keys if the
/// directory has no genesis yet.
pub fn ensure_initialized(data_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(data_dir)?;
    let genesis_path = data_dir.join("genesis.yaml");
    if !genesis_path.exists() {
        std::fs::write(&genesis_path, serde_yaml::to_string(&dev_genesis())?)?;
        let ks = keystore(data_dir)?;
        for (id, _) in DEV_VALIDATORS {
            ks.save(&dev_key(id))?;
        }
        let sealer = dev_key("VAL-A");
        let sealer_path = ks.save(&sealer)?;
        std::fs::copy(
            data_dir.join("keys").join(format!("{sealer_path}.key")),
            data_dir.join("sealer.key"),
        )?;
    }
    std::fs::create_dir_all(artifacts_dir(data_dir))?;
    Ok(())
}

pub fn load_genesis(data_dir: &Path) -> Result<GenesisConfig, Box<dyn std::error::Error>> {
    Ok(serde_yaml::from_str(&std::fs::read_to_string(
        data_dir.join("genesis.yaml"),
    )?)?)
}

pub async fn open_node(data_dir: &Path) -> Result<PersistentNode, Box<dyn std::error::Error>> {
    ensure_initialized(data_dir)?;
    let genesis = load_genesis(data_dir)?;
    let sealer_path = data_dir.join("sealer.key");
    let sealer = if sealer_path.exists() {
        Some(Keystore::load_file(&sealer_path)?)
    } else {
        None
    };
    Ok(PersistentNode::open(&data_dir.join("eel.db"), Some(genesis), sealer).await?)
}
