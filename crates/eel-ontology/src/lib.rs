//! The ontology is never stored directly: it is a projection of the ledger.
//!
//! [`OntologyState`] is rebuilt by applying events in ledger order through
//! [`StateProjector::apply_event`]. Every rule that can reject an event lives
//! here, so given identical ordered events, every node derives an identical
//! [`OntologyState::state_root`].

mod projector;
mod state;

pub use projector::{is_branch_scoped, ProjectionError, StateProjector, MAX_SIMULATION_STEPS};
pub use state::*;
