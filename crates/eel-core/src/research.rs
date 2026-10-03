//! Research requests, support commitments and research results.
//!
//! Economic objects are protocol structures only: support commitments are
//! counted in arbitrary development units with no financial value.

use crate::ids::*;
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
pub enum ResearchWorkType {
    Experiment,
    Replication,
    Simulation,
    FormalReview,
    LiteratureReview,
    DataCollection,
    CountermodelSearch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ResearchRequest {
    pub request_id: ResearchRequestId,
    pub linked_claims: Vec<ClaimId>,
    pub title: String,
    pub description: String,
    pub requested_work: Vec<ResearchWorkType>,
    #[serde(default)]
    pub acceptance_conditions: Vec<String>,
    pub created_by: IdentityId,
}

/// Which pool a commitment funds (white paper §12; decisions D-024).
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
pub enum CommitmentType {
    Execution,
    Replication,
    SupportRisk,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SupportCommitment {
    pub supporter: IdentityId,
    pub request_id: ResearchRequestId,
    /// Arbitrary development units. No wallets, no financial value.
    pub amount_units: u64,
    pub commitment_type: CommitmentType,
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
pub enum ResearchOutcome {
    Supporting,
    Contradicting,
    Null,
    Inconclusive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ResearchResult {
    pub request_id: ResearchRequestId,
    pub submitted_by: IdentityId,
    pub outcome: ResearchOutcome,
    #[serde(default)]
    pub artifacts: Vec<ArtifactId>,
    pub methodology: String,
    #[serde(default)]
    pub linked_claims: Vec<ClaimId>,
}
