# Decisions

The implementation specification v0.1 is authoritative. Where it leaves a
behavior open, this file records the gap and the choice made. Each choice is
the simplest deterministic behavior that can be extended later. Code that
depends on a decision cites it as `decisions D-0NN`.

Status: **Open** means a project decision is still wanted and the current
behavior is a placeholder. **Settled** means the choice follows directly from
the specification or has no reasonable alternative at this stage.

## Serialization, identity and signatures

**D-001 Canonical serialization format.** *Settled.* Canonical bytes are JSON
with keys sorted by UTF-8 bytes, all strings (keys and values) normalized to
Unicode NFC, integers in plain decimal, floats rejected, and no whitespace.
The serializer is hand-written in `eel-core::canonical` so that no dependency
feature (for example `serde_json/preserve_order`) can change protocol hashes.
Two strings that differ only in normalization form produce the same bytes;
this is intended.

**D-002 What is signed.** *Settled.* `event_id = BLAKE3(canonical unsigned
event)`, where the unsigned event is every field except `signature`. The
Ed25519 signature is over the 32 bytes of the event id. Ed25519 signatures are
deterministic, so the same key and event always give the same signature.

**D-003 Identity ids and registration.** *Settled.* `identity_id =
BLAKE3(public key)`. `IDENTITY_REGISTER` is self-signed with the key in its
payload. Genesis validators are registered implicitly by genesis. Every other
event must come from a registered identity.

**D-004 Key storage.** *Open.* Keys are stored as unencrypted seed files with
mode `0600` under `<data-dir>/keys/`. A passphrase-encrypted keystore is a
later improvement.

**D-005 Time.** *Settled.* Timestamps are milliseconds since the Unix epoch,
chosen by the author and covered by the signature. Nodes never read their own
clock during validation or projection. An event's timestamp may not precede
any of its parents'. A block's timestamp is the maximum of the previous
block's timestamp and its events' timestamps. Validator activity windows are
evaluated at the event's timestamp.

**D-008 Identifier families.** *Settled.* Content-derived ids (event, block,
state root, identity, artifact, simulation result) are hashes. Ids people need
to cite (claims, evidence, objections, branches, validators, jobs, requests,
validations, replications, proposals, assets) are chosen by the author, are
1–128 characters of `[A-Za-z0-9._:-]`, and must be unique where they live.
The projector enforces uniqueness. A research result has no named id in the
specification, so it is identified by the id of the event that submitted it.

**D-009 Parent events.** *Settled.* Clients set `parent_events` to the head of
the event's branch (empty for the first event). The ledger requires each
parent to exist and not to be later than the event; it does not require a
linear chain, so a future client may cite several parents.

## Ledger and blocks

**D-006 Block sealing on the development network.** *Open.* Production
consensus is out of scope. One genesis validator is named `block_sealer` in
the genesis file and signs every block. The block hash covers every header
field except the signature, so all nodes compute identical block hashes. The
development sealer key is published in `fixtures/` and has no security value.
Nodes that do not hold the key can follow by importing and verifying blocks.

**D-007 Block granularity.** *Settled.* The node and CLI seal one block per
accepted event, so a rejected event never leaves a partial block. Library
users (and the tests) may batch several events per block. Three nodes agree
on block hashes when they seal at the same points, or when they import the
same blocks.

**D-027 Storage.** *Settled.* SQLite holds the genesis configuration, events
and blocks; triggers abort any `UPDATE` or `DELETE` on them. The other tables
required by the specification are projections rewritten after each block, for
external querying. On open, a node rebuilds its ledger by re-verifying every
block signature, Merkle root, event signature and state root from genesis, so
a tampered database fails to load.

## Ontology

**D-010 Claim types and natures.** *Open.* The specification names
`ClaimType` and `ClaimNature` without listing values. `ClaimType` is
Proposition, Definition, Criterion, Hypothesis, Prediction, Observation,
Model, Method and Test. `ClaimNature` follows the Emergent Existence claim
register's uses (Constitutive, Representational, Diagnostic) plus Empirical,
Formal and Interpretive.

**D-011 Status transitions.** *Settled.* A claim is created `PROPOSED`.
`CLAIM_SUPERSEDE` sets `SUPERSEDED`. Every other status is set only by an
accepted canonicalization proposal. Validation, evidence and objections never
change a status, and nothing is set automatically (for example, an open
critical objection does not mark a claim `CONTESTED`; it blocks
canonicalization instead).

**D-012 Domains.** *Open.* Validators have domains but claims in the
specification do not. Claims gain an optional `domain` field (default
`general`). The domain of any other object is the domain of the claims it
bears on (evidence through its claim, an objection or replication through its
target, a simulation through its job's linked claims, a research result
through its linked claims and its request's). A validator must hold every
domain involved.

**D-013 Who may revise.** *Open.* Only a claim's author may revise or
supersede it. Anyone else expresses disagreement through objections, evidence
or a branch. This prevents one contributor from rewriting another's claim on
a shared branch.

**D-014 Status after revision.** *Settled.* A revision starts at `PROPOSED`.
Canonical standing belongs to a specific version, so a new version has not
yet earned it. Proposals cite `target_version` and fail if the claim has been
revised since.

**D-015 Resolving objections.** *Settled.* Anyone may reply to an objection;
only its author can mark it resolved. A claim's author cannot clear the way
to canonicalization by declaring critics answered.

**D-016 Branches.** *Settled.* `BRANCH_CREATE` is recorded on the parent
branch, and `fork_event` must equal the parent's head at that moment. The new
branch starts with a copy of the parent's ontology and evolves separately;
nothing on the parent changes. Claims, evidence, objections, replications,
validations and canonicalization are branch-scoped. Identities, artifacts,
simulation jobs and results, research requests and results, and DATA XCHANGE
assets are network-wide. A branch's history (its "view") is every
network-wide event plus the branch-scoped events of the branch and of its
ancestors up to each fork; replaying that view reproduces the branch state.

**D-017 Merging.** *Open.* `BRANCH_MERGE_PROPOSE` is recorded but not
executed. Merge semantics are deferred.

**D-034 Edges.** *Settled.* Edges are stored explicitly and never removed.
`CLAIM_CREATE` and `CLAIM_REVISE` add `DEPENDS_ON` edges and any declared
relations; evidence adds `SUPPORTS`/`CONTRADICTS`/`OBSERVES` and
`DERIVES_FROM` for the simulation or research result it rests on; successful
replications add `REPLICATES`; supersession adds `SUPERSEDES`. Dependency
traversal uses each claim's current dependencies, breadth first, ordered by
claim id.

**D-031 A "demonstrates possibility" relation.** *Open.* A replayed
simulation shows that a rule *can* produce a structure, not that nature does.
A dedicated relation (or evidence relation) would record that distinction;
the specification's relation list is fixed, so this is not implemented.

**D-032 Objections to edges.** *Open.* Edges are judgments too. `ObjectId`
has no edge kind, so objections cannot target an edge yet.

## Trust layer

**D-018 Canonicalization voting.** *Settled.* `CANONICALIZATION_ACCEPT` and
`_REJECT` are votes by validators holding the claim's domain, one vote per
validator per proposal. After each accept, the configured rule is evaluated
on current state; when it holds, the proposal is accepted and the claim takes
the proposed status. Otherwise the proposal stays open and records its
blockers. A proposal is rejected when `minimum_validators` validators reject
it. `PROPOSED` and `SUPERSEDED` cannot be proposed.

**D-019 Independent replications.** *Open.* The specification lists
replications by plain string; a `ReplicationId` type was added. A replication
counts towards `minimum_independent_replications` when it succeeded, it
replicates one of the proposal's evidence items (or the simulation or research
result that evidence rests on), and its author is neither the claim's author
nor the author of the replicated object. Each identity counts once. This is
identity separation only; institutional, data, method and implementation
independence are future work.

**D-020 Objection counting.** *Settled.* Unresolved critical objections are
counted from state (those targeting the claim or any of its evidence), not
taken from the proposal's `unresolved_objections` list, which is informational.

## Simulation

**D-021 Verification by replay.** *Settled.* `SIMULATION_RESULT` is recorded
as submitted. `SIMULATION_VERIFY` must come from a validator holding the
job's domains who is not the worker; every node replays the result while
projecting and rejects the event if its `reproduced` value is wrong. The
ledger therefore cannot contain a false verification, and a fabricated
result can only ever be verified as not reproduced.

**D-022 Engine and metrics.** *Settled.* SplitMix64 PRNG, integer-only
arithmetic, fixed iteration order, steps capped at 100,000 so replay stays
cheap. Metrics are development instruments: `LIFESPAN` is the longest run of
consecutive persistent steps of any vertex; `STRUCTURAL_PERSISTENCE` is the
permille of steps with at least one persistent structure (a connected group of
two or more persistent vertices); `PERTURBATION_RESISTANCE` is the permille of
perturbations to persistent vertices they survived (−1 if none occurred);
`RECURRENCE` counts steps whose structure set repeats an earlier one;
`COMPRESSION_RATIO` is 1000 minus the permille of distinct sign patterns over
the run; `RECURSIVE_COMPOSABILITY` is the largest number of structure pairs
bridged by one vertex. Effective dimensionality is not implemented, as the
specification requires.

## Research market

**D-023 Protocol credit.** *Settled.* A research result earns one unit of
non-transferable protocol credit once `minimum_validators` validators submit
passing validations of it, whatever its outcome (`SUPPORTING`,
`CONTRADICTING`, `NULL` or `INCONCLUSIVE`). Credit is a tally, not a token.

**D-024 Commitment types.** *Settled.* `CommitmentType` is the white paper's
three pools: `EXECUTION`, `REPLICATION` and `SUPPORT_RISK`. Amounts are
arbitrary development units with no financial value.

**D-025 DATA XCHANGE assets.** *Settled.* The specification's API has
`POST /xchange/assets` but its event list has no matching event, so
`XCHANGE_ASSET_REGISTER` was added.

**D-026 Access terms.** *Open.* `AccessTerms` is a license, an open-access
flag and free-text conditions.

## Interfaces

**D-028 Write API.** *Settled.* Private keys never leave the client, so every
write endpoint accepts a signed `EpistemicEvent`, and typed endpoints (for
example `POST /claims`) also check the event type. The OpenAPI description is
maintained by hand in `docs/openapi.yaml` and served at `GET /openapi.yaml`.

**D-029 CLI and node.** *Settled.* CLI commands open the node database in the
data directory directly; `eel node start` serves the same database over HTTP.
Run one writer at a time.

**D-030 License.** *Open.* Apache-2.0, chosen provisionally for its patent
grant; easy to change before outside contributions arrive.
