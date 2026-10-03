use serde::{Deserialize, Serialize};

/// Milliseconds since the Unix epoch (UTC).
///
/// Timestamps are chosen by the event author and are part of the signed event.
/// Nodes never read their own clock when projecting state, so the same event
/// stream always produces the same state on every node.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Default,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
)]
#[serde(transparent)]
pub struct Timestamp(pub i64);

impl Timestamp {
    /// Current wall-clock time. Only clients call this, when authoring events.
    pub fn now() -> Self {
        let d = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        Timestamp(d.as_millis() as i64)
    }
}
