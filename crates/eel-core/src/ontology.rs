//! Claims, evidence, objections, replications, edges and branches.

use crate::ids::*;
use serde::{Deserialize, Serialize};

/// Procedural status of a claim. These are states of the epistemic process,
/// never verdicts about truth.
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
pub enum ClaimStatus {
    Proposed,
    Candidate,
    Supported,
    Robust,
    Contested,
    Superseded,
    Falsified,
    Retired,
}

/// What kind of statement a claim is (decisions D-010).
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
pub enum ClaimType {
    Proposition,
    Definition,
    Criterion,
    Hypothesis,
    Prediction,
    Observation,
    Model,
    Method,
    Test,
}

/// The role a claim plays (decisions D-010).
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
pub enum ClaimNature {
    Constitutive,
    Representational,
    Diagnostic,
    Empirical,
    Formal,
    Interpretive,
}

/// Optional, extensible record of a claim's ontological commitments.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OntologicalDefensibility {
    #[serde(default)]
    pub required_entities: Vec<String>,
    #[serde(default)]
    pub ontology_level: String,
    #[serde(default)]
    pub lower_order_dependencies: Vec<ClaimId>,
    #[serde(default)]
    pub emergent_properties: Vec<String>,
    #[serde(default)]
    pub claim_nature: Vec<ClaimNature>,
    #[serde(default)]
    pub new_primitives: Vec<String>,
    #[serde(default)]
    pub lower_assumption_alternative: Option<String>,
    #[serde(default)]
    pub empirical_constraints: Vec<String>,
    #[serde(default)]
    pub failure_conditions: Vec<String>,
    /// Additional fields for later extensions; kept so that nothing is lost.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub extensions: std::collections::BTreeMap<String, String>,
}

fn default_domain() -> String {
    "general".to_string()
}

/// One version of a claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Claim {
    pub claim_id: ClaimId,
    pub version: u64,
    pub title: String,
    pub statement: String,
    pub claim_type: Vec<ClaimType>,
    pub ontology_level: String,
    pub status: ClaimStatus,
    #[serde(default)]
    pub dependencies: Vec<ClaimId>,
    #[serde(default)]
    pub ontological_defensibility: Option<OntologicalDefensibility>,
    /// Domain used for validator authority (decisions D-012).
    #[serde(default = "default_domain")]
    pub domain: String,
}

/// Typed relation between two objects in the ontology graph.
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
pub enum RelationType {
    DependsOn,
    Supports,
    Contradicts,
    Refines,
    Supersedes,
    Replicates,
    Falsifies,
    Implements,
    DerivesFrom,
    Generalizes,
    Specializes,
    AlternativeTo,
    Observes,
    Predicts,
    UsesDataFrom,
}

/// An explicit, stored edge. Every edge records the event that created it.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, schemars::JsonSchema,
)]
pub struct OntologyEdge {
    pub source: ObjectId,
    pub relation: RelationType,
    pub target: ObjectId,
    pub originating_event: EventId,
}

/// An additional relation declared when a claim is created or revised.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RelationSpec {
    pub relation: RelationType,
    pub target: ObjectId,
}

/// How a piece of evidence bears on its claim.
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
pub enum EvidenceRelation {
    Supports,
    Contradicts,
    Observes,
}

impl EvidenceRelation {
    pub fn as_relation(self) -> RelationType {
        match self {
            EvidenceRelation::Supports => RelationType::Supports,
            EvidenceRelation::Contradicts => RelationType::Contradicts,
            EvidenceRelation::Observes => RelationType::Observes,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Evidence {
    pub evidence_id: EvidenceId,
    pub claim_id: ClaimId,
    pub relation: EvidenceRelation,
    pub description: String,
    #[serde(default)]
    pub artifacts: Vec<ArtifactId>,
    /// A verified simulation result this evidence rests on, if any.
    #[serde(default)]
    pub simulation_result: Option<SimulationResultId>,
    /// A research result (identified by its submitting event) this evidence rests on.
    #[serde(default)]
    pub research_result: Option<EventId>,
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
pub enum ObjectionSeverity {
    Minor,
    Major,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Objection {
    pub objection_id: ObjectionId,
    pub target: ObjectId,
    pub severity: ObjectionSeverity,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Replication {
    pub replication_id: ReplicationId,
    /// The evidence or simulation result being replicated.
    pub target: ObjectId,
    pub protocol: String,
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
pub enum ReplicationOutcome {
    Success,
    Failure,
    Inconclusive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OntologyBranch {
    pub branch_id: BranchId,
    pub name: String,
    pub parent_branch: Option<BranchId>,
    pub fork_event: Option<EventId>,
    pub created_by: IdentityId,
}
