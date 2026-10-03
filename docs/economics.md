# Economics

**Nothing in this repository has financial value.** There is no currency, no
token, no wallet, no transfer, no interest and no staking contract.

## Separations

```
economic stake  ≠  epistemic reputation  ≠  governance authority
```

* Support commitments never confer validator rights or change a claim.
* Protocol credit is a non-transferable tally of validated work, kept apart
  from reputation (which is an interface returning zeros in the MVP).
* Funding never establishes validity: canonicalization depends only on
  validators, replications and objections.

## Pools

Support commitments name one of the white paper's three pools (decisions D-024):

| Pool | Funds |
| --- | --- |
| `EXECUTION` | Performing the experiment or simulation |
| `REPLICATION` | Independent parties reproducing it |
| `SUPPORT_RISK` | Commitment to the expected epistemic productivity of a direction |

Amounts are arbitrary development units.

## Outcome-independent credit

A research result is credited once enough validators confirm it was carried
out validly, regardless of whether it supports, contradicts, is null or
inconclusive. Researchers are therefore never paid to confirm (decisions
D-023). Delayed rewards for independent replication, information-gain
measures and long-term value profiles are future work.

## Regulatory note

A future tradable instrument attached to these pools (especially
`SUPPORT_RISK`) may be regulated as a security, a crypto-asset or a
prediction market depending on jurisdiction. Such work belongs to the separate
DATA XCHANGE project and needs legal review before launch.
