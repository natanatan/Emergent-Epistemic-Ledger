//! Validators, validation records and canonicalization proposals.

use crate::ids::*;
use crate::ontology::ClaimStatus;
use crate::time::Timestamp;
use serde::{Deserialize, Serialize};

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ValidatorClass {
    Genesis,
    Earned,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Validator {
    pub validator_id: ValidatorId,
    pub identity_id: IdentityId,
    pub domains: Vec<String>,
    pub validator_class: ValidatorClass,
    pub activated_at: Timestamp,
    pub expires_at: Option<Timestamp>,
}

impl Validator {
    pub fn is_active_at(&self, t: Timestamp) -> bool {
        t >= self.activated_at && self.expires_at.is_none_or(|e| t < e)
    }
    pub fn covers_domain(&self, domain: &str) -> bool {
        self.domains.iter().any(|d| d == domain)
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ValidationType {
    MethodConformance,
    ArtifactIntegrity,
    SimulationReplay,
    ReplicationConformance,
    DataProvenance,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ValidationOutcome {
    Pass,
    Fail,
    Inconclusive,
}

/// A procedural validation. It states that something was done correctly,
/// never that a claim is true.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ValidationRecord {
    pub validation_id: ValidationId,
    pub validator: ValidatorId,
    pub target: ObjectId,
    pub validation_type: ValidationType,
    pub outcome: ValidationOutcome,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CanonicalizationProposal {
    pub proposal_id: CanonicalizationProposalId,
    pub target_claim: ClaimId,
    pub target_version: u64,
    pub proposed_status: ClaimStatus,
    #[serde(default)]
    pub evidence: Vec<EvidenceId>,
    #[serde(default)]
    pub validations: Vec<ValidationId>,
    #[serde(default)]
    pub successful_replications: Vec<ReplicationId>,
    #[serde(default)]
    pub unresolved_objections: Vec<ObjectionId>,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProposalState {
    Open,
    Accepted,
    Rejected,
}

/// Configurable development thresholds. These are not claimed to be
/// scientifically optimal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CanonicalizationRule {
    pub minimum_validators: u32,
    pub minimum_independent_replications: u32,
    pub maximum_unresolved_critical_objections: u32,
}

impl Default for CanonicalizationRule {
    fn default() -> Self {
        CanonicalizationRule {
            minimum_validators: 2,
            minimum_independent_replications: 2,
            maximum_unresolved_critical_objections: 0,
        }
    }
}
