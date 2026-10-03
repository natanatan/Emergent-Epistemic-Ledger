//! End-to-end scenario covering the specification's success criteria, and the
//! mandatory three-node integration test.

use eel_core::event::*;
use eel_core::ontology::*;
use eel_core::research::*;
use eel_core::validation::*;
use eel_core::*;
use eel_ledger::Ledger;
use eel_tests::*;

const R1: u8 = 10; // researcher 1
const R2: u8 = 11; // researcher 2
const REP1: u8 = 12; // independent replicator
const REP2: u8 = 13; // independent replicator

fn p<T: std::str::FromStr>(s: &str) -> T
where
    T::Err: std::fmt::Debug,
{
    s.parse().unwrap()
}

struct Scenario {
    net: Net,
    sim_result: eel_core::simulation::SimulationResult,
}

/// Builds the full scenario on one node, sealing after each step.
fn scenario() -> Scenario {
    let mut net = Net::new();

    // 1. Two researchers (and two replicators) generate independent identities.
    for k in [R1, R2, REP1, REP2] {
        net.register(&key(k));
    }
    net.seal();

    // 2. One creates claims.
    net.submit(
        &key(R1),
        "main",
        create(claim("EE-REL-0001", &[], "ontology")),
    );
    net.submit(
        &key(R1),
        "main",
        create(claim("EE-REL-0002", &["EE-REL-0001"], "ontology")),
    );
    net.submit(
        &key(R1),
        "main",
        create(claim(
            "EE-REL-0003",
            &["EE-REL-0001", "EE-REL-0002"],
            "ontology",
        )),
    );
    net.seal();

    // 3. Another submits an objection.
    net.submit(
        &key(R2),
        "main",
        EventPayload::ObjectionAdd(Objection {
            objection_id: p("OBJ-0001"),
            target: ObjectId::new(ObjectKind::Claim, "EE-REL-0003"),
            severity: ObjectionSeverity::Critical,
            text: "Persistence is presupposed, not derived.".into(),
        }),
    );
    net.seal();

    // 4. The claim is revised without destroying history.
    let mut v2 = claim("EE-REL-0003", &["EE-REL-0001", "EE-REL-0002"], "ontology");
    v2.version = 2;
    v2.statement = "Persistence is the retention of distinctions across relational updates.".into();
    net.submit(
        &key(R1),
        "main",
        EventPayload::ClaimRevise(ClaimRevise {
            claim: v2,
            relations: vec![],
            rationale: Some("answers OBJ-0001".into()),
        }),
    );
    net.submit(
        &key(R1),
        "main",
        EventPayload::ObjectionReply(ObjectionReply {
            objection_id: p("OBJ-0001"),
            text: "Revised in v2 to derive persistence.".into(),
            resolves: false,
        }),
    );
    net.seal();

    // 5. Both create competing branches.
    for (k, name) in [(R1, "relational-emergence"), (R2, "alternative-model")] {
        let head = net.state().branch(&BranchId::main()).unwrap().head;
        net.submit(
            &key(k),
            "main",
            EventPayload::BranchCreate(OntologyBranch {
                branch_id: p(name),
                name: name.into(),
                parent_branch: Some(BranchId::main()),
                fork_event: head,
                created_by: key(k).identity_id(),
            }),
        );
    }
    net.submit(
        &key(R2),
        "alternative-model",
        create(claim("EE-ALT-0001", &["EE-REL-0001"], "ontology")),
    );
    net.submit(
        &key(R1),
        "relational-emergence",
        EventPayload::ObjectionAdd(Objection {
            objection_id: p("OBJ-B1"),
            target: ObjectId::new(ObjectKind::Claim, "EE-REL-0002"),
            severity: ObjectionSeverity::Minor,
            text: "only on this branch".into(),
        }),
    );
    net.seal();

    // 6. Research request and simulated support commitments.
    net.submit(
        &key(R1),
        "main",
        EventPayload::ResearchRequestCreate(ResearchRequest {
            request_id: p("REQ-0001"),
            linked_claims: vec![p("EE-REL-0003")],
            title: "Persistent relational structure search".into(),
            description: "Search deterministic graph-rule families for persistent structures."
                .into(),
            requested_work: vec![ResearchWorkType::Simulation, ResearchWorkType::Replication],
            acceptance_conditions: vec!["result is deterministically reproducible".into()],
            created_by: key(R1).identity_id(),
        }),
    );
    for (k, units, kind) in [
        (R2, 50u64, CommitmentType::Execution),
        (REP1, 30, CommitmentType::Replication),
    ] {
        net.submit(
            &key(k),
            "main",
            EventPayload::ResearchRequestSupport(SupportCommitment {
                supporter: key(k).identity_id(),
                request_id: p("REQ-0001"),
                amount_units: units,
                commitment_type: kind,
            }),
        );
    }
    net.seal();

    // 7. A deterministic simulation produces a result.
    let def = sim_define("SIM-0001", &["EE-REL-0003"]);
    net.submit(
        &key(R1),
        "main",
        EventPayload::SimulationDefine(def.clone()),
    );
    let sim_result = run_and_submit(&mut net, &key(R1), &def, 4);
    net.seal();

    // 8. Independent validators replay it.
    verify(&mut net, "VAL-A", 1, &sim_result, true).unwrap();
    verify(&mut net, "VAL-B", 2, &sim_result, true).unwrap();
    net.seal();

    // 9. The result becomes evidence, and researchers submit results with
    //    opposite directions; both are credited once validated.
    net.submit(
        &key(R1),
        "main",
        EventPayload::EvidenceAdd(Evidence {
            simulation_result: Some(sim_result.result_id),
            ..evidence("EV-SIM-0001", "EE-REL-0003")
        }),
    );
    let supporting = net.submit(
        &key(R1),
        "main",
        EventPayload::ResearchResultSubmit(ResearchResult {
            request_id: p("REQ-0001"),
            submitted_by: key(R1).identity_id(),
            outcome: ResearchOutcome::Supporting,
            artifacts: vec![sim_result.result_artifact],
            methodology: "seed 4 of SIM-0001".into(),
            linked_claims: vec![p("EE-REL-0003")],
        }),
    );
    let contradicting = net.submit(
        &key(R2),
        "main",
        EventPayload::ResearchResultSubmit(ResearchResult {
            request_id: p("REQ-0001"),
            submitted_by: key(R2).identity_id(),
            outcome: ResearchOutcome::Contradicting,
            artifacts: vec![],
            methodology: "countermodel search".into(),
            linked_claims: vec![p("EE-REL-0003")],
        }),
    );
    for (i, rr) in [supporting, contradicting].iter().enumerate() {
        for (v, k) in [("VAL-A", 1u8), ("VAL-B", 2)] {
            net.submit(
                &key(k),
                "main",
                EventPayload::ValidationSubmit(ValidationRecord {
                    validation_id: p(&format!("VAL-RR-{i}-{v}")),
                    validator: vid(v),
                    target: ObjectId::new(ObjectKind::ResearchResult, rr),
                    validation_type: ValidationType::MethodConformance,
                    outcome: ValidationOutcome::Pass,
                    notes: None,
                }),
            );
        }
    }
    net.seal();

    // 10. Independent replications, objection resolution, canonicalization.
    for (k, id) in [(REP1, "REP-0001"), (REP2, "REP-0002")] {
        net.submit(
            &key(k),
            "main",
            EventPayload::ReplicationRegister(Replication {
                replication_id: p(id),
                target: ObjectId::new(ObjectKind::SimulationResult, sim_result.result_id),
                protocol: "independent replay".into(),
            }),
        );
        net.submit(
            &key(k),
            "main",
            EventPayload::ReplicationResult(ReplicationResultPayload {
                replication_id: p(id),
                outcome: ReplicationOutcome::Success,
                artifacts: vec![],
                notes: None,
            }),
        );
    }
    net.submit(
        &key(R2),
        "main",
        EventPayload::ObjectionReply(ObjectionReply {
            objection_id: p("OBJ-0001"),
            text: "v2 answers it.".into(),
            resolves: true,
        }),
    );
    net.submit(
        &key(R1),
        "main",
        EventPayload::CanonicalizationPropose(CanonicalizationProposal {
            proposal_id: p("CAN-0001"),
            target_claim: p("EE-REL-0003"),
            target_version: 2,
            proposed_status: ClaimStatus::Supported,
            evidence: vec![p("EV-SIM-0001")],
            validations: vec![],
            successful_replications: vec![p("REP-0001"), p("REP-0002")],
            unresolved_objections: vec![],
        }),
    );
    net.submit(
        &key(1),
        "main",
        EventPayload::CanonicalizationAccept(CanonicalizationDecision {
            proposal_id: p("CAN-0001"),
            validator: vid("VAL-A"),
            reason: None,
        }),
    );
    net.seal();
    net.submit(
        &key(2),
        "main",
        EventPayload::CanonicalizationAccept(CanonicalizationDecision {
            proposal_id: p("CAN-0001"),
            validator: vid("VAL-B"),
            reason: None,
        }),
    );
    net.seal();

    Scenario { net, sim_result }
}

#[test]
fn success_criteria_scenario() {
    let Scenario { net, sim_result } = scenario();
    let st = net.state();
    let main = &st.branches[&BranchId::main()].state;

    // Claim revised without destroying history.
    let c3 = &main.claims[&p("EE-REL-0003")];
    assert_eq!(c3.versions.len(), 2);
    assert_eq!(c3.versions[0].claim.version, 1);
    assert_eq!(c3.current.version, 2);

    // Objection recorded and resolved by its own author.
    assert!(main.objections[&p("OBJ-0001")].resolved);

    // Competing branches with common ancestry and isolation.
    let alt = &st.branches[&p::<BranchId>("alternative-model")];
    let rel = &st.branches[&p::<BranchId>("relational-emergence")];
    assert_eq!(alt.branch.parent_branch, Some(BranchId::main()));
    assert!(
        alt.state.claims.contains_key(&p("EE-REL-0001")),
        "inherits ancestry"
    );
    assert!(alt.state.claims.contains_key(&p("EE-ALT-0001")));
    assert!(
        !main.claims.contains_key(&p("EE-ALT-0001")),
        "isolated from main"
    );
    assert!(
        !rel.state.claims.contains_key(&p("EE-ALT-0001")),
        "isolated from sibling"
    );
    assert!(rel.state.objections.contains_key(&p("OBJ-B1")));
    assert!(!main.objections.contains_key(&p("OBJ-B1")));

    // Support commitments in development units.
    let req = &st.research_requests[&p("REQ-0001")];
    let totals = eel_xchange::pool_totals(&req.commitments);
    assert_eq!((totals.execution, totals.replication), (50, 30));

    // Simulation replayed by two validators.
    assert_eq!(
        st.simulation_results[&sim_result.result_id].successful_replays(),
        2
    );

    // Researchers credited regardless of outcome direction.
    assert_eq!(st.protocol_credit.get(&key(R1).identity_id()), Some(&1));
    assert_eq!(st.protocol_credit.get(&key(R2).identity_id()), Some(&1));

    // Canonicalization only after two validators, two independent replications
    // and no unresolved critical objection.
    assert_eq!(
        main.proposals[&p("CAN-0001")].state,
        ProposalState::Accepted
    );
    assert_eq!(c3.current.status, ClaimStatus::Supported);
    assert_eq!(st.canonical_claims(&BranchId::main()).len(), 1);

    // Everything remains queryable: every event is retrievable by id.
    for (id, e) in net.ledger.events() {
        assert_eq!(net.ledger.event(id), Some(e));
    }
}

#[test]
fn canonicalization_waits_for_second_validator() {
    // After VAL-A alone the proposal must still be open.
    let Scenario { net, .. } = scenario();
    let blocks = net.ledger.blocks();
    let before_last = net
        .ledger
        .state_at(blocks[blocks.len() - 2].block.height)
        .unwrap();
    let prop = &before_last.branches[&BranchId::main()].state.proposals[&p("CAN-0001")];
    assert_eq!(prop.state, ProposalState::Open);
    assert_eq!(prop.accepts.len(), 1);
}

#[test]
fn branch_state_reconstructs_independently() {
    let Scenario { net, .. } = scenario();
    for b in ["main", "alternative-model", "relational-emergence"] {
        let id: BranchId = p(b);
        let rebuilt = net.ledger.reconstruct_branch(&id).unwrap();
        assert_eq!(
            rebuilt.branches[&id].state,
            net.state().branches[&id].state,
            "branch {b}"
        );
    }
    // The alternative branch's view excludes the sibling's branch-scoped events.
    let view = net.ledger.branch_view(&p("alternative-model")).unwrap();
    assert!(!view
        .iter()
        .any(|e| e.branch.as_str() == "relational-emergence"));
}

#[test]
fn dependency_traversal_is_deterministic() {
    let Scenario { net, .. } = scenario();
    let deps = net
        .state()
        .dependency_closure(&BranchId::main(), &p("EE-REL-0003"));
    assert_eq!(deps, vec![p::<ClaimId>("EE-REL-0001"), p("EE-REL-0002")]);
    for _ in 0..5 {
        assert_eq!(
            net.state()
                .dependency_closure(&BranchId::main(), &p("EE-REL-0003")),
            deps
        );
    }
}

/// Mandatory: three independent nodes from identical genesis, same event
/// stream, identical block hashes, state roots, graph, claim versions and
/// simulation verification results.
#[test]
fn three_nodes_derive_identical_state() {
    let Scenario { net: origin, .. } = scenario();

    // Node A and B re-execute the stream and seal locally at the same points;
    // node C follows by importing A's blocks and verifying them.
    let mut nodes: Vec<Ledger> = (0..2)
        .map(|_| Ledger::new(dev_genesis(), &key(1)).unwrap())
        .collect();
    for block in &origin.ledger.blocks()[1..] {
        for node in nodes.iter_mut() {
            for id in &block.event_ids {
                node.submit(origin.ledger.event(id).unwrap().clone())
                    .unwrap();
            }
            node.seal(&key(1)).unwrap();
        }
    }
    let mut follower =
        Ledger::from_genesis_block(dev_genesis(), origin.ledger.blocks()[0].clone()).unwrap();
    for block in &origin.ledger.blocks()[1..] {
        let events = block
            .event_ids
            .iter()
            .map(|id| origin.ledger.event(id).unwrap().clone())
            .collect();
        follower.import_block(block.clone(), events).unwrap();
    }
    nodes.push(follower);

    let expected_hashes: Vec<_> = origin
        .ledger
        .blocks()
        .iter()
        .map(|b| b.block_hash)
        .collect();
    let o = origin.ledger.state();
    for (i, n) in nodes.iter().enumerate() {
        let s = n.state();
        let hashes: Vec<_> = n.blocks().iter().map(|b| b.block_hash).collect();
        assert_eq!(hashes, expected_hashes, "node {i} block hashes");
        assert_eq!(s.state_root(), o.state_root(), "node {i} state root");
        for (bid, b) in &o.branches {
            assert_eq!(
                s.branches[bid].state.edges, b.state.edges,
                "node {i} graph on {bid}"
            );
            for (cid, c) in &b.state.claims {
                assert_eq!(
                    s.branches[bid].state.claims[cid].versions, c.versions,
                    "node {i} versions of {cid}"
                );
            }
        }
        for (rid, r) in &o.simulation_results {
            assert_eq!(
                s.simulation_results[rid].verifications, r.verifications,
                "node {i} sim verification"
            );
        }
    }
}

#[tokio::test]
async fn sqlite_roundtrip_reverifies_everything() {
    let Scenario { net, .. } = scenario();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("eel.db");
    {
        let store = eel_storage::Store::open(&path).await.unwrap();
        // Rebuild a ledger block by block so the store sees each block once.
        let mut l = Ledger::new(dev_genesis(), &key(1)).unwrap();
        store.initialize(&l).await.unwrap();
        for block in &net.ledger.blocks()[1..] {
            for id in &block.event_ids {
                l.submit(net.ledger.event(id).unwrap().clone()).unwrap();
            }
            let rec = l.seal(&key(1)).unwrap().unwrap().clone();
            store.append_block(&l, &rec).await.unwrap();
        }
    }
    let store = eel_storage::Store::open(&path).await.unwrap();
    let loaded = store.load().await.unwrap();
    assert_eq!(loaded.state().state_root(), net.state().state_root());
    assert_eq!(loaded.head().block_hash, net.ledger.head().block_hash);

    // Projection tables are populated.
    use sqlx::Row;
    let n: i64 = sqlx::query("SELECT COUNT(*) FROM claim_versions")
        .fetch_one(store.pool())
        .await
        .unwrap()
        .get(0);
    assert!(n >= 4);
}

#[tokio::test]
async fn persistent_node_seals_each_event() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("node.db");
    let mut node = eel_storage::PersistentNode::open(&path, Some(dev_genesis()), Some(key(1)))
        .await
        .unwrap();
    let (_, e) = key(R1).identity_register_event(Timestamp(5_000)).unwrap();
    node.submit(e).await.unwrap();
    let (_, e) = key(R1)
        .author_event(
            Timestamp(5_001),
            BranchId::main(),
            vec![],
            create(claim("EE-REL-0001", &[], "ontology")),
        )
        .unwrap();
    node.submit(e).await.unwrap();
    // A rejected event leaves storage untouched.
    let (_, bad) = key(R2)
        .author_event(
            Timestamp(5_002),
            BranchId::main(),
            vec![],
            create(claim("EE-REL-0002", &[], "ontology")),
        )
        .unwrap();
    assert!(node.submit(bad).await.is_err());
    let root = node.ledger.state().state_root();
    drop(node);
    let reopened = eel_storage::PersistentNode::open(&path, None, None)
        .await
        .unwrap();
    assert_eq!(reopened.ledger.blocks().len(), 3);
    assert_eq!(reopened.ledger.state().state_root(), root);
}
