//! Strongly typed identifiers.
//!
//! Two families exist:
//!
//! * **Hash identifiers** (`EventId`, `BlockHash`, `StateRoot`, `IdentityId`,
//!   `ArtifactId`, `SimulationResultId`) are derived from content and cannot
//!   be chosen by authors.
//! * **Named identifiers** (`ClaimId`, `EvidenceId`, ...) are chosen by the
//!   author in the event payload, so that people can cite them (for example
//!   `EE-REL-0001`). The projector enforces their uniqueness.

use crate::error::{CoreError, Result};
use crate::hash::Hash;
use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;
use std::str::FromStr;

macro_rules! hash_id {
    ($(#[$m:meta])* $name:ident) => {
        $(#[$m])*
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize, schemars::JsonSchema)]
        #[serde(transparent)]
        pub struct $name(pub Hash);

        impl $name {
            pub fn as_hash(&self) -> &Hash { &self.0 }
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { fmt::Display::fmt(&self.0, f) }
        }
        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{}({})", stringify!($name), self.0.to_hex()) }
        }
        impl FromStr for $name {
            type Err = CoreError;
            fn from_str(s: &str) -> Result<Self> { Ok($name(Hash::from_str(s)?)) }
        }
        impl From<Hash> for $name {
            fn from(h: Hash) -> Self { $name(h) }
        }
    };
}

hash_id!(
    /// BLAKE3 of the canonical unsigned event.
    EventId
);
hash_id!(
    /// BLAKE3 of the canonical unsigned block header.
    BlockHash
);
hash_id!(
    /// BLAKE3 of the canonical projected ontology state.
    StateRoot
);
hash_id!(
    /// BLAKE3 of the identity's Ed25519 public key.
    IdentityId
);
hash_id!(
    /// BLAKE3 of the artifact's content bytes.
    ArtifactId
);
hash_id!(
    /// BLAKE3 of the canonical simulation result (with its id field zeroed).
    SimulationResultId
);
hash_id!(
    /// Generic content hash (rulesets, initial conditions, engine).
    ContentHash
);

/// Maximum length of a named identifier.
pub const MAX_NAMED_ID_LEN: usize = 128;

fn validate_named(kind: &'static str, value: &str) -> Result<()> {
    let err = |reason| CoreError::InvalidId {
        kind,
        value: value.to_string(),
        reason,
    };
    if value.is_empty() {
        return Err(err("must not be empty"));
    }
    if value.len() > MAX_NAMED_ID_LEN {
        return Err(err("longer than 128 bytes"));
    }
    if !value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':'))
    {
        return Err(err(
            "may contain only ASCII letters, digits, '-', '_', '.' and ':'",
        ));
    }
    Ok(())
}

macro_rules! named_id {
    ($(#[$m:meta])* $name:ident) => {
        $(#[$m])*
        #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, schemars::JsonSchema)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self> {
                let value = value.into();
                validate_named(stringify!($name), &value)?;
                Ok($name(value))
            }
            pub fn as_str(&self) -> &str { &self.0 }
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(&self.0) }
        }
        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{}({})", stringify!($name), self.0) }
        }
        impl FromStr for $name {
            type Err = CoreError;
            fn from_str(s: &str) -> Result<Self> { $name::new(s) }
        }
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
                let s = String::deserialize(d)?;
                $name::new(s).map_err(serde::de::Error::custom)
            }
        }
    };
}

named_id!(ClaimId);
named_id!(EvidenceId);
named_id!(ObjectionId);
named_id!(BranchId);
named_id!(ValidatorId);
named_id!(SimulationJobId);
named_id!(ResearchRequestId);
named_id!(ValidationId);
named_id!(
    /// Not listed in the specification's identifier set; needed so that
    /// canonicalization proposals can cite replications (see decisions D-019).
    ReplicationId
);
named_id!(CanonicalizationProposalId);
named_id!(XchangeAssetId);
named_id!(MergeProposalId);

impl BranchId {
    /// The root branch created by genesis.
    pub fn main() -> Self {
        BranchId("main".to_string())
    }
}

/// Kind of object an [`ObjectId`] points to.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ObjectKind {
    Claim,
    Evidence,
    Objection,
    Replication,
    Artifact,
    Validation,
    SimulationJob,
    SimulationResult,
    ResearchRequest,
    ResearchResult,
    XchangeAsset,
    Branch,
    Identity,
}

/// A typed reference to any protocol object.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, schemars::JsonSchema,
)]
pub struct ObjectId {
    pub kind: ObjectKind,
    pub id: String,
}

impl ObjectId {
    pub fn new(kind: ObjectKind, id: impl fmt::Display) -> Self {
        ObjectId {
            kind,
            id: id.to_string(),
        }
    }
    pub fn claim(id: &ClaimId) -> Self {
        Self::new(ObjectKind::Claim, id)
    }
}

impl fmt::Display for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}",
            serde_json::to_value(self.kind).unwrap().as_str().unwrap(),
            self.id
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_ids_validate() {
        assert!(ClaimId::new("EE-REL-0001").is_ok());
        assert!(ClaimId::new("").is_err());
        assert!(ClaimId::new("has space").is_err());
        assert!(serde_json::from_str::<ClaimId>("\"bad id\"").is_err());
    }
}
