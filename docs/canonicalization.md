# Canonicalization

Canonical means *the current best-supported state of a claim version under
explicit, configurable rules*. It does not mean true, compulsory, permanent or
unanimous. Canonicalization is provisional, and a revision returns the claim
to `PROPOSED`.

## Proposal

```yaml
proposal_id: CAN-0001
target_claim: EE-REL-0003
target_version: 2
proposed_status: SUPPORTED
evidence: [EV-0001]
validations: [VALID-0001]
successful_replications: [REP-0001, REP-0002]
unresolved_objections: []
```

At proposal time every cited object must exist, the evidence must belong to the
claim and not be retracted, the replications must have succeeded, and the
target version must be current. `PROPOSED` and `SUPERSEDED` cannot be proposed.

## Rule

Development defaults, configured in genesis and not claimed to be
scientifically optimal:

```yaml
canonicalization:
  minimum_validators: 2
  minimum_independent_replications: 2
  maximum_unresolved_critical_objections: 0
```

## Votes

Validators holding the claim's domain vote with `CANONICALIZATION_ACCEPT` or
`_REJECT`, one vote each. After each accept, the rule is evaluated on current
state:

* **validators:** distinct accepting validators;
* **independent replications:** distinct replicators of the proposal's
  evidence (or the results it rests on), excluding the claim's author and the
  author of the replicated object (decisions D-019);
* **critical objections:** unresolved critical objections to the claim or its
  evidence, counted from state (D-020);
* **version:** the claim has not been revised since the proposal.

If everything holds, the proposal is accepted and the claim takes the
proposed status. Otherwise it stays open and records what it is waiting on.
`minimum_validators` rejections close it as rejected. A single validator can
never satisfy a multi-validator rule.

`GET /ontology/canonical` and `eel ontology canonical` list claims whose
current version holds a canonicalized status.
