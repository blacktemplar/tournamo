//! What was played in a match beyond who plays it: its decision, see DESIGN.md, *Match details*.

use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use crate::entities::competition::MatchResult;

/// Who won a match, or a part of one, and why, if not in the usual way.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Decision {
    result: MatchResult,
    reason: Option<ResultReason>,
}

impl Decision {
    /// A decision for `result`, reached in the usual way.
    pub fn new(result: MatchResult) -> Self {
        Self {
            result,
            reason: None,
        }
    }

    /// This decision, reached for `reason`.
    #[must_use]
    pub fn with_reason(mut self, reason: ResultReason) -> Self {
        self.reason = Some(reason);
        self
    }

    pub fn result(&self) -> MatchResult {
        self.result
    }

    /// Why the decision was reached, if not in the usual way.
    pub fn reason(&self) -> Option<&ResultReason> {
        self.reason.as_ref()
    }
}

impl From<MatchResult> for Decision {
    fn from(result: MatchResult) -> Self {
        Self::new(result)
    }
}

/// Why a decision was reached other than in the usual way, e.g. `walkover` or `retired`: a key the
/// match rules declare.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ResultReason(Cow<'static, str>);

impl ResultReason {
    pub const fn new(key: &'static str) -> Self {
        Self(Cow::Borrowed(key))
    }

    pub fn key(&self) -> &str {
        &self.0
    }
}
