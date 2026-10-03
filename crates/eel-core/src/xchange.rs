//! DATA XCHANGE asset envelopes. No payments are modeled.

use crate::ids::*;
use serde::{Deserialize, Serialize};

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
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum XchangeAssetType {
    Paper,
    Dataset,
    Evidence,
    Compute,
    ReviewLabor,
    ReplicationLabor,
    Simulation,
    Model,
    Protocol,
}

/// Terms under which an asset is offered. Free text in the MVP (decisions D-026).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AccessTerms {
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub open_access: bool,
    #[serde(default)]
    pub conditions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct XchangeAsset {
    pub asset_id: XchangeAssetId,
    pub asset_type: XchangeAssetType,
    #[serde(default)]
    pub artifact_ref: Option<ArtifactId>,
    #[serde(default)]
    pub ontology_links: Vec<ObjectId>,
    pub provider: IdentityId,
    pub access_terms: AccessTerms,
}

/// Off-ledger content, referenced by hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Artifact {
    pub artifact_id: ArtifactId,
    /// `blake3:<hex>`; equal to the artifact id. A `sha256:<hex>` may be added
    /// in `external_hashes` for compatibility.
    pub content_hash: String,
    pub uri: String,
    pub media_type: String,
    pub size_bytes: u64,
    pub author: IdentityId,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub external_hashes: Vec<String>,
}
