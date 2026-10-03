# Ontology

The ontology is the projection of the ledger. It is never edited directly.

## Claims

A claim has an author-chosen id, a version, title, statement, claim types,
ontology level, procedural status, dependencies, an optional
`ontological_defensibility` record, and a `domain` (decisions D-012).

* Created at version 1 with status `PROPOSED`. Dependencies must exist on
  the branch; a claim cannot depend on itself.
* Only the author may revise or supersede (D-013). A revision is the full
  new version, numbered current + 1, and starts at `PROPOSED` (D-014). Every
  version is kept with its event, author and rationale.
* Superseded claims stay queryable with their full history.

### Statuses

`PROPOSED`, `CANDIDATE`, `SUPPORTED`, `ROBUST`, `CONTESTED`, `SUPERSEDED`,
`FALSIFIED`, `RETIRED`. These are procedural states. Only an accepted
canonicalization sets anything other than `PROPOSED` and `SUPERSEDED` (D-011).

### Ontological defensibility

Optional and extensible: required entities, ontology level, lower-order
dependencies, emergent properties, claim nature, new primitives, a
lower-assumption alternative, empirical constraints and failure conditions,
plus a free `extensions` map.

## Edges

Stored explicitly with the event that created them, and never removed:
`DEPENDS_ON`, `SUPPORTS`, `CONTRADICTS`, `REFINES`, `SUPERSEDES`,
`REPLICATES`, `FALSIFIES`, `IMPLEMENTS`, `DERIVES_FROM`, `GENERALIZES`,
`SPECIALIZES`, `ALTERNATIVE_TO`, `OBSERVES`, `PREDICTS`, `USES_DATA_FROM`.
Claims may declare relations to any existing object when created or revised
(except `SUPERSEDES`, which needs `CLAIM_SUPERSEDE`).

Dependency traversal follows each claim's current dependencies breadth first,
ordered by claim id.

## Evidence, objections and replications

* Evidence bears on one claim and may cite artifacts, a simulation result or a
  research result. Only its author can retract it; retracted evidence stays
  visible.
* Objections target any object and have a severity (`MINOR`, `MAJOR`,
  `CRITICAL`). Anyone may reply; only the objection's author can resolve it
  (D-015).
* Replications target evidence, a simulation result or a research result;
  only the replicator reports the outcome.

## Branches

```
main
 ├── relational-emergence
 └── alternative-model
```

`BRANCH_CREATE` is recorded on the parent, with `fork_event` equal to the
parent's head. The new branch copies the parent's ontology and then evolves
independently; no branch can delete or rewrite history (D-016). A branch can be
rebuilt on its own from its *view*: every network-wide event plus the
branch-scoped events of its ancestors up to each fork and its own.

Merge proposals are recorded but not executed (D-017).
