//! Genesis configuration: network identity, constitutional principles,
//! genesis validators and development rules.

use crate::ids::*;
use crate::time::Timestamp;
use crate::validation::{CanonicalizationRule, ValidatorClass};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NetworkConfig {
    pub name: String,
    pub protocol_version: String,
}

/// A validator as written in the genesis file: the public key is given, and
/// the identity id is derived from it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GenesisValidator {
    pub validator_id: ValidatorId,
    /// Ed25519 public key, hex.
    pub public_key: String,
    pub domains: Vec<String>,
    #[serde(default = "genesis_class")]
    pub validator_class: ValidatorClass,
    pub activated_at: Timestamp,
    #[serde(default)]
    pub expires_at: Option<Timestamp>,
}

fn genesis_class() -> ValidatorClass {
    ValidatorClass::Genesis
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GenesisConfig {
    pub network: NetworkConfig,
    pub principles: Vec<String>,
    /// Timestamp of the genesis block.
    pub genesis_time: Timestamp,
    pub validators: Vec<GenesisValidator>,
    #[serde(default)]
    pub canonicalization: CanonicalizationRule,
    /// Validator that seals blocks on the development network (decisions D-006).
    pub block_sealer: ValidatorId,
}

/// The constitutional principles every genesis file must carry.
pub const REQUIRED_PRINCIPLES: &[&str] = &[
    "ledger_consensus_is_not_truth_consensus",
    "economic_stake_is_not_epistemic_authority",
    "minority_branches_remain_recoverable",
    "provenance_is_immutable",
    "scientific_claims_are_revisable",
    "falsification_is_valuable",
    "canonicalization_is_provisional",
];
