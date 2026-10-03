# Architecture

One modular Rust workspace, no microservices. The design follows a single rule:
**state is a pure function of the ordered event history.** Everything else is
there to make that history signed, ordered, persistent and queryable.

```
identity ─► signed epistemic event ─► ledger (append-only, blocks, Merkle)
                                          │
                                          ▼
                               state projector (all rules)
                                          │
                                          ▼
                    ontology: claims · evidence · objections · branches
                    validators · canonicalization · research · simulations
```

## Crates and dependency direction

Dependencies point downward only.

```
eel-cli ──────────┐
eel-api ──────────┤
                  ▼
             eel-storage ──► eel-ledger ──► eel-ontology ──┬──► eel-validation ──┐
                                                           ├──► eel-xchange ─────┤
                                                           ├──► eel-simulation ──┤
                                                           └──► eel-crypto ──────┤
                                                                                 ▼
                                                                             eel-core
```

| Crate | Responsibility | Knows nothing about |
| --- | --- | --- |
| `eel-core` | Typed identifiers; canonical serialization; `EpistemicEvent` and every payload type; genesis configuration | signatures, storage, rules |
| `eel-crypto` | Ed25519 keys; event id → signature; verification; keystore files | state |
| `eel-simulation` | Deterministic integer simulator; `UsefulWorkEngine`; replay verification | the ledger |
| `eel-validation` | Validator authority (domain, time, identity); `ValidatorAdmissionPolicy`; `ReputationProvider`; canonicalization rule evaluation | events |
| `eel-xchange` | Pool totals; outcome-independent protocol credit | currency (there is none) |
| `eel-ontology` | `OntologyState` and `StateProjector::apply_event`: the only place an event can be rejected for its content | blocks, storage |
| `eel-ledger` | Event ordering, parent checks, Merkle roots, blocks, block import, state at height, branch views | SQL |
| `eel-storage` | SQLite schema and migrations; persist blocks; reload with full re-verification; projection tables | HTTP |
| `eel-api` | Axum REST API; data-directory layout; `eel-node` | terminal UX |
| `eel-cli` | The `eel` binary: YAML in, signed events out | — |

## Event lifecycle

1. **Authoring (client).** The CLI builds a payload, sets `parent_events` to
   the branch head and `timestamp` to the author's clock, computes
   `event_id = BLAKE3(canonical unsigned event)`, and signs the id with the
   author's Ed25519 key. Private keys never leave the client.
2. **Ledger checks.** Not a duplicate; every parent exists; the timestamp is not
   earlier than any parent's.
3. **Projection.** `OntologyState::apply_event` verifies the signature against
   the registered key, then applies the type-specific rules (existence,
   uniqueness, authorship, validator domain and activity, replay of
   simulation verifications). It works on a copy and commits only on success,
   so a rejected event changes nothing.
4. **Sealing.** Pending events are sealed into a block: Merkle root of event
   ids, state root of the projected ontology, timestamp derived from the
   events, proposer signature over the block hash.
5. **Persistence.** The block and its events are written in one SQLite
   transaction; projection tables are refreshed.

## Determinism

Every node that applies the same ordered events from the same genesis derives
the same state root, because:

* canonical bytes are produced by a hand-written serializer (sorted keys, NFC,
  integers only);
* all state collections are ordered maps and sets;
* no rule reads the node's clock, randomness, environment or file system;
* simulation replay uses integer arithmetic and a seeded SplitMix64;
* block timestamps come from events; Ed25519 signatures are deterministic.

`tests/integration` runs three independent nodes on the same event stream
(two re-executing and sealing locally, one importing and verifying blocks) and
asserts identical block hashes, state roots, ontology graphs, claim versions and
simulation verification results.

## Branches

Each branch holds its own ontology (claims, edges, evidence, objections,
replications, validations, proposals). A fork copies the parent's ontology at
its current head; afterwards the two evolve independently and nothing is ever
deleted. Network-wide registries (identities, artifacts, simulation jobs and
results, research, DATA XCHANGE assets) are shared. `Ledger::branch_view`
selects the events that make up a branch's history, and
`Ledger::reconstruct_branch` replays them to rebuild that branch on its own.

## Storage

`events` and `blocks` are append-only (database triggers abort `UPDATE` and
`DELETE`). Loading a database replays and re-verifies every block, so a
modified database fails to load rather than serving a different ontology. The
remaining tables are rebuildable projections for external queries.

## Extension points

Marked `TODO(EEL-FUTURE)` in code:

* `ReputationAdmissionPolicy` and a real `ReputationProvider`;
* PoEUW reward integration, zkVM verification and probabilistic challenges
  behind `UsefulWorkEngine`;
* branch merge execution;
* a settlement interface for an external DATA XCHANGE;
* production networking and consensus to replace the development block sealer.
