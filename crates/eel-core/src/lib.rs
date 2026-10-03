//! Core types for the Emergent Epistemic Ledger (EEL).
//!
//! This crate has no knowledge of storage, networking or signatures. It defines
//! the strongly typed identifiers, canonical serialization, hashing helpers and
//! every protocol object that travels inside an [`EpistemicEvent`].

pub mod canonical;
pub mod error;
pub mod event;
pub mod genesis;
pub mod hash;
pub mod ids;
pub mod ontology;
pub mod research;
pub mod simulation;
pub mod time;
pub mod validation;
pub mod xchange;

pub use canonical::CanonicalSerialize;
pub use error::CoreError;
pub use event::{EpistemicEvent, EventPayload, EventType, UnsignedEvent};
pub use hash::Hash;
pub use ids::*;
pub use time::Timestamp;

/// Protocol schema version written into every event.
pub const SCHEMA_VERSION: &str = "0.1";
