//! Simulation jobs and results. Integer-only so that every node computes
//! identical bytes.

use crate::hash::Hash;
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
pub enum SimulationMetric {
    Lifespan,
    StructuralPersistence,
    PerturbationResistance,
    Recurrence,
    CompressionRatio,
    RecursiveComposability,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MetricValue {
    pub metric: SimulationMetric,
    pub value: i64,
}

/// Integer parameters of the relational graph engine's update rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Ruleset {
    /// Divisor applied to the weighted neighbour sum.
    pub coupling_divisor: i64,
    /// Retention of a vertex's own state, in permille.
    pub self_retention_permille: i64,
    /// Probability (permille) that a vertex is perturbed in a step.
    pub perturbation_permille: i64,
    /// Maximum absolute size of a perturbation.
    pub perturbation_magnitude: i64,
    /// States are clamped to [-state_bound, state_bound].
    pub state_bound: i64,
    /// A vertex is persistent when |state| >= this for the whole memory window.
    pub persistence_threshold: i64,
    /// Number of past states each vertex remembers.
    pub memory_length: u32,
}

/// Parameters from which the initial graph is generated with the seed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct InitialConditions {
    pub vertices: u32,
    /// Probability (permille) of a directed edge between two distinct vertices.
    pub edge_permille: i64,
    /// Edge weights are drawn from [-weight_bound, weight_bound].
    pub weight_bound: i64,
    /// Initial states are drawn from [-initial_state_bound, initial_state_bound].
    pub initial_state_bound: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SimulationJob {
    pub job_id: SimulationJobId,
    #[serde(default)]
    pub linked_claims: Vec<ClaimId>,
    pub engine_version: String,
    pub initial_conditions_hash: ContentHash,
    pub ruleset_hash: ContentHash,
    pub seed_start: u64,
    pub seed_end: u64,
    pub steps: u64,
    pub metrics: Vec<SimulationMetric>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SimulationResult {
    /// BLAKE3 of the canonical result with this field set to all zeros.
    pub result_id: SimulationResultId,
    pub job_id: SimulationJobId,
    pub worker: IdentityId,
    pub seed: u64,
    pub engine_hash: ContentHash,
    pub ruleset_hash: ContentHash,
    pub initial_state_hash: ContentHash,
    pub final_state_hash: ContentHash,
    pub metrics: Vec<MetricValue>,
    pub result_artifact: ArtifactId,
}

impl SimulationResult {
    /// Computes the content-derived result id.
    pub fn compute_id(&self) -> crate::error::Result<SimulationResultId> {
        use crate::CanonicalSerialize;
        let mut copy = self.clone();
        copy.result_id = SimulationResultId(Hash::ZERO);
        Ok(SimulationResultId(copy.canonical_hash()?))
    }
}
