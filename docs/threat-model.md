# Threat model

Scope: a development network with public validator keys and a single block
sealer. The table states what the MVP does about each threat and where it is
tested.

| Threat | MVP response | Tested in |
| --- | --- | --- |
| **Sybil creation**: many pseudonymous identities simulate independent validation | Validator allowlist from genesis; validator actions must be signed by the validator's own identity; replication independence excludes the claim and evidence authors (identity separation only) | `adversarial::sybil_identities_cannot_validate`, `validator_cannot_impersonate_another`, `replications_by_claim_author_are_not_independent` |
| **Genesis validator capture** | Multi-validator thresholds; domain restrictions; activity windows (`expires_at`); every action attributable; any branch can fork | `validator_outside_domain_rejected_and_inside_accepted`, `expired_validator_rejected`, `integration::canonicalization_waits_for_second_validator` |
| **Forged or altered events** | Ed25519 over the content-derived event id; author bound to key hash | `forged_signature_rejected`, `deterministic::valid_signature_accepted_invalid_rejected` |
| **Replay** | Event ids are unique in the ledger | `replayed_event_rejected` |
| **History rewriting** | No delete or rewrite operation exists; SQLite triggers block `UPDATE`/`DELETE` on events and blocks; loading re-verifies everything | `history_cannot_be_deleted`, `eel-storage` unit test, `tampered_block_fails_import` |
| **Plagiarism** | Timestamped, signed provenance on every object and version; ancestry links | `integration::success_criteria_scenario` |
| **Artifact mutation** | Content-addressed BLAKE3 ids; `eel artifact verify` | `artifact_mutation_is_detected` |
| **Simulation fabrication** | Deterministic replay by every node; misreported verifications rejected; workers cannot self-verify | `fabricated_simulation_result_is_recorded_as_not_reproduced`, `worker_cannot_verify_own_result_and_false_verification_rejected` |
| **Claim hijacking** | Only the author revises or supersedes; only an objection's author resolves it | `only_author_revises_claim`, `claim_author_cannot_resolve_someone_elses_objection` |
| **Funding buys truth** | Validation and funding never change status; only canonicalization does | `validation_never_sets_supported` |
| **Reviewer collusion** | Future: independence metrics and graph analysis | — |
| **AI spam** | Future: submission cost, deduplication, review requirements | — |
| **Economic capture** | Constitutional separation: economic stake ≠ epistemic reputation ≠ governance | `economics.md` |

## Known limitations

* The development sealer key is public: anyone can seal blocks on a copy of the
  network. Production consensus is out of scope.
* Independence is identity separation only. One lab with several registered
  identities can still appear as several replicators.
* Keys are stored unencrypted on disk (decisions D-004).
* The CLI and a running node share one SQLite file; run one writer at a time.
