//! Shared helpers for the EEL test suites: a development genesis with three
//! validators, deterministic keys, and an event-authoring harness.

use eel_core::event::*;
use eel_core::genesis::*;
use eel_core::ontology::*;
use eel_core::simulation::*;
use eel_core::validation::*;
use eel_core::*;
use eel_crypto::KeyPair;
use eel_ledger::Ledger;

pub fn key(n: u8) -> KeyPair {
    KeyPair::from_seed([n; 32])
}

pub fn vid(s: &str) -> ValidatorId {
    s.parse().unwrap()
}

/// VAL-A and VAL-B cover `ontology` and `physics`; VAL-C covers only
/// `physics`. VAL-A seals blocks. Researchers use keys 10 and up.
pub fn dev_genesis() -> GenesisConfig {
    let v = |id: &str, k: u8, domains: &[&str]| GenesisValidator {
        validator_id: vid(id),
        public_key: key(k).public_key_hex(),
        domains: domains.iter().map(|d| d.to_string()).collect(),
        validator_class: ValidatorClass::Genesis,
        activated_at: Timestamp(0),
        expires_at: None,
    };
    GenesisConfig {
        network: NetworkConfig {
            name: "EEL Development Network".into(),
            protocol_version: "0.1".into(),
        },
        principles: REQUIRED_PRINCIPLES.iter().map(|s| s.to_string()).collect(),
        genesis_time: Timestamp(1_000),
        validators: vec![
            v("VAL-A", 1, &["ontology", "physics"]),
            v("VAL-B", 2, &["ontology", "physics"]),
            v("VAL-C", 3, &["physics"]),
        ],
        canonicalization: CanonicalizationRule::default(),
        block_sealer: vid("VAL-A"),
    }
}

/// A ledger plus a deterministic clock for authoring events.
pub struct Net {
    pub ledger: Ledger,
    pub sealer: KeyPair,
    clock: i64,
}

impl Default for Net {
    fn default() -> Self {
        Self::new()
    }
}

impl Net {
    pub fn new() -> Self {
        Net {
            ledger: Ledger::new(dev_genesis(), &key(1)).unwrap(),
            sealer: key(1),
            clock: 2_000,
        }
    }

    /// Authors and signs an event (parent: the branch head) without submitting it.
    pub fn author(&mut self, who: &KeyPair, branch: &str, payload: EventPayload) -> EpistemicEvent {
        self.clock += 1;
        let branch: BranchId = branch.parse().unwrap();
        let parents = self
            .ledger
            .state()
            .branch(&branch)
            .and_then(|b| b.head)
            .into_iter()
            .collect();
        who.author_event(Timestamp(self.clock), branch, parents, payload)
            .unwrap()
            .1
    }

    pub fn try_submit(
        &mut self,
        who: &KeyPair,
        branch: &str,
        payload: EventPayload,
    ) -> eel_ledger::Result<EventId> {
        let e = self.author(who, branch, payload);
        self.ledger.submit(e)
    }

    pub fn submit(&mut self, who: &KeyPair, branch: &str, payload: EventPayload) -> EventId {
        self.try_submit(who, branch, payload).unwrap()
    }

    pub fn register(&mut self, who: &KeyPair) -> EventId {
        self.clock += 1;
        let (_, e) = who.identity_register_event(Timestamp(self.clock)).unwrap();
        self.ledger.submit(e).unwrap()
    }

    pub fn seal(&mut self) {
        self.ledger.seal(&self.sealer).unwrap();
    }

    pub fn state(&self) -> &eel_ontology::OntologyState {
        self.ledger.state()
    }

    pub fn claim(&self, branch: &str, id: &str) -> &eel_ontology::ClaimRecord {
        &self.state().branches[&branch.parse::<BranchId>().unwrap()]
            .state
            .claims[&id.parse::<ClaimId>().unwrap()]
    }
}

pub fn claim(id: &str, deps: &[&str], domain: &str) -> Claim {
    Claim {
        claim_id: id.parse().unwrap(),
        version: 1,
        title: format!("Title of {id}"),
        statement: format!("Statement of {id}"),
        claim_type: vec![ClaimType::Proposition],
        ontology_level: "relational".into(),
        status: ClaimStatus::Proposed,
        dependencies: deps.iter().map(|d| d.parse().unwrap()).collect(),
        ontological_defensibility: None,
        domain: domain.into(),
    }
}

pub fn create(c: Claim) -> EventPayload {
    EventPayload::ClaimCreate(ClaimCreate {
        claim: c,
        relations: vec![],
    })
}

pub fn evidence(id: &str, claim: &str) -> Evidence {
    Evidence {
        evidence_id: id.parse().unwrap(),
        claim_id: claim.parse().unwrap(),
        relation: EvidenceRelation::Supports,
        description: "test evidence".into(),
        artifacts: vec![],
        simulation_result: None,
        research_result: None,
    }
}

pub fn ruleset() -> Ruleset {
    Ruleset {
        coupling_divisor: 8,
        self_retention_permille: 900,
        perturbation_permille: 50,
        perturbation_magnitude: 200,
        state_bound: 1000,
        persistence_threshold: 300,
        memory_length: 5,
    }
}

pub fn initial_conditions() -> InitialConditions {
    InitialConditions {
        vertices: 24,
        edge_permille: 120,
        weight_bound: 4,
        initial_state_bound: 1000,
    }
}

pub fn sim_define(job: &str, claims: &[&str]) -> SimulationDefine {
    let r = ruleset();
    let ic = initial_conditions();
    SimulationDefine {
        job: eel_simulation::make_job(
            job.parse().unwrap(),
            claims.iter().map(|c| c.parse().unwrap()).collect(),
            &r,
            &ic,
            1,
            8,
            120,
            eel_simulation::all_metrics(),
        )
        .unwrap(),
        ruleset: r,
        initial_conditions: ic,
    }
}

/// Artifact record for some bytes, authored by `who`.
pub fn artifact(who: &KeyPair, bytes: &[u8]) -> eel_core::xchange::Artifact {
    let id = ArtifactId(Hash::digest(bytes));
    eel_core::xchange::Artifact {
        artifact_id: id,
        content_hash: format!("blake3:{id}"),
        uri: format!("file:///artifacts/{id}"),
        media_type: "application/json".into(),
        size_bytes: bytes.len() as u64,
        author: who.identity_id(),
        license: None,
        external_hashes: vec![],
    }
}

/// Runs a seed of a job as `worker`, registering the artifact and submitting
/// the result. Returns the result.
pub fn run_and_submit(
    net: &mut Net,
    worker: &KeyPair,
    def: &SimulationDefine,
    seed: u64,
) -> SimulationResult {
    let engine = eel_simulation::RelationalGraphEngine::new(
        def.job.clone(),
        def.ruleset.clone(),
        def.initial_conditions.clone(),
        worker.identity_id(),
    )
    .unwrap();
    let (result, bytes) = engine.execute_seed(seed).unwrap();
    net.submit(
        worker,
        "main",
        EventPayload::ArtifactRegister(artifact(worker, &bytes)),
    );
    net.submit(
        worker,
        "main",
        EventPayload::SimulationResult(result.clone()),
    );
    result
}

pub fn verify(
    net: &mut Net,
    validator: &str,
    k: u8,
    result: &SimulationResult,
    reproduced: bool,
) -> eel_ledger::Result<EventId> {
    net.try_submit(
        &key(k),
        "main",
        EventPayload::SimulationVerify(SimulationVerify {
            result_id: result.result_id,
            validator: vid(validator),
            reproduced,
        }),
    )
}
