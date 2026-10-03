# Contributing

## Before you push

```sh
cargo fmt --all
cargo clippy --workspace --all-targets
cargo test --workspace
examples/walkthrough.sh
```

## Ground rules

1. **Determinism first.** Priority order: determinism, correctness,
   auditability, testability, simplicity, performance. Projection code must
   not read clocks, randomness, the environment or files. Use ordered
   collections (`BTreeMap`, `BTreeSet`). Protocol fields are integers, never
   floats.
2. **Do not invent protocol semantics silently.** If the specification does
   not say what to do, add an entry to `docs/decisions.md`, implement the
   simplest deterministic behavior, and cite the decision in the code.
3. **Every rule lives in the projector.** If an event can be rejected for its
   content, the check belongs in `eel-ontology`, so every node applies it.
4. **History is append-only.** No change may delete or rewrite an event,
   block, claim version or edge.
5. **Deferred systems stay deferred.** No currency, staking, DAO voting or
   consensus based on useful work. Interfaces for future work are marked
   `TODO(EEL-FUTURE)`.
6. **Tests for every rule.** Mandatory tests live in `tests/deterministic`,
   `tests/adversarial` and `tests/integration`. A new attack goes in
   `tests/adversarial` together with the rule that stops it.

## Changing protocol types

Changing any type in `eel-core` changes canonical bytes and therefore every
hash. Regenerate the schemas with `cargo run --bin eel -- schema` and say in
the pull request that event ids change.
