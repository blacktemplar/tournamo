//! The match format of a phase: what is entered for its matches and what it means, see DESIGN.md,
//! *Match details*.

use dyn_clone::DynClone;
use plugin_factory::{Plugin, axis, register};
use serde::{Deserialize, Serialize};
use thiserror::Error;

mod best_of;
mod scored;
mod tallies;

pub use best_of::BestOf;
pub use scored::Scored;
pub use tallies::{GamesWon, PointsScored};

use crate::{
    entities::{
        Match, MatchResult, PhaseData,
        details::{Decision, EnteredValue, FieldKey, Part, ResultReason},
    },
    plugins::Phrase,
    type_map::TypeMap,
};

/// What is entered for the matches of a phase, and what it means.
///
/// The engine trusts what is reported: a match format describes what a result looks like, so that a
/// consumer can show and enter it, and refuse only what does not fit that description, never a
/// result breaking the rules of a sport.
#[axis]
pub trait MatchFormat: Plugin + DynClone {
    /// What a result of `match` looks like. May depend on the match, e.g. on its stage.
    fn template(&self, phase_data: &PhaseData, r#match: &Match) -> Template;

    /// What the metrics read of `match`, which has just been decided: its tallies, e.g. the points
    /// each side scored, from its [contest](Match::contest). Each tally is declared as a
    /// [`MatchTally`](crate::plugins::facts::MatchTally) the match format provides.
    ///
    /// The contest fits the [`template`](Self::template), but is not checked against the rules of
    /// any sport: a tally takes what was entered, as it was entered.
    fn tally(&self, phase_data: &PhaseData, r#match: &Match) -> TypeMap;

    /// The result the values and parts of `part`, at `level` of the template, suggest, for a
    /// consumer filling in a result: `None` if they suggest none, e.g. since they are missing.
    /// The parts of `part` are decided, as far as they suggest.
    fn suggest(&self, level: &Level, part: &Part) -> Option<MatchResult>;
}

dyn_clone::clone_trait_object!(MatchFormat);

/// What a result of a match looks like: the match as its root level.
#[derive(Debug, Clone, PartialEq)]
pub struct Template {
    pub root: Level,
}

/// What a part of a match at one level looks like: a match, a game, a set, a rubber, a leg.
#[derive(Debug, Clone, PartialEq)]
pub struct Level {
    /// What a part at this level is called, e.g. `game`.
    pub name: Phrase,
    /// The values entered for a part at this level, in the order they are entered.
    pub fields: Vec<FieldSpec>,
    /// Why a part at this level may be decided other than in the usual way.
    pub reasons: Vec<ReasonSpec>,
    /// The parts a part at this level is played in.
    pub parts: Parts,
}

impl Level {
    /// A level called `name` with no fields, reasons or parts.
    pub fn new(name: Phrase) -> Self {
        Self {
            name,
            fields: Vec::new(),
            reasons: Vec::new(),
            parts: Parts::None,
        }
    }

    /// The field of `key`, if this level has one.
    pub fn field(&self, key: &FieldKey) -> Option<&FieldSpec> {
        self.fields.iter().find(|field| field.key == *key)
    }

    /// Checks that what was entered for `part`, a part at this level, fits it: its values are of
    /// fields of this level and of their kinds, its reason is one of this level, and each of its
    /// parts fits the level of its place.
    ///
    /// # Errors
    /// Returns what does not fit first.
    pub fn check(&self, part: &Part) -> Result<(), Misfit> {
        if let Some(reason) = part.decision().and_then(Decision::reason)
            && !self.reasons.iter().any(|spec| spec.reason == *reason)
        {
            return Err(Misfit::UnknownReason(reason.clone()));
        }
        for (key, sides) in part.values().iter() {
            let field = self
                .field(key)
                .ok_or_else(|| Misfit::UnknownField(key.clone()))?;
            if [sides.a, sides.b]
                .into_iter()
                .flatten()
                .any(|value| !field.kind.admits(value))
            {
                return Err(Misfit::WrongKind(key.clone()));
            }
        }
        for (index, part) in part.parts().iter().enumerate() {
            self.level_of_part(index)
                .ok_or(Misfit::TooManyParts)?
                .check(part)?;
        }
        Ok(())
    }

    /// The level of the part at `index` (from 0) of a part at this level, if it can have one.
    pub fn level_of_part(&self, index: usize) -> Option<&Level> {
        match &self.parts {
            Parts::None => None,
            Parts::Same { level, max, .. } => (index < *max as usize).then_some(&**level),
            Parts::Each(levels) => levels.get(index),
        }
    }
}

/// What does not fit the [`Template`] of a match in what was entered for it.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Misfit {
    #[error("there is no field {}", .0.key())]
    UnknownField(FieldKey),
    #[error("{} takes another kind of value", .0.key())]
    WrongKind(FieldKey),
    #[error("{} is no reason for a decision", .0.key())]
    UnknownReason(ResultReason),
    #[error("there are more parts than are played")]
    TooManyParts,
}

/// The parts a part is played in.
#[derive(Debug, Clone, PartialEq)]
pub enum Parts {
    /// None: the part is not divided.
    None,
    /// Up to `max` parts at the same `level`, e.g. the games of a best of three; `min` of them,
    /// at most `max`, are the fewest that may decide the part when played, e.g. two of a best of
    /// three. Fewer are taken: a part may be decided otherwise, e.g. by a walkover.
    Same {
        level: Box<Level>,
        min: u32,
        max: u32,
    },
    /// One part at each of the `levels`, in order, e.g. the rubbers of a league fixture.
    Each(Vec<Level>),
}

/// A value entered for each side of a part.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldSpec {
    pub key: FieldKey,
    pub label: Phrase,
    pub kind: ValueKind,
    /// What is likely, to warn of a typo; never enforced.
    pub hints: Hints,
}

/// What kind of [`EnteredValue`] a field takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueKind {
    /// [`EnteredValue::Count`].
    Count,
    /// [`EnteredValue::Decimal`].
    Decimal,
}

impl ValueKind {
    /// Whether `value` is of this kind.
    pub fn admits(self, value: EnteredValue) -> bool {
        matches!(
            (self, value),
            (Self::Count, EnteredValue::Count(_)) | (Self::Decimal, EnteredValue::Decimal(_))
        )
    }
}

/// What the values of a field are likely to be: a consumer warns of others, never refuses them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Hints {
    /// The count the winner of a part usually has, or more than it, then leading by one or two:
    /// e.g. 21–17, 22–20 or 30–29 in a game to 21.
    pub winning_score: Option<u16>,
}

/// A reason a part may be decided for other than in the usual way.
#[derive(Debug, Clone, PartialEq)]
pub struct ReasonSpec {
    pub reason: ResultReason,
    pub label: Phrase,
}

/// The match format of a phase that does not choose one: [`WinnerOnly`].
pub(crate) fn default() -> Box<dyn MatchFormat> {
    Box::new(WinnerOnly)
}

/// The match format of a match decided by its winner alone, or drawn, with nothing else entered.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::winner_only",
    label = "winner only",
    description = "who won a match, or that it was drawn, without a score"
)]
pub struct WinnerOnly;

#[register]
impl MatchFormat for WinnerOnly {
    fn template(&self, _: &PhaseData, _: &Match) -> Template {
        Template {
            root: Level::new(Phrase::new("match", "match")),
        }
    }

    fn tally(&self, _: &PhaseData, _: &Match) -> TypeMap {
        TypeMap::new()
    }

    fn suggest(&self, _: &Level, _: &Part) -> Option<MatchResult> {
        None
    }
}
