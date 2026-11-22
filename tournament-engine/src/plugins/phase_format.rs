use std::num::NonZeroU32;

use dyn_clone::DynClone;
use plugin_factory::{Plugin, axis, compatibility, config::PluginConfig};

use crate::{
    entities::{Bye, DivisionView, NewMatch},
    plugins::{
        Phrase,
        facts::NoDraws,
        metrics_calculator::{MorePoints, MorePointsPerMatch},
    },
    type_map::TypeMap,
};

/// How the matches of a phase are scheduled.
///
/// A format may keep state in `self` and update it in [`Self::create_new_matches`], in a field
/// left out of its schema with `#[plugin(skip)]`. That state is serialized together with the
/// phase, and it must only depend on what the [`DivisionView`]s showed the format and the calls it
/// has seen, so that replaying the transition log recomputes it.
#[axis]
pub trait PhaseFormat: Plugin + DynClone {
    /// Suggested ranking and scoring settings for this configuration. Consumers apply these
    /// when starting a phase or explicitly restoring suggestions; they are not compatibility rules.
    fn suggestions(&self) -> PhaseSuggestions {
        PhaseSuggestions::default()
    }

    /// The matches and byes to schedule next, given everything the phase has played so far. Empty
    /// if there is nothing to schedule, which is how a finished phase is reported.
    ///
    /// Called when the phase starts and again after every reported match result. Scheduling a bye
    /// does not lead to another call.
    ///
    /// `phase` holds the data of the phase and, when asked for, its standings, which count every
    /// result reported so far and every bye reached. A format reading a metric, from the standings
    /// or the data, must require it in its declaration ([`CalculatesMetric`]), as a criterion does.
    ///
    /// An implementation must only schedule rankings of the phase, must not return a match or a
    /// bye whose id already exists in the phase, and must not put the same ranking into two matches
    /// or byes of the same round. Rounds have to order the matches of each ranking by when they are
    /// played, since metrics such as [`RoundOfFirstLoss`] read a later round as a later match.
    ///
    /// A match is numbered in the order it is scheduled, across the competition, unless it asks
    /// for a number with [`NewMatch::with_number`], e.g. its place in a bracket numbered from the
    /// start; a number asked for is relative to the division (see
    /// [`Division::number_base`](crate::Division::number_base)) and must not be one another match
    /// of the division has or asks for, nor above [`Self::highest_number`].
    ///
    /// Anything a consumer needs to know about a match or a bye beyond its round, e.g. the stage of
    /// a bracket it belongs to, is attached to it with [`Annotated::with_annotation`]. Every
    /// annotation a format writes has to be provided as a
    /// [`MatchAnnotation`](crate::plugins::facts::MatchAnnotation) in its declaration, so that the
    /// plugins reading it can require it.
    ///
    /// [`CalculatesMetric`]: crate::plugins::facts::CalculatesMetric
    /// [`RoundOfFirstLoss`]: super::metrics_calculator::round_of_first_loss::RoundOfFirstLoss
    /// [`Annotated::with_annotation`]: crate::entities::Annotated::with_annotation
    fn create_new_matches(&mut self, phase: &DivisionView<'_>) -> Schedule;

    /// The fewest entrants this format schedules matches for, 2 unless it says otherwise; with
    /// fewer, it schedules none. The start is not refused for it, so that a log started with fewer
    /// still replays; a consumer may refuse to start such a phase.
    fn min_entrants(&self) -> usize {
        2
    }

    /// The stage of the phase a match or a bye with `annotations` belongs to, e.g. the semifinals,
    /// or `None` if this format names no stages. Read from the annotations the format wrote on the
    /// match or the bye, see [`Annotated::annotations`].
    ///
    /// [`Annotated::annotations`]: crate::entities::Annotated::annotations
    fn stage(&self, annotations: &TypeMap) -> Option<Stage> {
        let _ = annotations;
        None
    }

    /// The stage that names a round whose matches and byes belong to `stages` (each distinct one
    /// once, at least one), e.g. the final for a round that also has the match for third place;
    /// `None` if no stage names the round. By default the stage all of them are, else `None`.
    fn round_stage(&self, stages: &[Stage]) -> Option<Stage> {
        match stages {
            [only] => Some(only.clone()),
            _ => None,
        }
    }

    /// The bracket the phase is played in, if this format plays one, e.g. a knockout; `None` before
    /// the phase starts. Each pairing is annotated as its match or bye will be, so with its stage.
    fn bracket(&self) -> Option<&Bracket> {
        None
    }

    /// The highest number a match of this format asks for, relative to its division, once it has
    /// scheduled for the first time: the numbers up to it are kept for its matches, and a match
    /// of another division asking for no number is numbered after them. `None` if it asks for
    /// none; by default the highest number of its [`bracket`](Self::bracket), if any.
    fn highest_number(&self) -> Option<NonZeroU32> {
        self.bracket().and_then(Bracket::highest_number)
    }

    /// Whether the order of the rankings a division starts with means something to this format,
    /// as it does to a knockout seeding its bracket by it; `true` unless the format says otherwise,
    /// as one drawing its pairings by lot does, where the order changes no more than which pairings
    /// the lot gives. A consumer need not ask for a [`Seeding`] of a format that reads none.
    ///
    /// [`Seeding`]: crate::plugins::seeding::Seeding
    fn reads_seeding(&self) -> bool {
        true
    }

    /// Whether how many divisions a phase of this format is played in is something anyone
    /// setting up a competition decides, as for a round robin, whose matches grow with the square
    /// of the size of a group, so that splitting it into groups is what keeps it short; `false`
    /// unless the format says otherwise, and then a consumer asks for divisions only from those
    /// who know the formats.
    fn has_basic_divisions(&self) -> bool {
        false
    }
}

dyn_clone::clone_trait_object!(PhaseFormat);

impl dyn PhaseFormat {
    /// Whether a match of this format may end in a [`MatchResult::Draw`]: unless it provides
    /// [`NoDraws`], which a format whose every match has to produce a winner does.
    ///
    /// A draw reported for a format that allows none is rejected, so
    /// [`PhaseFormat::create_new_matches`] never sees one.
    ///
    /// [`MatchResult::Draw`]: crate::entities::MatchResult::Draw
    pub fn allows_draws(&self) -> bool {
        !compatibility::provides_fact::<NoDraws>(self)
    }
}

/// The ranking criteria, in priority order, and metrics suggested for a phase.
/// Each config must belong to the corresponding axis. Suggestions are validated like user choices.
#[derive(Debug, Default)]
pub struct PhaseSuggestions {
    pub ranking: Vec<PluginConfig>,
    pub metrics: Vec<PluginConfig>,
    /// The criteria comparing entrants of the same place in different groups, in priority order,
    /// suggested only for a phase played in several, see
    /// [`PhasePlugins::default_value`](crate::plugins::PhasePlugins::default_value).
    pub across: Vec<PluginConfig>,
}

impl PhaseSuggestions {
    /// The comparison across groups of a format ranking by points: more points per match, which
    /// suits groups of different sizes, then more points.
    pub(crate) fn across_by_points() -> Vec<PluginConfig> {
        vec![
            PluginConfig::of::<MorePointsPerMatch>(),
            PluginConfig::of::<MorePoints>(),
        ]
    }
}

/// The name of a stage of a phase, as a [`PhaseFormat`] names a match or a bye by, see
/// [`PhaseFormat::stage`]: the phrase of its key among the format's stages, e.g. `"final"`.
pub type Stage = Phrase;

/// What a [`PhaseFormat`] schedules at once, see [`PhaseFormat::create_new_matches`].
#[derive(Debug, Default)]
pub struct Schedule {
    pub matches: Vec<NewMatch>,
    pub byes: Vec<Bye>,
}

impl From<Vec<NewMatch>> for Schedule {
    /// Schedules `matches` and no byes.
    fn from(matches: Vec<NewMatch>) -> Self {
        Self {
            matches,
            byes: Vec::new(),
        }
    }
}

mod bracket;
pub mod double_elimination;
pub mod monster_dyp;
pub mod round_robin;
pub mod single_elimination;

pub(crate) use bracket::seeding_order;
pub use bracket::{Advance, Bracket, BracketPairing, BracketSide, PairingId, Seat};
// what an earlier version of a format, see `plugins::deprecated`, builds its bracket with
pub(crate) use bracket::{BracketBuilder, Source, phase_round};
pub use double_elimination::{DoubleElimination, DoubleEliminationRound};
pub use monster_dyp::MonsterDyp;
pub use round_robin::{RoundRobin, random_rounds::RandomRounds};
pub use single_elimination::{KnockoutRound, SingleElimination};

#[cfg(test)]
mod tests {
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::*;

    /// A format allows draws unless it provides that there are none, as the eliminations do.
    #[test]
    fn only_a_format_providing_no_draws_allows_none() {
        let allows = |format: Box<dyn PhaseFormat>| format.allows_draws();

        assert!(allows(Box::new(RoundRobin)));
        assert!(!allows(Box::new(SingleElimination::new())));
        assert!(!allows(Box::new(DoubleElimination::new())));
    }
}
