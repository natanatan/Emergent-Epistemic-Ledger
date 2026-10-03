//! Attacks from the threat model, each expected to fail.

use eel_core::event::*;
use eel_core::ontology::*;
use eel_core::validation::*;
use eel_core::*;
use eel_tests::*;

fn base() -> Net {
    let mut net = Net::new();
    for k in 10..=14 {
        net.register(&key(k));
    }
    net.submit(
        &key(10),
        "main",
        create(claim("EE-REL-0001", &[], "ontology")),
    );
    net.submit(
        &key(10),
        "main",
        EventPayload::EvidenceAdd(evidence("EV-1", "EE-REL-0001")),
    );
    net
}

fn validation(id: &str, validator: &str, target: ObjectId) -> EventPayload {
    EventPayload::ValidationSubmit(ValidationRecord {
        validation_id: id.parse().unwrap(),
        validator: vid(validator),
        target,
        validation_type: ValidationType::MethodConformance,
        outcome: ValidationOutcome::Pass,
        notes: None,
    })
}

fn ev(id: &str) -> ObjectId {
    ObjectId::new(ObjectKind::Evidence, id)
}

#[test]
fn sybil_identities_cannot_validate() {
    let mut net = base();
    // Many fresh identities claiming to be a validator are rejected.
    for k in 20..25 {
        net.register(&key(k));
        assert!(net
            .try_submit(
                &key(k),
                "main",
                validation(&format!("V-{k}"), "VAL-A", ev("EV-1"))
            )
            .is_err());
    }
    // A made-up validator id is rejected too.
    assert!(net
        .try_submit(&key(20), "main", validation("V-X", "VAL-Z", ev("EV-1")))
        .is_err());
}

#[test]
fn unregistered_author_rejected() {
    let mut net = base();
    assert!(net
        .try_submit(
            &key(99),
            "main",
            create(claim("EE-REL-0009", &[], "ontology"))
        )
        .is_err());
}

#[test]
fn forged_signature_rejected() {
    let mut net = base();
    let mut e = net.author(
        &key(10),
        "main",
        create(claim("EE-REL-0002", &[], "ontology")),
    );
    e.author = key(11).identity_id(); // claim someone else's authorship
    assert!(net.ledger.submit(e).is_err());
}

#[test]
fn replayed_event_rejected() {
    let mut net = base();
    let e = net.author(
        &key(10),
        "main",
        create(claim("EE-REL-0002", &[], "ontology")),
    );
    net.ledger.submit(e.clone()).unwrap();
    assert!(matches!(
        net.ledger.submit(e),
        Err(eel_ledger::LedgerError::Duplicate(_))
    ));
}

#[test]
fn unknown_parent_rejected() {
    let mut net = base();
    let (_, e) = key(10)
        .author_event(
            Timestamp(9_999),
            BranchId::main(),
            vec![EventId(Hash::digest(b"nope"))],
            create(claim("EE-REL-0002", &[], "ontology")),
        )
        .unwrap();
    assert!(matches!(
        net.ledger.submit(e),
        Err(eel_ledger::LedgerError::UnknownParent(_))
    ));
}

#[test]
fn validator_outside_domain_rejected_and_inside_accepted() {
    let mut net = base();
    // VAL-C has only `physics`; the claim is in `ontology`.
    assert!(net
        .try_submit(&key(3), "main", validation("V-1", "VAL-C", ev("EV-1")))
        .is_err());
    net.submit(&key(1), "main", validation("V-2", "VAL-A", ev("EV-1")));
}

#[test]
fn validator_cannot_impersonate_another() {
    let mut net = base();
    assert!(net
        .try_submit(&key(2), "main", validation("V-1", "VAL-A", ev("EV-1")))
        .is_err());
}

#[test]
fn validation_never_sets_supported() {
    let mut net = base();
    net.submit(
        &key(1),
        "main",
        validation(
            "V-1",
            "VAL-A",
            ObjectId::new(ObjectKind::Claim, "EE-REL-0001"),
        ),
    );
    net.submit(&key(2), "main", validation("V-2", "VAL-B", ev("EV-1")));
    assert_eq!(
        net.claim("main", "EE-REL-0001").current.status,
        ClaimStatus::Proposed
    );
}

#[test]
fn only_author_revises_claim() {
    let mut net = base();
    let mut c = claim("EE-REL-0001", &[], "ontology");
    c.version = 2;
    c.statement = "hijacked".into();
    let p = EventPayload::ClaimRevise(ClaimRevise {
        claim: c,
        relations: vec![],
        rationale: None,
    });
    assert!(net.try_submit(&key(11), "main", p).is_err());
}

#[test]
fn claim_author_cannot_resolve_someone_elses_objection() {
    let mut net = base();
    net.submit(
        &key(11),
        "main",
        EventPayload::ObjectionAdd(Objection {
            objection_id: "OBJ-1".parse().unwrap(),
            target: ObjectId::new(ObjectKind::Claim, "EE-REL-0001"),
            severity: ObjectionSeverity::Critical,
            text: "circular".into(),
        }),
    );
    let reply = |resolves| {
        EventPayload::ObjectionReply(ObjectionReply {
            objection_id: "OBJ-1".parse().unwrap(),
            text: "answered".into(),
            resolves,
        })
    };
    assert!(net.try_submit(&key(10), "main", reply(true)).is_err());
    net.submit(&key(10), "main", reply(false));
    net.submit(&key(11), "main", reply(true));
}

#[test]
fn history_cannot_be_deleted() {
    // There is no event type that removes anything; revisions and supersession
    // keep every prior version queryable.
    let mut net = base();
    net.submit(
        &key(10),
        "main",
        create(claim("EE-REL-0002", &[], "ontology")),
    );
    net.submit(
        &key(10),
        "main",
        EventPayload::ClaimSupersede(ClaimSupersede {
            claim_id: "EE-REL-0001".parse().unwrap(),
            superseded_by: "EE-REL-0002".parse().unwrap(),
            rationale: None,
        }),
    );
    let old = net.claim("main", "EE-REL-0001");
    assert_eq!(old.current.status, ClaimStatus::Superseded);
    assert_eq!(old.versions.len(), 1);
    assert!(net.state().branches[&BranchId::main()]
        .state
        .evidence
        .contains_key(&"EV-1".parse().unwrap()));
}

#[test]
fn worker_cannot_verify_own_result_and_false_verification_rejected() {
    let mut net = base();
    let def = sim_define("SIM-1", &["EE-REL-0001"]);
    net.submit(
        &key(10),
        "main",
        EventPayload::SimulationDefine(def.clone()),
    );
    // VAL-A's own key acting as worker.
    let r = run_and_submit(&mut net, &key(1), &def, 2);
    assert!(
        verify(&mut net, "VAL-A", 1, &r, true).is_err(),
        "self-verification"
    );
    assert!(
        verify(&mut net, "VAL-B", 2, &r, false).is_err(),
        "misreported replay"
    );
    verify(&mut net, "VAL-B", 2, &r, true).unwrap();
    assert!(
        verify(&mut net, "VAL-B", 2, &r, true).is_err(),
        "double verification"
    );
}

#[test]
fn fabricated_simulation_result_is_recorded_as_not_reproduced() {
    let mut net = base();
    let def = sim_define("SIM-1", &["EE-REL-0001"]);
    net.submit(
        &key(10),
        "main",
        EventPayload::SimulationDefine(def.clone()),
    );
    let engine = eel_simulation::RelationalGraphEngine::new(
        def.job.clone(),
        def.ruleset.clone(),
        def.initial_conditions.clone(),
        key(12).identity_id(),
    )
    .unwrap();
    let (mut r, bytes) = engine.execute_seed(3).unwrap();
    r.metrics[0].value += 100;
    r.result_id = r.compute_id().unwrap();
    net.submit(
        &key(12),
        "main",
        EventPayload::ArtifactRegister(artifact(&key(12), &bytes)),
    );
    net.submit(&key(12), "main", EventPayload::SimulationResult(r.clone()));
    assert!(verify(&mut net, "VAL-A", 1, &r, true).is_err());
    verify(&mut net, "VAL-A", 1, &r, false).unwrap();
    assert_eq!(
        net.state().simulation_results[&r.result_id].successful_replays(),
        0
    );
}

#[test]
fn artifact_mutation_is_detected() {
    let bytes = b"dataset v1".to_vec();
    let a = artifact(&key(10), &bytes);
    let mut mutated = bytes.clone();
    mutated[0] ^= 1;
    assert_ne!(ArtifactId(Hash::digest(&mutated)), a.artifact_id);
    // A registration whose content hash does not match its id is rejected.
    let mut net = base();
    let mut bad = a.clone();
    bad.content_hash = format!("blake3:{}", Hash::digest(&mutated));
    assert!(net
        .try_submit(&key(10), "main", EventPayload::ArtifactRegister(bad))
        .is_err());
}

#[test]
fn replications_by_claim_author_are_not_independent() {
    let mut net = base();
    for (k, id) in [(10u8, "REP-1"), (11, "REP-2")] {
        net.submit(
            &key(k),
            "main",
            EventPayload::ReplicationRegister(Replication {
                replication_id: id.parse().unwrap(),
                target: ev("EV-1"),
                protocol: "rerun".into(),
            }),
        );
        net.submit(
            &key(k),
            "main",
            EventPayload::ReplicationResult(ReplicationResultPayload {
                replication_id: id.parse().unwrap(),
                outcome: ReplicationOutcome::Success,
                artifacts: vec![],
                notes: None,
            }),
        );
    }
    net.submit(
        &key(10),
        "main",
        EventPayload::CanonicalizationPropose(CanonicalizationProposal {
            proposal_id: "CAN-1".parse().unwrap(),
            target_claim: "EE-REL-0001".parse().unwrap(),
            target_version: 1,
            proposed_status: ClaimStatus::Supported,
            evidence: vec!["EV-1".parse().unwrap()],
            validations: vec![],
            successful_replications: vec!["REP-1".parse().unwrap(), "REP-2".parse().unwrap()],
            unresolved_objections: vec![],
        }),
    );
    for (v, k) in [("VAL-A", 1u8), ("VAL-B", 2)] {
        net.submit(
            &key(k),
            "main",
            EventPayload::CanonicalizationAccept(CanonicalizationDecision {
                proposal_id: "CAN-1".parse().unwrap(),
                validator: vid(v),
                reason: None,
            }),
        );
    }
    let p = &net.state().branches[&BranchId::main()].state.proposals[&"CAN-1".parse().unwrap()];
    assert_eq!(p.state, ProposalState::Open);
    assert!(
        p.blockers
            .iter()
            .any(|b| b.contains("1 of 2 required independent replications")),
        "{:?}",
        p.blockers
    );
    assert_eq!(
        net.claim("main", "EE-REL-0001").current.status,
        ClaimStatus::Proposed
    );
}

#[test]
fn expired_validator_rejected() {
    let mut g = dev_genesis();
    g.validators[1].expires_at = Some(Timestamp(1_500));
    let mut ledger = eel_ledger::Ledger::new(g, &key(1)).unwrap();
    let (_, e) = key(10).identity_register_event(Timestamp(2_000)).unwrap();
    ledger.submit(e).unwrap();
    let (_, e) = key(10)
        .author_event(
            Timestamp(2_001),
            BranchId::main(),
            vec![],
            create(claim("EE-REL-0001", &[], "ontology")),
        )
        .unwrap();
    ledger.submit(e).unwrap();
    let (_, e) = key(2)
        .author_event(
            Timestamp(2_002),
            BranchId::main(),
            vec![],
            validation(
                "V-1",
                "VAL-B",
                ObjectId::new(ObjectKind::Claim, "EE-REL-0001"),
            ),
        )
        .unwrap();
    assert!(ledger.submit(e).is_err());
}

#[test]
fn tampered_block_fails_import() {
    let mut net = base();
    net.seal();
    let mut follower =
        eel_ledger::Ledger::from_genesis_block(dev_genesis(), net.ledger.blocks()[0].clone())
            .unwrap();
    let mut record = net.ledger.blocks()[1].clone();
    let events: Vec<_> = record
        .event_ids
        .iter()
        .map(|id| net.ledger.event(id).unwrap().clone())
        .collect();
    record.block.state_root = StateRoot(Hash::digest(b"forged"));
    assert!(follower.import_block(record, events).is_err());
    assert_eq!(follower.blocks().len(), 1);
}
