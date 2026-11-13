//! What was played in a match beyond who plays it, see DESIGN.md, *Match details*.

use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use crate::{
    entities::{Annotated, MatchResult, Team},
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

/// Why a decision was reached other than in the usual way, e.g. `walkover` or `retired`: a key
/// a match format declares.
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
/// for it and its parts, as deep as they were entered. Their meaning is the match format's.
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
    pub fn with_value(mut self, field: FieldKey, values: Sides<Option<EnteredValue>>) -> Self {
        self.values.set(field, values);
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
    pub fn value(&self, field: &FieldKey) -> Option<&Sides<Option<EnteredValue>>> {
        self.values.get(field)
    }

    /// The parts of this part, in playing order.
    pub fn parts(&self) -> &[Part] {
        &self.parts
    }

    /// Takes the decision, values and parts of `entered`, keeping the annotations of this part.
    pub(crate) fn enter(&mut self, entered: Part) {
        let annotations = std::mem::take(&mut self.annotations);
        *self = Self {
            annotations,
            ..entered
        };
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

/// The values of a part, by field, at most one entry each, in the order the fields were first
/// entered: for each side, `None` if it has none (an innings is one side's).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct Values(Vec<(FieldKey, Sides<Option<EnteredValue>>)>);

impl Values {
    /// No values.
    pub fn new() -> Self {
        Self::default()
    }

    /// The values of `field`, if any.
    pub fn get(&self, field: &FieldKey) -> Option<&Sides<Option<EnteredValue>>> {
        self.0
            .iter()
            .find(|(key, _)| key == field)
            .map(|(_, values)| values)
    }

    /// Sets the values of `field` to `values`, in the place of any it had, else after the others.
    pub fn set(&mut self, field: FieldKey, values: Sides<Option<EnteredValue>>) {
        match self.0.iter_mut().find(|(key, _)| *key == field) {
            Some((_, old)) => *old = values,
            None => self.0.push((field, values)),
        }
    }

    /// Removes the values of `field`, if it has any.
    pub fn remove(&mut self, field: &FieldKey) {
        self.0.retain(|(key, _)| key != field);
    }

    /// Each field with its values, in order.
    pub fn iter(&self) -> impl Iterator<Item = (&FieldKey, &Sides<Option<EnteredValue>>)> {
        self.0.iter().map(|(key, values)| (key, values))
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl FromIterator<(FieldKey, Sides<Option<EnteredValue>>)> for Values {
    /// The values of each field, the last ones of a field given more than once.
    fn from_iter<I: IntoIterator<Item = (FieldKey, Sides<Option<EnteredValue>>)>>(iter: I) -> Self {
        let mut values = Self::new();
        for (field, sides) in iter {
            values.set(field, sides);
        }
        values
    }
}

impl<'de> Deserialize<'de> for Values {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Vec::deserialize(deserializer)
            .map(Vec::into_iter)
            .map(Self::from_iter)
    }
}

/// The key of a field of a part, e.g. `points` or `yellow-cards`: declared by the match format.
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
pub enum EnteredValue {
    /// A number counted, from 0 to 65535: goals, points, games, cards.
    Count(u16),
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

#[cfg(test)]
mod tests {
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::*;

    #[test]
    fn counts_load_only_within_their_range() {
        assert_eq!(
            serde_json::from_str::<EnteredValue>(r#"{"Count":65535}"#).unwrap(),
            EnteredValue::Count(u16::MAX)
        );
        for invalid in [r#"{"Count":65536}"#, r#"{"Count":4294967295}"#] {
            assert!(serde_json::from_str::<EnteredValue>(invalid).is_err());
        }
    }

    #[test]
    fn values_keep_one_entry_per_field_in_place() {
        let count = |n| Sides::new(Some(EnteredValue::Count(n)), None);
        let (goals, cards) = (FieldKey::new("goals"), FieldKey::new("cards"));
        let mut values = Values::new();
        values.set(goals.clone(), count(1));
        values.set(cards.clone(), count(2));
        values.set(goals.clone(), count(3));

        let fields = |values: &Values| {
            values
                .iter()
                .map(|(key, _)| key.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(fields(&values), [goals.clone(), cards]);
        assert_eq!(values.get(&goals), Some(&count(3)));

        let saved =
            r#"[["goals",{"a":{"Count":1},"b":null}],["goals",{"a":{"Count":3},"b":null}]]"#;
        let loaded: Values = serde_json::from_str(saved).unwrap();
        assert_eq!(loaded.get(&goals), Some(&count(3)));
        assert_eq!(fields(&loaded), [goals]);
    }
}
