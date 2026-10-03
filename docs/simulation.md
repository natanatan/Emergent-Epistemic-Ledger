# Simulation

A minimal deterministic relational-graph simulator (`RelationalGraphEngine`,
engine version `relational-graph-engine/0.1.0`).

## Model

```rust
struct Vertex { id: u64, state: i64, memory: Vec<i64> }
struct Edge   { source: u64, target: u64, weight: i64 }
```

Integer arithmetic only. Randomness comes from SplitMix64 seeded with the job
seed (separately salted for initialization and for steps). Iteration order is
fixed.

```
initialize_graph(seed)
for each timestep:
    apply_local_interactions          # state·retention/1000 + Σ weight·neighbour / divisor
    generate_deterministic_perturbations
    apply_constraints                 # clamp to ±state_bound
    update_vertex_states              # and memory windows
    detect_persistent_structures      # same-sign, |state| ≥ threshold for the whole window;
                                      # connected groups of ≥ 2 such vertices
    record_metrics
```

## Jobs and results

A job registers its ruleset and initial conditions with `SIMULATION_DEFINE`;
the job carries their hashes. A worker runs a seed and submits
`SIMULATION_RESULT` with the engine, ruleset, initial-state and final-state
hashes, the metrics, and an artifact (canonical JSON of the final graph and
metrics). The result id is the hash of the result itself.

## Verification

Given engine hash, ruleset hash, initial state hash and seed, every compliant
node produces the identical final state hash and metrics. `eel sim verify
<result-id>` replays locally and records `SIMULATION_VERIFY` as the current
validator identity. Every node replays again while projecting and rejects a
verification that misreports the replay, so a fabricated result can only be
recorded as not reproduced (decisions D-021). Workers cannot verify their own
results.

## Metrics

`LIFESPAN`, `STRUCTURAL_PERSISTENCE`, `PERTURBATION_RESISTANCE`, `RECURRENCE`,
`COMPRESSION_RATIO`, `RECURSIVE_COMPOSABILITY`, defined in decisions D-022.
Effective dimensionality is deliberately not implemented.

These metrics are development instruments for exercising the protocol, not
validated measures of emergence. A simulation shows that a rule *can* produce
a structure; it is not evidence that nature does (see decisions D-031).

## Useful work

`UsefulWorkEngine` (`execute`, `verify`) is the abstraction a future
proof-of-epistemically-useful-work scheme would build on. Useful work is not
part of consensus; reward integration, zkVM verification and probabilistic
challenges are `TODO(EEL-FUTURE)`.
