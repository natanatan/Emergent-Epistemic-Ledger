#!/usr/bin/env bash
# End-to-end walkthrough of the specification's success criteria using the CLI.
# Usage: examples/walkthrough.sh   (builds the CLI, uses a fresh data directory)
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build -q --bins
EEL="$PWD/target/debug/eel"
export EEL_HOME="${EEL_HOME:-$(mktemp -d)}"
echo "data directory: $EEL_HOME"
step() { printf '\n== %s\n' "$*"; }
id_of() { sed -n 's/.*"identity_id":"\([0-9a-f]*\)".*/\1/p'; }

step "1. Two researchers (and two replicators) create independent identities"
A=$("$EEL" --json identity create | id_of)
B=$("$EEL" --json identity create | id_of)
C=$("$EEL" --json identity create | id_of)
D=$("$EEL" --json identity create | id_of)
echo "A=$A"; echo "B=$B"; echo "C=$C"; echo "D=$D"
"$EEL" validator list

step "2. A creates the genesis test ontology"
for f in examples/claims/EE-REL-000{1,2,3,4,5}.yaml; do "$EEL" --identity "$A" claim create "$f"; done

step "3. B objects to EE-REL-0003"
"$EEL" --identity "$B" objection add examples/objection.yaml

step "4. A revises EE-REL-0003 without destroying history"
"$EEL" --identity "$A" claim revise examples/claim-revision.yaml
"$EEL" claim history EE-REL-0003

step "5. A and B create competing branches"
"$EEL" --identity "$A" branch create relational-emergence
"$EEL" --identity "$B" branch create alternative-model
"$EEL" --identity "$B" --branch alternative-model claim create examples/alt-claim.yaml
"$EEL" branch list

step "6. A registers an artifact and evidence; a genesis validator validates it in its domain"
"$EEL" --identity "$A" artifact register examples/data.json
"$EEL" --identity "$A" evidence add examples/evidence.yaml
"$EEL" --identity VAL-A validate examples/validation.yaml
echo "VAL-C has no authority in the ontology domain:"
sed 's/VALID-0001/VALID-0002/; s/VAL-A/VAL-C/' examples/validation.yaml > "$EEL_HOME/validation-c.yaml"
if "$EEL" --identity VAL-C validate "$EEL_HOME/validation-c.yaml"; then echo "unexpected"; exit 1; fi

step "7-8. A research request with simulated support commitments"
"$EEL" --identity "$A" research request create examples/research-request.yaml
"$EEL" --identity "$B" research request support REQ-0001 --units 50 --pool execution
"$EEL" --identity "$C" research request support REQ-0001 --units 20 --pool replication

step "9. A deterministic simulation produces a result"
"$EEL" --identity "$A" sim run examples/simulation.yaml --seed 1 | tee "$EEL_HOME/sim.txt"
RESULT=$(sed -n 's/^seed 1: result \([0-9a-f]*\).*/\1/p' "$EEL_HOME/sim.txt")

step "10. Independent validators replay it"
"$EEL" --identity VAL-A sim verify "$RESULT"
"$EEL" --identity VAL-B sim verify "$RESULT"

step "11. The result becomes evidence; a research result is credited whatever its outcome"
cat > "$EEL_HOME/ev-sim.yaml" <<YAML
evidence_id: EV-SIM-0001
claim_id: EE-REL-0003
relation: SUPPORTS
description: Seed 1 of SIM-0001, replayed by two validators.
simulation_result: $RESULT
YAML
"$EEL" --identity "$A" evidence add "$EEL_HOME/ev-sim.yaml"
RR=$("$EEL" --identity "$A" --json research result submit examples/result.yaml | sed -n 's/.*"event_id":"\([0-9a-f]*\)".*/\1/p')
for V in VAL-A VAL-B; do
  cat > "$EEL_HOME/rr-$V.yaml" <<YAML
validation_id: VALID-RR-$V
validator: $V
target: { kind: research_result, id: $RR }
validation_type: METHOD_CONFORMANCE
outcome: PASS
YAML
  "$EEL" --identity "$V" validate "$EEL_HOME/rr-$V.yaml"
done
"$EEL" identity show "$A"

step "Replications by two independent identities, and B resolves the objection"
"$EEL" --identity "$C" replication register examples/replication-1.yaml
"$EEL" --identity "$C" replication result examples/replication-1-result.yaml
"$EEL" --identity "$D" replication register examples/replication-2.yaml
"$EEL" --identity "$D" replication result examples/replication-2-result.yaml
"$EEL" --identity "$B" objection reply OBJ-0001 "Version 2 answers this." --resolves

step "Canonicalization needs two validators"
"$EEL" --identity "$A" canonical propose examples/canonicalization.yaml
"$EEL" --identity VAL-A canonical accept CAN-0001
"$EEL" --identity VAL-B canonical accept CAN-0001
"$EEL" claim show EE-REL-0003

step "12. Everything remains queryable"
"$EEL" ontology graph
"$EEL" --branch alternative-model ontology graph
"$EEL" ontology canonical
"$EEL" ontology state --height 3
"$EEL" node status
echo
echo "Walkthrough complete. Serve this ledger with: EEL_HOME=$EEL_HOME $EEL node start"
