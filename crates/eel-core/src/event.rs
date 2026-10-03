//! The epistemic event: the only way state ever changes.

use crate::canonical::CanonicalSerialize;
use crate::error::Result;
use crate::ids::*;
use crate::ontology::*;
use crate::research::*;
use crate::simulation::*;
use crate::time::Timestamp;
use crate::validation::*;
use crate::xchange::*;
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
pub enum EventType {
    IdentityRegister,
    ClaimCreate,
    ClaimRevise,
    ClaimSupersede,
    EvidenceAdd,
    EvidenceRetract,
    ObjectionAdd,
    ObjectionReply,
    ReplicationRegister,
    ReplicationResult,
    ValidationSubmit,
    BranchCreate,
    BranchMergePropose,
    CanonicalizationPropose,
    CanonicalizationAccept,
    CanonicalizationReject,
    ArtifactRegister,
    SimulationDefine,
    SimulationResult,
    SimulationVerify,
    ResearchRequestCreate,
    ResearchRequestSupport,
    ResearchResultSubmit,
    /// Not in the specification's event list; required by the
    /// `POST /xchange/assets` endpoint (decisions D-025).
    XchangeAssetRegister,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct IdentityRegister {
    pub identity_id: IdentityId,
    /// Ed25519 public key, hex.
    pub public_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ClaimCreate {
    pub claim: Claim,
    #[serde(default)]
    pub relations: Vec<RelationSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ClaimRevise {
    /// The full new version. `version` must be the current version plus one.
    pub claim: Claim,
    #[serde(default)]
    pub relations: Vec<RelationSpec>,
    #[serde(default)]
    pub rationale: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ClaimSupersede {
    pub claim_id: ClaimId,
    pub superseded_by: ClaimId,
    #[serde(default)]
    pub rationale: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct EvidenceRetract {
    pub evidence_id: EvidenceId,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ObjectionReply {
    pub objection_id: ObjectionId,
    pub text: String,
    /// Only the objection's author can mark it resolved (decisions D-015).
    #[serde(default)]
    pub resolves: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ReplicationResultPayload {
    pub replication_id: ReplicationId,
    pub outcome: ReplicationOutcome,
    #[serde(default)]
    pub artifacts: Vec<ArtifactId>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BranchMergePropose {
    pub proposal_id: MergeProposalId,
    pub source: BranchId,
    pub target: BranchId,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CanonicalizationDecision {
    pub proposal_id: CanonicalizationProposalId,
    pub validator: ValidatorId,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SimulationDefine {
    pub job: SimulationJob,
    /// The ruleset and initial conditions are registered with the job so that
    /// any node can replay results; their hashes must match the job.
    pub ruleset: Ruleset,
    pub initial_conditions: InitialConditions,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SimulationVerify {
    pub result_id: SimulationResultId,
    pub validator: ValidatorId,
    /// The verifier's replay outcome. The projector replays the result itself
    /// and rejects the event if this value is wrong (decisions D-021).
    pub reproduced: bool,
}

/// The typed payload. The variant must match the event's `event_type`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EventPayload {
    IdentityRegister(IdentityRegister),
    ClaimCreate(ClaimCreate),
    ClaimRevise(ClaimRevise),
    ClaimSupersede(ClaimSupersede),
    EvidenceAdd(Evidence),
    EvidenceRetract(EvidenceRetract),
    ObjectionAdd(Objection),
    ObjectionReply(ObjectionReply),
    ReplicationRegister(Replication),
    ReplicationResult(ReplicationResultPayload),
    ValidationSubmit(ValidationRecord),
    BranchCreate(OntologyBranch),
    BranchMergePropose(BranchMergePropose),
    CanonicalizationPropose(CanonicalizationProposal),
    CanonicalizationAccept(CanonicalizationDecision),
    CanonicalizationReject(CanonicalizationDecision),
    ArtifactRegister(Artifact),
    SimulationDefine(SimulationDefine),
    SimulationResult(SimulationResult),
    SimulationVerify(SimulationVerify),
    ResearchRequestCreate(ResearchRequest),
    ResearchRequestSupport(SupportCommitment),
    ResearchResultSubmit(ResearchResult),
    XchangeAssetRegister(XchangeAsset),
}

impl EventPayload {
    pub fn event_type(&self) -> EventType {
        use EventPayload as P;
        match self {
            P::IdentityRegister(_) => EventType::IdentityRegister,
            P::ClaimCreate(_) => EventType::ClaimCreate,
            P::ClaimRevise(_) => EventType::ClaimRevise,
            P::ClaimSupersede(_) => EventType::ClaimSupersede,
            P::EvidenceAdd(_) => EventType::EvidenceAdd,
            P::EvidenceRetract(_) => EventType::EvidenceRetract,
            P::ObjectionAdd(_) => EventType::ObjectionAdd,
            P::ObjectionReply(_) => EventType::ObjectionReply,
            P::ReplicationRegister(_) => EventType::ReplicationRegister,
            P::ReplicationResult(_) => EventType::ReplicationResult,
            P::ValidationSubmit(_) => EventType::ValidationSubmit,
            P::BranchCreate(_) => EventType::BranchCreate,
            P::BranchMergePropose(_) => EventType::BranchMergePropose,
            P::CanonicalizationPropose(_) => EventType::CanonicalizationPropose,
            P::CanonicalizationAccept(_) => EventType::CanonicalizationAccept,
            P::CanonicalizationReject(_) => EventType::CanonicalizationReject,
            P::ArtifactRegister(_) => EventType::ArtifactRegister,
            P::SimulationDefine(_) => EventType::SimulationDefine,
            P::SimulationResult(_) => EventType::SimulationResult,
            P::SimulationVerify(_) => EventType::SimulationVerify,
            P::ResearchRequestCreate(_) => EventType::ResearchRequestCreate,
            P::ResearchRequestSupport(_) => EventType::ResearchRequestSupport,
            P::ResearchResultSubmit(_) => EventType::ResearchResultSubmit,
            P::XchangeAssetRegister(_) => EventType::XchangeAssetRegister,
        }
    }
}

/// Everything in an event except its signature. Its canonical hash is the
/// event id, and the signature is made over that id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct UnsignedEvent {
    pub schema_version: String,
    pub event_type: EventType,
    pub timestamp: Timestamp,
    pub author: IdentityId,
    pub branch: BranchId,
    pub parent_events: Vec<EventId>,
    pub payload: EventPayload,
    #[serde(default)]
    pub artifact_refs: Vec<ArtifactId>,
}

impl UnsignedEvent {
    pub fn event_id(&self) -> Result<EventId> {
        Ok(EventId(self.canonical_hash()?))
    }
}

/// A signed epistemic event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct EpistemicEvent {
    pub schema_version: String,
    pub event_type: EventType,
    pub timestamp: Timestamp,
    pub author: IdentityId,
    pub branch: BranchId,
    pub parent_events: Vec<EventId>,
    pub payload: EventPayload,
    #[serde(default)]
    pub artifact_refs: Vec<ArtifactId>,
    /// Ed25519 signature over the 32 bytes of the event id, hex.
    pub signature: String,
}

impl EpistemicEvent {
    pub fn unsigned(&self) -> UnsignedEvent {
        UnsignedEvent {
            schema_version: self.schema_version.clone(),
            event_type: self.event_type,
            timestamp: self.timestamp,
            author: self.author,
            branch: self.branch.clone(),
            parent_events: self.parent_events.clone(),
            payload: self.payload.clone(),
            artifact_refs: self.artifact_refs.clone(),
        }
    }

    pub fn event_id(&self) -> Result<EventId> {
        self.unsigned().event_id()
    }

    pub fn from_unsigned(u: UnsignedEvent, signature: String) -> Self {
        EpistemicEvent {
            schema_version: u.schema_version,
            event_type: u.event_type,
            timestamp: u.timestamp,
            author: u.author,
            branch: u.branch,
            parent_events: u.parent_events,
            payload: u.payload,
            artifact_refs: u.artifact_refs,
            signature,
        }
    }
}
