//! What was played in a match beyond who plays it, see DESIGN.md, *Match details*.

use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use crate::{
    entities::competition::{Annotated, MatchResult, Team},
    score::Score,
    type_map::TypeMap,
};

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

/// A match, or a game, set, rubber or leg of one: how it was decided, if it was, the values entered
/// for it and its parts, as deep as they were entered. Their meaning is the match rules'.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Part {
    decision: Option<Decision>,
    values: Values,
    parts: Vec<Part>,
    annotations: TypeMap,
}

impl Part {
    /// A part without a decision, values, parts or annotations.
    pub fn new() -> Self {
        Self::default()
    }

    /// This part, decided by `decision`.
    #[must_use]
    pub fn with_decision(mut self, decision: impl Into<Decision>) -> Self {
        self.decision = Some(decision.into());
        self
    }

    /// This part with `values` for `field`, replacing any it had.
    #[must_use]
    pub fn with_value(mut self, field: FieldKey, values: Sides<Option<Value>>) -> Self {
        match self.values.iter_mut().find(|(key, _)| *key == field) {
            Some((_, old)) => *old = values,
            None => self.values.push((field, values)),
        }
        self
    }

    /// This part with `part` as its last part.
    #[must_use]
    pub fn with_part(mut self, part: Part) -> Self {
        self.parts.push(part);
        self
    }

    /// How this part was decided, or `None` if it was not (yet), or has no winner of its own.
    pub fn decision(&self) -> Option<&Decision> {
        self.decision.as_ref()
    }

    /// The values entered for this part, by field, in the order they were entered.
    pub fn values(&self) -> &Values {
        &self.values
    }

    /// The values entered for `field`, if any.
    pub fn value(&self, field: &str) -> Option<&Sides<Option<Value>>> {
        self.values
            .iter()
            .find(|(key, _)| key.key() == field)
            .map(|(_, values)| values)
    }

    /// The parts of this part, in playing order.
    pub fn parts(&self) -> &[Part] {
        &self.parts
    }

    pub(crate) fn decide(&mut self, decision: Decision, values: Values, parts: Vec<Part>) {
        self.decision = Some(decision);
        self.values = values;
        self.parts = parts;
    }
}

impl Annotated for Part {
    fn annotations(&self) -> &TypeMap {
        &self.annotations
    }

    fn annotations_mut(&mut self) -> &mut TypeMap {
        &mut self.annotations
    }
}

/// The values of a part, by field: for each side, `None` if it has none (an innings is one
/// side's).
pub type Values = Vec<(FieldKey, Sides<Option<Value>>)>;

/// The key of a field of a part, e.g. `points` or `yellow-cards`: declared by the match rules.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FieldKey(Cow<'static, str>);

impl FieldKey {
    pub const fn new(key: &'static str) -> Self {
        Self(Cow::Borrowed(key))
    }

    pub fn key(&self) -> &str {
        &self.0
    }
}

/// A value entered for a side of a part.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Value {
    /// A number counted: goals, points, games, cards.
    Count(u32),
    /// A number with a fraction: a judged score, victory points.
    Decimal(Score),
}

/// One of a kind for each side of a match.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Sides<T> {
    pub a: T,
    pub b: T,
}

impl<T> Sides<T> {
    pub const fn new(a: T, b: T) -> Self {
        Self { a, b }
    }

    /// The one of `team`.
    pub fn of(&self, team: Team) -> &T {
        match team {
            Team::A => &self.a,
            Team::B => &self.b,
        }
    }
}
