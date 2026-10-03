//! The trust layer: who may validate what, and when a canonicalization
//! proposal has met the configured procedure.
//!
//! Nothing here decides truth. Validation is procedural, and canonicalization
//! means "the current best-supported state under these explicit rules".

use eel_core::validation::{CanonicalizationRule, Validator, ValidatorClass};
use eel_core::{IdentityId, Timestamp, ValidatorId};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ValidationError {
    #[error("unknown validator `{0}`")]
    UnknownValidator(ValidatorId),
    #[error("event author is not the identity of validator `{0}`")]
    NotValidatorIdentity(ValidatorId),
    #[error("validator `{0}` is not active at this time")]
    Inactive(ValidatorId),
    #[error("validator `{validator}` has no authority in domain `{domain}`")]
    OutsideDomain {
        validator: ValidatorId,
        domain: String,
    },
}

/// Decides whether an identity may act as a validator in a domain.
pub trait ValidatorAdmissionPolicy {
    fn eligible(&self, identity: &IdentityId, domain: &str) -> Result<bool, ValidationError>;
}

/// MVP policy: only genesis validators, only in their configured domains.
pub struct GenesisOnlyAdmissionPolicy<'a> {
    pub validators: &'a BTreeMap<ValidatorId, Validator>,
}

impl ValidatorAdmissionPolicy for GenesisOnlyAdmissionPolicy<'_> {
    fn eligible(&self, identity: &IdentityId, domain: &str) -> Result<bool, ValidationError> {
        Ok(self.validators.values().any(|v| {
            v.identity_id == *identity
                && v.validator_class == ValidatorClass::Genesis
                && v.covers_domain(domain)
        }))
    }
}

/// TODO(EEL-FUTURE): admit earned validators from domain reputation.
pub struct ReputationAdmissionPolicy;

/// Multidimensional, domain-specific reputation. Not purchasable, not
/// transferable, and separate from economic stake and governance.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReputationMetrics {
    pub replication_reliability: u32,
    pub formal_reasoning: u32,
    pub experimental_design: u32,
    pub simulation_verification: u32,
    pub objection_quality: u32,
    pub prediction_calibration: u32,
    pub data_quality: u32,
    pub ontology_integration: u32,
}

pub trait ReputationProvider {
    fn metrics(
        &self,
        identity: &IdentityId,
        domain: &str,
    ) -> Result<ReputationMetrics, ValidationError>;
}

/// MVP: everyone has zero reputation.
/// TODO(EEL-FUTURE): compute reputation from validated contribution history.
pub struct NullReputationProvider;

impl ReputationProvider for NullReputationProvider {
    fn metrics(&self, _: &IdentityId, _: &str) -> Result<ReputationMetrics, ValidationError> {
        Ok(ReputationMetrics::default())
    }
}

/// Checks that `author` may act as `validator_id` at `at` in every domain.
pub fn authorize_validator<'a>(
    validators: &'a BTreeMap<ValidatorId, Validator>,
    validator_id: &ValidatorId,
    author: &IdentityId,
    at: Timestamp,
    domains: &BTreeSet<String>,
) -> Result<&'a Validator, ValidationError> {
    let v = validators
        .get(validator_id)
        .ok_or_else(|| ValidationError::UnknownValidator(validator_id.clone()))?;
    if v.identity_id != *author {
        return Err(ValidationError::NotValidatorIdentity(validator_id.clone()));
    }
    if !v.is_active_at(at) {
        return Err(ValidationError::Inactive(validator_id.clone()));
    }
    let policy = GenesisOnlyAdmissionPolicy { validators };
    for d in domains {
        if !policy.eligible(author, d)? {
            return Err(ValidationError::OutsideDomain {
                validator: validator_id.clone(),
                domain: d.clone(),
            });
        }
    }
    Ok(v)
}

/// Facts about a proposal at evaluation time, computed by the projector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposalFacts {
    pub accepting_validators: usize,
    pub independent_replications: usize,
    pub unresolved_critical_objections: usize,
    pub version_is_current: bool,
}

/// Why a proposal cannot yet be accepted. Empty means it can.
pub fn canonicalization_blockers(rule: &CanonicalizationRule, f: &ProposalFacts) -> Vec<String> {
    let mut out = vec![];
    if f.accepting_validators < rule.minimum_validators as usize {
        out.push(format!(
            "{} of {} required validator acceptances",
            f.accepting_validators, rule.minimum_validators
        ));
    }
    if f.independent_replications < rule.minimum_independent_replications as usize {
        out.push(format!(
            "{} of {} required independent replications",
            f.independent_replications, rule.minimum_independent_replications
        ));
    }
    if f.unresolved_critical_objections > rule.maximum_unresolved_critical_objections as usize {
        out.push(format!(
            "{} unresolved critical objections (maximum {})",
            f.unresolved_critical_objections, rule.maximum_unresolved_critical_objections
        ));
    }
    if !f.version_is_current {
        out.push("the claim has been revised since the proposal".into());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> BTreeMap<ValidatorId, Validator> {
        let v = Validator {
            validator_id: "VAL-A".parse().unwrap(),
            identity_id: IdentityId::default(),
            domains: vec!["ontology".into()],
            validator_class: ValidatorClass::Genesis,
            activated_at: Timestamp(0),
            expires_at: Some(Timestamp(100)),
        };
        [(v.validator_id.clone(), v)].into_iter().collect()
    }

    #[test]
    fn domain_and_time_bounds() {
        let r = registry();
        let id = "VAL-A".parse().unwrap();
        let me = IdentityId::default();
        let ont: BTreeSet<String> = ["ontology".to_string()].into();
        let onc: BTreeSet<String> = ["oncology".to_string()].into();
        assert!(authorize_validator(&r, &id, &me, Timestamp(5), &ont).is_ok());
        assert!(matches!(
            authorize_validator(&r, &id, &me, Timestamp(5), &onc),
            Err(ValidationError::OutsideDomain { .. })
        ));
        assert!(matches!(
            authorize_validator(&r, &id, &me, Timestamp(100), &ont),
            Err(ValidationError::Inactive(_))
        ));
    }

    #[test]
    fn one_validator_cannot_satisfy_two() {
        let f = ProposalFacts {
            accepting_validators: 1,
            independent_replications: 2,
            unresolved_critical_objections: 0,
            version_is_current: true,
        };
        assert!(!canonicalization_blockers(&CanonicalizationRule::default(), &f).is_empty());
    }
}
