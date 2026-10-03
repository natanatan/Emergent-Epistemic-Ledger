# Ledger

## Append-only log

Events are applied in submission order. Each must be new, cite existing
parents, and not be timestamped before them. The projector validates and
applies it atomically; rejected events leave no trace.

## Merkle tree

Over the ordered event ids of a block:

* leaf = BLAKE3(`0x00` ‖ event id)
* node = BLAKE3(`0x01` ‖ left ‖ right)
* an odd last node is promoted, never duplicated
* the root of no events is 32 zero bytes

## Blocks

```
height               u64
previous_block_hash  BlockHash (zero for genesis)
timestamp            max(previous block, events in block)
events_root          Merkle root of the block's event ids
state_root           BLAKE3 of the canonical ontology after the block
proposer             ValidatorId of the configured block sealer
signature            Ed25519 over the block hash
```

The block hash covers every field except the signature. On the development
network one genesis validator (`block_sealer`) seals blocks (decisions D-006).
The node seals one block per accepted event (D-007).

## Import and verification

`Ledger::import_block` accepts a block from elsewhere only if it extends the
head, its proposer is the configured sealer, its hash and signature verify,
its Merkle root matches its events, every event applies, its timestamp is
derived from its events, and the resulting state root matches. Otherwise
nothing changes. Loading from SQLite uses the same path, so storage is
re-verified on every start.

## History

* `state_at(height)` replays from genesis to any block.
* `branch_view(branch)` and `reconstruct_branch(branch)` rebuild one branch.
* The REST API exposes `GET /ontology/state/{height}`.

## Storage

SQLite tables `meta`, `events` and `blocks` are the source of truth and are
protected by triggers that abort `UPDATE` and `DELETE`. The other tables
(`identities`, `validators`, `claims`, `claim_versions`, `ontology_edges`,
`evidence`, `objections`, `validations`, `branches`,
`canonicalization_proposals`, `artifacts`, `xchange_assets`,
`research_requests`, `support_commitments`, `research_results`,
`simulation_jobs`, `simulation_results`, `simulation_verifications`) are
projections rewritten after each block.
