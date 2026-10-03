# Validation

**Validation is procedural.** A validation states that something was done
according to a declared protocol. It never makes a claim true and never
changes a claim's status.

## Validators

Validators are configured in genesis:

```yaml
validators:
  - validator_id: VAL-A
    public_key: <hex>
    domains: [general, ontology, physics]
    validator_class: GENESIS
    activated_at: 0
    expires_at: null
```

To act as a validator, an event's author must be the validator's identity,
the validator must be active at the event's timestamp, and it must hold every
domain the target touches (decisions D-012). Admission goes through
`ValidatorAdmissionPolicy`; the MVP uses `GenesisOnlyAdmissionPolicy`.
`ReputationAdmissionPolicy` and `ReputationProvider` are interfaces only
(`TODO(EEL-FUTURE)`), and the MVP reputation provider returns zeros.

## Validation types

`METHOD_CONFORMANCE`, `ARTIFACT_INTEGRITY`, `SIMULATION_REPLAY`,
`REPLICATION_CONFORMANCE`, `DATA_PROVENANCE`, each with outcome `PASS`,
`FAIL` or `INCONCLUSIVE`. A validator submits a given type for a given target
once.

## Side effects

The only side effect of a validation is protocol credit: a research result
that has passing validations from `minimum_validators` distinct validators
credits its submitter once, whatever its outcome (decisions D-023).

## Bootstrap transition

Initial trust is constituted (genesis); enduring trust must be earned.
`ValidatorClass::Earned` exists so that a future admission policy can add
validators from domain reputation, and genesis validators can be given
`expires_at` to sunset their privileges.
