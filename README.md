# Emergent Epistemic Ledger (EEL)

An open, locally runnable prototype of epistemic infrastructure: claims,
evidence, objections, replications and simulations recorded as signed events
in an append-only ledger, from which every node deterministically projects the
same evolving ontology.

> **Ledger consensus is not truth consensus.** The ledger proves that a
> contribution was made, by whom, when, and that it has not been altered. It
> never declares a claim true. Statuses such as `SUPPORTED` mean "accepted
> under these explicit, configurable procedures", and they remain revisable.

This repository implements Phases 0–5 of the *EEL Claude Code Implementation
Specification v0.1*. It has **no cryptocurrency, tokens, wallets, payments or
staking**: support commitments are counted in arbitrary development units, and
"protocol credit" is a non-transferable tally of validated work. Any future
DATA XCHANGE currency is a separate project.

## Quick start

```sh
cargo test                                         # 53 tests, including the three-node test
cargo run --bin eel -- identity create             # creates ./.eel with the development network
cargo run --bin eel -- claim create examples/claim.yaml
cargo run --bin eel -- ontology graph
cargo run --bin eel -- research request create examples/research-request.yaml
cargo run --bin eel -- sim run examples/simulation.yaml
cargo run --bin eel -- --identity VAL-B sim verify <result-id>
cargo run --bin eel-node -- start                  # REST API on http://127.0.0.1:7878
```

`examples/walkthrough.sh` runs every success criterion of the specification
through the CLI on a fresh data directory: two researchers, a claim, an
objection, a revision, competing branches, domain-restricted validation, a
research request with support commitments, a deterministic simulation replayed
by two validators, outcome-independent credit, independent replications, and a
canonicalization that needs two validators.

## What is where

| Path | Contents |
| --- | --- |
| `crates/eel-core` | Typed ids, canonical serialization, every protocol object and event |
| `crates/eel-crypto` | Ed25519 identities, event signing and verification, local keystore |
| `crates/eel-ontology` | The state projector: every validation rule and the projected ontology |
| `crates/eel-ledger` | Append-only event log, Merkle trees, blocks, historical and branch reconstruction |
| `crates/eel-validation` | Genesis validators, domain authority, admission and reputation interfaces, canonicalization rules |
| `crates/eel-simulation` | Deterministic integer relational-graph simulator and replay verification |
| `crates/eel-xchange` | Research-market structures: pools, outcome-independent credit |
| `crates/eel-storage` | SQLite persistence; events and blocks are append-only at the database level |
| `crates/eel-api` | REST API and the `eel-node` binary |
| `crates/eel-cli` | The `eel` command-line client |
| `docs/` | Protocol documentation, threat model, and `decisions.md` |
| `schemas/` | JSON Schemas generated from the Rust types (`eel schema`) |
| `examples/` | YAML inputs, the genesis test ontology, and the walkthrough |
| `fixtures/` | Development genesis and the public development validator keys |
| `tests/` | Integration, determinism and adversarial suites |

Start with [ARCHITECTURE.md](ARCHITECTURE.md), then
[docs/protocol.md](docs/protocol.md). Every place where the specification left
a behavior open is recorded in [docs/decisions.md](docs/decisions.md).

## The constitution

The genesis file must carry these principles, and the code enforces the ones
that can be enforced:

1. Ledger consensus is not truth consensus.
2. Economic stake is not epistemic authority.
3. Minority branches remain recoverable.
4. Provenance is immutable.
5. Scientific claims are revisable.
6. Falsification is valuable.
7. Canonicalization is provisional.

## Status

A prototype for exercising protocol mechanics. The genesis test ontology
(`EE-REL-0001` … `EE-REL-0005`) exists only to drive the examples; the
Emergent Existence framework itself is not imported. The development network's
validator keys are public. Do not use this for anything that needs security.

## License

Apache-2.0 (provisional; see decisions D-030).
