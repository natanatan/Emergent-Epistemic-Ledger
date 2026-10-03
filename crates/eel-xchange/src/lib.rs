//! Research-market logic that stays inside the ledger.
//!
//! There is no currency here. Support commitments are counted in arbitrary
//! development units, and "protocol credit" is a non-transferable tally of
//! validated work. A future DATA XCHANGE (a separate project) may settle real
//! bounties by reading these ledger events.
//!
//! TODO(EEL-FUTURE): settlement interface for the external DATA XCHANGE.

use eel_core::research::{CommitmentType, ResearchOutcome, SupportCommitment};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Credit awarded for one validated research result.
pub const CREDIT_PER_VALID_RESULT: u64 = 1;

/// Researchers receive protocol credit for valid work regardless of the
/// direction of the outcome, so nobody is paid to confirm.
pub fn credit_for_result(
    passing_validators: usize,
    minimum_validators: u32,
    _outcome: ResearchOutcome,
) -> u64 {
    if passing_validators >= minimum_validators as usize {
        CREDIT_PER_VALID_RESULT
    } else {
        0
    }
}

/// Committed development units per pool.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PoolTotals {
    pub execution: u64,
    pub replication: u64,
    pub support_risk: u64,
}

pub fn pool_totals<'a>(commitments: impl IntoIterator<Item = &'a SupportCommitment>) -> PoolTotals {
    let mut t = PoolTotals::default();
    for c in commitments {
        let slot = match c.commitment_type {
            CommitmentType::Execution => &mut t.execution,
            CommitmentType::Replication => &mut t.replication,
            CommitmentType::SupportRisk => &mut t.support_risk,
        };
        *slot = slot.saturating_add(c.amount_units);
    }
    t
}

/// Supporters per pool, for display.
pub fn supporters_by_pool<'a>(
    commitments: impl IntoIterator<Item = &'a SupportCommitment>,
) -> BTreeMap<CommitmentType, usize> {
    let mut m = BTreeMap::new();
    for c in commitments {
        *m.entry(c.commitment_type).or_insert(0) += 1;
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credit_is_outcome_independent() {
        for o in [
            ResearchOutcome::Supporting,
            ResearchOutcome::Contradicting,
            ResearchOutcome::Null,
            ResearchOutcome::Inconclusive,
        ] {
            assert_eq!(credit_for_result(2, 2, o), 1);
            assert_eq!(credit_for_result(1, 2, o), 0);
        }
    }
}
