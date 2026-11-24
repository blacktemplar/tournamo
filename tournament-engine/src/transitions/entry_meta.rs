//! What the log records about each of its entries besides what it does.

use serde::{Deserialize, Serialize};

#[cfg(doc)]
use crate::CompetitionStateMachine;

/// A point in time, in UTC: the milliseconds since the Unix epoch.
///
/// Serialized as that number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Timestamp(i64);

impl Timestamp {
    /// The point in time `millis` milliseconds after the Unix epoch (1970-01-01T00:00:00Z),
    /// before it if negative.
    #[must_use]
    pub const fn from_unix_millis(millis: i64) -> Self {
        Self(millis)
    }

    /// The milliseconds since the Unix epoch (1970-01-01T00:00:00Z), negative before it.
    #[must_use]
    pub const fn unix_millis(self) -> i64 {
        self.0
    }
}

/// What the log of a [`CompetitionStateMachine`] records about each entry, undo entries included:
/// when it was appended. It plays no part in replaying the log.
///
/// Serialized as `{"at": <timestamp or null>}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntryMeta {
    /// `None` for an entry appended before times were recorded.
    at: Option<Timestamp>,
}

impl EntryMeta {
    /// An entry appended at `at`, by the clock of the device appending it.
    #[must_use]
    pub const fn recorded_at(at: Timestamp) -> Self {
        Self { at: Some(at) }
    }

    /// When the entry was appended, by the clock of the device appending it: the order of the log
    /// is the order of what happened, which these times need not follow. `None` if the entry was
    /// appended before times were recorded.
    #[must_use]
    pub const fn at(&self) -> Option<Timestamp> {
        self.at
    }
}

/// An entry of no known time, for tests, which mostly do not care when an entry was appended.
#[cfg(feature = "test-util")]
impl Default for EntryMeta {
    fn default() -> Self {
        Self { at: None }
    }
}
