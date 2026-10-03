use eel_core::event::{BranchMergePropose, SimulationDefine};
use eel_core::genesis::GenesisConfig;
use eel_core::ontology::*;
use eel_core::research::*;
use eel_core::simulation::SimulationResult;
use eel_core::validation::*;
use eel_core::xchange::*;
use eel_core::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityRecord {
    pub identity_id: IdentityId,
    pub public_key: String,
    pub created_at: Timestamp,
    /// None for identities created by genesis.
    pub registered_by: Option<EventId>,
}

/// Provenance common to every object: who created it, in which event, when.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    pub author: IdentityId,
    pub event: EventId,
    pub timestamp: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimVersionRecord {
    pub claim: Claim,
    pub provenance: Provenance,
    pub rationale: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimRecord {
    /// The current version, including its procedural status.
    pub current: Claim,
    /// Every version ever recorded, oldest first. Never shortened.
    pub versions: Vec<ClaimVersionRecord>,
    pub created: Provenance,
    pub superseded_by: Option<ClaimId>,
    /// Status changes, oldest first.
    pub status_history: Vec<StatusChange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusChange {
    pub version: u64,
    pub status: ClaimStatus,
    pub event: EventId,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceRecord {
    pub evidence: Evidence,
    pub provenance: Provenance,
    pub retracted: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplyRecord {
    pub text: String,
    pub resolves: bool,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectionRecord {
    pub objection: Objection,
    pub provenance: Provenance,
    pub replies: Vec<ReplyRecord>,
    pub resolved: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationRecord {
    pub replication: Replication,
    pub provenance: Provenance,
    pub outcome: Option<ReplicationOutcome>,
    pub result_event: Option<EventId>,
    pub result_artifacts: Vec<ArtifactId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationEntry {
    pub record: ValidationRecord,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposalRecord {
    pub proposal: CanonicalizationProposal,
    pub provenance: Provenance,
    pub accepts: BTreeSet<ValidatorId>,
    pub rejects: BTreeSet<ValidatorId>,
    pub state: ProposalState,
    /// Why the proposal is not yet accepted, as of the last vote.
    pub blockers: Vec<String>,
}

/// Everything that belongs to one ontology branch.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BranchState {
    pub claims: BTreeMap<ClaimId, ClaimRecord>,
    pub edges: BTreeSet<OntologyEdge>,
    pub evidence: BTreeMap<EvidenceId, EvidenceRecord>,
    pub objections: BTreeMap<ObjectionId, ObjectionRecord>,
    pub replications: BTreeMap<ReplicationId, ReplicationRecord>,
    pub validations: BTreeMap<ValidationId, ValidationEntry>,
    pub proposals: BTreeMap<CanonicalizationProposalId, ProposalRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BranchRecord {
    pub branch: OntologyBranch,
    /// The last event recorded on this branch.
    pub head: Option<EventId>,
    pub event_count: u64,
    pub state: BranchState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimulationJobRecord {
    pub definition: SimulationDefine,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimulationResultRecord {
    pub result: SimulationResult,
    pub provenance: Provenance,
    /// Replay outcome per verifying validator.
    pub verifications: BTreeMap<ValidatorId, bool>,
}

impl SimulationResultRecord {
    pub fn successful_replays(&self) -> usize {
        self.verifications.values().filter(|v| **v).count()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchRequestRecord {
    pub request: ResearchRequest,
    pub provenance: Provenance,
    pub commitments: Vec<SupportCommitment>,
    /// Submitting events of results for this request.
    pub results: Vec<EventId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchResultRecord {
    pub result: ResearchResult,
    pub provenance: Provenance,
    pub passing_validators: BTreeSet<ValidatorId>,
    pub credited: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergeProposalRecord {
    pub proposal: BranchMergePropose,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkInfo {
    pub name: String,
    pub protocol_version: String,
    pub principles: Vec<String>,
    pub canonicalization: CanonicalizationRule,
    pub block_sealer: ValidatorId,
}

/// The full projected state. All maps are ordered, so its canonical
/// serialization (and therefore the state root) is deterministic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OntologyState {
    pub network: NetworkInfo,
    pub identities: BTreeMap<IdentityId, IdentityRecord>,
    pub validators: BTreeMap<ValidatorId, Validator>,
    pub branches: BTreeMap<BranchId, BranchRecord>,
    pub artifacts: BTreeMap<ArtifactId, Artifact>,
    pub simulation_jobs: BTreeMap<SimulationJobId, SimulationJobRecord>,
    pub simulation_results: BTreeMap<SimulationResultId, SimulationResultRecord>,
    pub research_requests: BTreeMap<ResearchRequestId, ResearchRequestRecord>,
    pub research_results: BTreeMap<EventId, ResearchResultRecord>,
    pub xchange_assets: BTreeMap<XchangeAssetId, (XchangeAsset, Provenance)>,
    pub merge_proposals: BTreeMap<MergeProposalId, MergeProposalRecord>,
    /// Non-transferable protocol credit for validated work.
    pub protocol_credit: BTreeMap<IdentityId, u64>,
    pub events_applied: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum GenesisError {
    #[error("genesis is missing the principle `{0}`")]
    MissingPrinciple(String),
    #[error("block sealer `{0}` is not a genesis validator")]
    UnknownSealer(ValidatorId),
    #[error("duplicate validator `{0}`")]
    DuplicateValidator(ValidatorId),
    #[error("validator `{0}`: {1}")]
    BadValidator(ValidatorId, String),
}

impl OntologyState {
    /// State before any event: genesis validators, their identities, and `main`.
    pub fn genesis(config: &GenesisConfig) -> std::result::Result<Self, GenesisError> {
        for p in eel_core::genesis::REQUIRED_PRINCIPLES {
            if !config.principles.iter().any(|x| x == p) {
                return Err(GenesisError::MissingPrinciple(p.to_string()));
            }
        }
        let mut identities = BTreeMap::new();
        let mut validators = BTreeMap::new();
        for gv in &config.validators {
            let key = eel_crypto::parse_public_key(&gv.public_key)
                .map_err(|e| GenesisError::BadValidator(gv.validator_id.clone(), e.to_string()))?;
            let identity_id = eel_crypto::identity_id_for(&key.to_bytes());
            if gv.domains.is_empty() {
                return Err(GenesisError::BadValidator(
                    gv.validator_id.clone(),
                    "no domains".into(),
                ));
            }
            let v = Validator {
                validator_id: gv.validator_id.clone(),
                identity_id,
                domains: gv.domains.clone(),
                validator_class: gv.validator_class,
                activated_at: gv.activated_at,
                expires_at: gv.expires_at,
            };
            if validators.insert(gv.validator_id.clone(), v).is_some() {
                return Err(GenesisError::DuplicateValidator(gv.validator_id.clone()));
            }
            identities.insert(
                identity_id,
                IdentityRecord {
                    identity_id,
                    public_key: gv.public_key.clone(),
                    created_at: config.genesis_time,
                    registered_by: None,
                },
            );
        }
        let sealer = validators
            .get(&config.block_sealer)
            .ok_or_else(|| GenesisError::UnknownSealer(config.block_sealer.clone()))?;
        let main = BranchRecord {
            branch: OntologyBranch {
                branch_id: BranchId::main(),
                name: "main".into(),
                parent_branch: None,
                fork_event: None,
                created_by: sealer.identity_id,
            },
            head: None,
            event_count: 0,
            state: BranchState::default(),
        };
        Ok(OntologyState {
            network: NetworkInfo {
                name: config.network.name.clone(),
                protocol_version: config.network.protocol_version.clone(),
                principles: config.principles.clone(),
                canonicalization: config.canonicalization.clone(),
                block_sealer: config.block_sealer.clone(),
            },
            identities,
            validators,
            branches: [(BranchId::main(), main)].into_iter().collect(),
            artifacts: BTreeMap::new(),
            simulation_jobs: BTreeMap::new(),
            simulation_results: BTreeMap::new(),
            research_requests: BTreeMap::new(),
            research_results: BTreeMap::new(),
            xchange_assets: BTreeMap::new(),
            merge_proposals: BTreeMap::new(),
            protocol_credit: BTreeMap::new(),
            events_applied: 0,
        })
    }

    /// BLAKE3 of the canonical serialization of the whole state.
    pub fn state_root(&self) -> StateRoot {
        StateRoot(
            self.canonical_hash()
                .expect("ontology state contains only canonical types"),
        )
    }

    pub fn branch(&self, id: &BranchId) -> Option<&BranchRecord> {
        self.branches.get(id)
    }

    /// Transitive dependencies of a claim, breadth first with ties broken by
    /// claim id, so the order is deterministic.
    pub fn dependency_closure(&self, branch: &BranchId, claim: &ClaimId) -> Vec<ClaimId> {
        let Some(b) = self.branches.get(branch) else {
            return vec![];
        };
        let mut seen = BTreeSet::new();
        let mut out = vec![];
        let mut queue = VecDeque::from([claim.clone()]);
        while let Some(c) = queue.pop_front() {
            let Some(rec) = b.state.claims.get(&c) else {
                continue;
            };
            let mut deps = rec.current.dependencies.clone();
            deps.sort();
            for d in deps {
                if seen.insert(d.clone()) {
                    out.push(d.clone());
                    queue.push_back(d);
                }
            }
        }
        out
    }

    /// Claims on a branch whose current status was set by an accepted
    /// canonicalization of their current version, ordered by claim id.
    pub fn canonical_claims(&self, branch: &BranchId) -> Vec<&Claim> {
        let Some(b) = self.branches.get(branch) else {
            return vec![];
        };
        let ids: BTreeSet<&ClaimId> = b
            .state
            .proposals
            .values()
            .filter(|p| p.state == ProposalState::Accepted)
            .map(|p| &p.proposal.target_claim)
            .collect();
        ids.into_iter()
            .filter_map(|id| b.state.claims.get(id))
            .filter(|c| {
                c.superseded_by.is_none()
                    && c.status_history.last().is_some_and(|h| {
                        h.version == c.current.version && h.reason.starts_with("canonicalization")
                    })
            })
            .map(|c| &c.current)
            .collect()
    }
}
