//! YAML input formats for CLI commands. Fields that the CLI can fill in
//! (versions, statuses, the current identity, hashes) are optional here.

use eel_core::event::SimulationDefine;
use eel_core::ontology::*;
use eel_core::research::*;
use eel_core::simulation::*;
use eel_core::xchange::*;
use eel_core::*;
use serde::Deserialize;

fn general() -> String {
    "general".into()
}

#[derive(Deserialize)]
pub struct ClaimInput {
    pub claim_id: ClaimId,
    pub title: String,
    pub statement: String,
    pub claim_type: Vec<ClaimType>,
    #[serde(default)]
    pub ontology_level: String,
    #[serde(default)]
    pub dependencies: Vec<ClaimId>,
    #[serde(default)]
    pub ontological_defensibility: Option<OntologicalDefensibility>,
    #[serde(default = "general")]
    pub domain: String,
    #[serde(default)]
    pub relations: Vec<RelationSpec>,
    /// Used by `claim revise`.
    #[serde(default)]
    pub rationale: Option<String>,
}

impl ClaimInput {
    pub fn into_claim(self, version: u64) -> (Claim, Vec<RelationSpec>) {
        (
            Claim {
                claim_id: self.claim_id,
                version,
                title: self.title,
                statement: self.statement,
                claim_type: self.claim_type,
                ontology_level: self.ontology_level,
                status: ClaimStatus::Proposed,
                dependencies: self.dependencies,
                ontological_defensibility: self.ontological_defensibility,
                domain: self.domain,
            },
            self.relations,
        )
    }
}

#[derive(Deserialize)]
pub struct ResearchRequestInput {
    pub request_id: ResearchRequestId,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub linked_claims: Vec<ClaimId>,
    pub requested_work: Vec<ResearchWorkType>,
    #[serde(default)]
    pub acceptance_conditions: Vec<String>,
}

impl ResearchRequestInput {
    pub fn into_request(self, me: IdentityId) -> ResearchRequest {
        ResearchRequest {
            request_id: self.request_id,
            linked_claims: self.linked_claims,
            title: self.title,
            description: self.description,
            requested_work: self.requested_work,
            acceptance_conditions: self.acceptance_conditions,
            created_by: me,
        }
    }
}

#[derive(Deserialize)]
pub struct ResearchResultInput {
    pub request_id: ResearchRequestId,
    pub outcome: ResearchOutcome,
    #[serde(default)]
    pub artifacts: Vec<ArtifactId>,
    pub methodology: String,
    #[serde(default)]
    pub linked_claims: Vec<ClaimId>,
}

impl ResearchResultInput {
    pub fn into_result(self, me: IdentityId) -> ResearchResult {
        ResearchResult {
            request_id: self.request_id,
            submitted_by: me,
            outcome: self.outcome,
            artifacts: self.artifacts,
            methodology: self.methodology,
            linked_claims: self.linked_claims,
        }
    }
}

#[derive(Deserialize)]
pub struct XchangeAssetInput {
    pub asset_id: XchangeAssetId,
    pub asset_type: XchangeAssetType,
    #[serde(default)]
    pub artifact_ref: Option<ArtifactId>,
    #[serde(default)]
    pub ontology_links: Vec<ObjectId>,
    pub access_terms: AccessTerms,
}

impl XchangeAssetInput {
    pub fn into_asset(self, me: IdentityId) -> XchangeAsset {
        XchangeAsset {
            asset_id: self.asset_id,
            asset_type: self.asset_type,
            artifact_ref: self.artifact_ref,
            ontology_links: self.ontology_links,
            provider: me,
            access_terms: self.access_terms,
        }
    }
}

/// A simulation job with its ruleset and initial conditions inline; the CLI
/// computes the hashes.
#[derive(Deserialize)]
pub struct SimulationInput {
    pub job_id: SimulationJobId,
    #[serde(default)]
    pub linked_claims: Vec<ClaimId>,
    pub ruleset: Ruleset,
    pub initial_conditions: InitialConditions,
    pub seed_start: u64,
    pub seed_end: u64,
    pub steps: u64,
    #[serde(default)]
    pub metrics: Vec<SimulationMetric>,
}

impl SimulationInput {
    pub fn into_define(self) -> Result<SimulationDefine, eel_simulation::SimError> {
        let metrics = if self.metrics.is_empty() {
            eel_simulation::all_metrics()
        } else {
            self.metrics
        };
        let job = eel_simulation::make_job(
            self.job_id,
            self.linked_claims,
            &self.ruleset,
            &self.initial_conditions,
            self.seed_start,
            self.seed_end,
            self.steps,
            metrics,
        )?;
        Ok(SimulationDefine {
            job,
            ruleset: self.ruleset,
            initial_conditions: self.initial_conditions,
        })
    }
}
