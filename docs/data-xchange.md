# DATA XCHANGE (ledger side)

The DATA XCHANGE is the interoperability and resource-allocation layer around
the ledger. In this repository it exists only as protocol structures; there are
no payments, wallets or currency. A tradable currency, if any, belongs to a
separate DATA XCHANGE project that settles by reading ledger events.

## Asset envelopes

```yaml
asset_id: ASSET-0001
asset_type: DATASET   # PAPER, DATASET, EVIDENCE, COMPUTE, REVIEW_LABOR,
                      # REPLICATION_LABOR, SIMULATION, MODEL, PROTOCOL
artifact_ref: <artifact id, optional>
ontology_links: [{ kind: claim, id: EE-REL-0003 }]
access_terms: { license: CC-BY-4.0, open_access: true, conditions: [] }
```

Registered with `XCHANGE_ASSET_REGISTER` (decisions D-025); the provider is the
event author. Listed at `GET /xchange/assets`.

## Artifacts

Large content stays off-ledger. An artifact is registered by its BLAKE3 hash,
with a `file://` URI in the MVP and an optional `sha256:` hash for external
compatibility. `eel artifact verify` detects modified content.
`ipfs://`, `https://`, `doi:` and `git:` URIs are future work.

## Research requests

Requests link to claims, name the work wanted (`EXPERIMENT`, `REPLICATION`,
`SIMULATION`, `FORMAL_REVIEW`, `LITERATURE_REVIEW`, `DATA_COLLECTION`,
`COUNTERMODEL_SEARCH`) and state acceptance conditions. Supporters attach
commitments (see `economics.md`), researchers submit results, and validators
check them.

## What an external exchange can read

* research requests and their acceptance conditions;
* support commitments per pool;
* research results with their outcome direction;
* validations of those results, and which results are credited;
* simulation results and their replay verifications;
* replications and their outcomes.
