# Protocol

## Objects and identifiers

| Identifier | Derivation | Example |
| --- | --- | --- |
| `EventId` | BLAKE3 of the canonical unsigned event | 64 hex chars |
| `BlockHash` | BLAKE3 of the canonical unsigned block header | 64 hex chars |
| `StateRoot` | BLAKE3 of the canonical projected state | 64 hex chars |
| `IdentityId` | BLAKE3 of the Ed25519 public key | 64 hex chars |
| `ArtifactId` | BLAKE3 of the artifact's bytes | 64 hex chars |
| `SimulationResultId` | BLAKE3 of the canonical result with the id zeroed | 64 hex chars |
| `ClaimId`, `EvidenceId`, `ObjectionId`, `BranchId`, `ValidatorId`, `ValidationId`, `ReplicationId`, `SimulationJobId`, `ResearchRequestId`, `CanonicalizationProposalId`, `XchangeAssetId`, `MergeProposalId` | chosen by the author; 1–128 of `[A-Za-z0-9._:-]` | `EE-REL-0001` |

`ObjectId` is a typed reference `{kind, id}` with `kind` one of `claim`,
`evidence`, `objection`, `replication`, `artifact`, `validation`,
`simulation_job`, `simulation_result`, `research_request`, `research_result`,
`xchange_asset`, `branch`, `identity`.

## Canonical serialization

JSON with object keys sorted by UTF-8 bytes, every string NFC-normalized,
integers in plain decimal, floating-point numbers rejected, and no whitespace.
All hashing and signing uses these bytes. See decisions D-001.

## Epistemic event

```json
{
  "schema_version": "0.1",
  "event_type": "CLAIM_CREATE",
  "timestamp": 1791034658365,
  "author": "<identity id>",
  "branch": "main",
  "parent_events": ["<event id>"],
  "payload": { "CLAIM_CREATE": { "claim": { ... }, "relations": [] } },
  "artifact_refs": [],
  "signature": "<128 hex chars>"
}
```

* `event_id = BLAKE3(canonical(event without signature))`
* `signature = Ed25519(author key, event_id bytes)`
* `payload` is tagged with the event type and must match `event_type`.
* `timestamp` is milliseconds since the Unix epoch, chosen by the author.

The full schema is `schemas/event/EpistemicEvent.json`.

## Event types

| Type | Scope | Effect |
| --- | --- | --- |
| `IDENTITY_REGISTER` | network | Registers a self-signed public key |
| `CLAIM_CREATE` | branch | New claim at version 1, status `PROPOSED`; `DEPENDS_ON` and declared edges |
| `CLAIM_REVISE` | branch | New version by the claim's author; status back to `PROPOSED` |
| `CLAIM_SUPERSEDE` | branch | Marks a claim `SUPERSEDED` by another; adds `SUPERSEDES` |
| `EVIDENCE_ADD` | branch | Evidence bearing on a claim; `SUPPORTS`/`CONTRADICTS`/`OBSERVES` edge |
| `EVIDENCE_RETRACT` | branch | Author retracts evidence (kept, flagged) |
| `OBJECTION_ADD` | branch | Objection to any object, with severity |
| `OBJECTION_REPLY` | branch | Reply; only the objection's author can resolve it |
| `REPLICATION_REGISTER` | branch | Declares a replication of evidence or a result |
| `REPLICATION_RESULT` | branch | Replicator reports the outcome; success adds `REPLICATES` |
| `VALIDATION_SUBMIT` | branch | A validator's procedural validation within its domains |
| `BRANCH_CREATE` | branch (parent) | Forks the parent's ontology at its head |
| `BRANCH_MERGE_PROPOSE` | network | Records a merge proposal (not executed) |
| `CANONICALIZATION_PROPOSE` | branch | Proposes a status for a claim version |
| `CANONICALIZATION_ACCEPT` / `_REJECT` | branch | A validator's vote |
| `ARTIFACT_REGISTER` | network | Off-ledger content by hash |
| `SIMULATION_DEFINE` | network | Job with its ruleset and initial conditions |
| `SIMULATION_RESULT` | network | A worker's result for one seed |
| `SIMULATION_VERIFY` | network | A validator's replay outcome, re-checked by every node |
| `RESEARCH_REQUEST_CREATE` | network | A research request linked to claims |
| `RESEARCH_REQUEST_SUPPORT` | network | A support commitment in development units |
| `RESEARCH_RESULT_SUBMIT` | network | A result with outcome `SUPPORTING`/`CONTRADICTING`/`NULL`/`INCONCLUSIVE` |
| `XCHANGE_ASSET_REGISTER` | network | A DATA XCHANGE asset envelope (decisions D-025) |

Branch-scoped events affect only the branch they are recorded on.

## Validity

An event is accepted only if all of the following hold, in this order:

1. it is not already in the ledger, its parents exist, and its timestamp is
   not earlier than any parent's;
2. its schema version is supported and its payload matches its type;
3. its author is registered (or it is a self-registration) and the signature
   verifies against the author's key;
4. its branch exists;
5. the type-specific rules in `eel-ontology` hold (see `ontology.md`,
   `validation.md`, `canonicalization.md`, `simulation.md`).

## Blocks

See `ledger.md`.
