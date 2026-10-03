//! Mandatory determinism tests: canonical bytes, hashes, signatures, Merkle
//! roots, state roots, historical reconstruction and simulation replay.

use eel_core::canonical::canonical_value_bytes;
use eel_core::event::*;
use eel_core::*;
use eel_ledger::merkle;
use eel_tests::*;
use serde_json::json;

// ---------------------------------------------------------------- cryptographic

#[test]
fn same_canonical_object_same_hash_byte_for_byte() {
    let a = json!({"statement": "Persistence", "version": 1, "dependencies": ["EE-REL-0001"]});
    let b = json!({"version": 1, "dependencies": ["EE-REL-0001"], "statement": "Persistence"});
    let bytes_a = canonical_value_bytes(&a).unwrap();
    let bytes_b = canonical_value_bytes(&b).unwrap();
    assert_eq!(bytes_a, bytes_b);
    assert_eq!(
        String::from_utf8(bytes_a.clone()).unwrap(),
        r#"{"dependencies":["EE-REL-0001"],"statement":"Persistence","version":1}"#
    );
    assert_eq!(Hash::digest(&bytes_a), Hash::digest(&bytes_b));
}

#[test]
fn event_ids_are_reproducible_byte_for_byte() {
    // The same key, timestamp and payload must always yield the same bytes,
    // id and (Ed25519 is deterministic) signature.
    let make = || {
        key(10)
            .author_event(
                Timestamp(5),
                BranchId::main(),
                vec![],
                create(claim("EE-REL-0001", &[], "ontology")),
            )
            .unwrap()
    };
    let (id1, e1) = make();
    let (id2, e2) = make();
    assert_eq!(id1, id2);
    assert_eq!(
        e1.unsigned().canonical_bytes().unwrap(),
        e2.unsigned().canonical_bytes().unwrap()
    );
    assert_eq!(e1.signature, e2.signature);
}

#[test]
fn mutated_object_different_hash() {
    let (id, e) = key(10)
        .author_event(
            Timestamp(5),
            BranchId::main(),
            vec![],
            create(claim("EE-REL-0001", &[], "ontology")),
        )
        .unwrap();
    let mut m = e.unsigned();
    if let EventPayload::ClaimCreate(c) = &mut m.payload {
        c.claim.statement.push('.');
    }
    assert_ne!(m.event_id().unwrap(), id);
}

#[test]
fn valid_signature_accepted_invalid_rejected() {
    let k = key(10);
    let (_, e) = k.identity_register_event(Timestamp(1)).unwrap();
    assert!(eel_crypto::verify_event(&e, &k.public_key_hex()).is_ok());
    let mut bad = e.clone();
    let mut sig = hex_flip(&bad.signature);
    std::mem::swap(&mut bad.signature, &mut sig);
    assert!(eel_crypto::verify_event(&bad, &k.public_key_hex()).is_err());
}

fn hex_flip(s: &str) -> String {
    let mut c: Vec<char> = s.chars().collect();
    c[0] = if c[0] == '0' { '1' } else { '0' };
    c.into_iter().collect()
}

// ---------------------------------------------------------------------- ledger

fn scripted_net() -> Net {
    let mut net = Net::new();
    net.register(&key(10));
    net.register(&key(11));
    net.seal();
    net.submit(
        &key(10),
        "main",
        create(claim("EE-REL-0001", &[], "ontology")),
    );
    net.submit(
        &key(10),
        "main",
        create(claim("EE-REL-0002", &["EE-REL-0001"], "ontology")),
    );
    net.seal();
    net.submit(
        &key(11),
        "main",
        EventPayload::EvidenceAdd(evidence("EV-1", "EE-REL-0002")),
    );
    net.seal();
    net
}

#[test]
fn same_events_same_merkle_root() {
    let net = scripted_net();
    let ids: Vec<EventId> = net.ledger.events().map(|(id, _)| *id).collect();
    assert_eq!(merkle::root(&ids), merkle::root(&ids.clone()));
    for b in net.ledger.blocks() {
        assert_eq!(merkle::root(&b.event_ids), b.block.events_root);
    }
}

#[test]
fn same_ledger_same_state_root() {
    let a = scripted_net();
    let b = scripted_net();
    assert_eq!(a.state().state_root(), b.state().state_root());
    let ha: Vec<_> = a.ledger.blocks().iter().map(|b| b.block_hash).collect();
    let hb: Vec<_> = b.ledger.blocks().iter().map(|b| b.block_hash).collect();
    assert_eq!(ha, hb);
}

#[test]
fn historical_state_reconstructs_correctly() {
    let net = scripted_net();
    for b in net.ledger.blocks() {
        let s = net.ledger.state_at(b.block.height).unwrap();
        assert_eq!(
            s.state_root(),
            b.block.state_root,
            "height {}",
            b.block.height
        );
    }
    let at1 = net.ledger.state_at(1).unwrap();
    assert!(at1.branches[&BranchId::main()].state.claims.is_empty());
    let at2 = net.ledger.state_at(2).unwrap();
    assert_eq!(at2.branches[&BranchId::main()].state.claims.len(), 2);
}

// ------------------------------------------------------------------ simulation

fn engine() -> eel_simulation::RelationalGraphEngine {
    let d = sim_define("SIM-1", &[]);
    eel_simulation::RelationalGraphEngine::new(
        d.job,
        d.ruleset,
        d.initial_conditions,
        key(10).identity_id(),
    )
    .unwrap()
}

#[test]
fn same_seed_same_final_hash() {
    let e = engine();
    assert_eq!(e.execute_seed(5).unwrap(), e.execute_seed(5).unwrap());
}

/// Runs a simulation in a child process and prints its final hash and metrics.
#[test]
fn same_inputs_across_processes_same_metrics() {
    if std::env::var("EEL_SIM_CHILD").is_ok() {
        let (r, _) = engine().execute_seed(7).unwrap();
        println!(
            "SIMRESULT {}",
            serde_json::to_string(&(r.final_state_hash, r.metrics)).unwrap()
        );
        return;
    }
    let run = || {
        let out = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "same_inputs_across_processes_same_metrics",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("EEL_SIM_CHILD", "1")
            .output()
            .unwrap();
        let text = String::from_utf8(out.stdout).unwrap();
        text.lines()
            .find_map(|l| l.split_once("SIMRESULT ").map(|(_, r)| r.to_string()))
            .expect("child printed a result")
    };
    let (a, b) = (run(), run());
    assert_eq!(a, b);
    let (local, _) = engine().execute_seed(7).unwrap();
    assert_eq!(
        a,
        serde_json::to_string(&(local.final_state_hash, local.metrics)).unwrap()
    );
}

#[test]
fn modified_ruleset_different_ruleset_hash() {
    let mut r = ruleset();
    r.persistence_threshold += 1;
    assert_ne!(
        eel_simulation::ruleset_hash(&r).unwrap(),
        eel_simulation::ruleset_hash(&ruleset()).unwrap()
    );
}

#[test]
fn fabricated_result_fails_replay() {
    use eel_simulation::UsefulWorkEngine;
    let e = engine();
    let (mut r, _) = e.execute_seed(3).unwrap();
    assert!(e.verify(&r).unwrap());
    r.final_state_hash = ContentHash(Hash::digest(b"made up"));
    r.result_id = r.compute_id().unwrap();
    assert!(!e.verify(&r).unwrap());
}

#[test]
fn simulation_produces_structure() {
    // Not a scientific claim: a sanity check that the development metrics
    // are not trivially zero for the example parameters.
    let (r, _) = engine().execute_seed(1).unwrap();
    assert!(r.metrics.iter().any(|m| m.value > 0), "{:?}", r.metrics);
}
