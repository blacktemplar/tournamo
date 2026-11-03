use std::any::Any;

use dyn_clone::DynClone;
use plugin_factory::{Plugin, axis, compatibility};

use crate::{
    entities::competition::{Bye, NewMatch, PhaseData},
    plugins::{Phrase, facts::NoDraws},
    type_map::TypeMap,
};

/// How the matches of a phase are scheduled.
///
/// A format may keep state in `self` and update it in [`Self::create_new_matches`], in a field
/// left out of its schema with `#[plugin(skip)]`. That state is serialized together with the
/// phase, and it must only depend on the [`PhaseData`] and the calls the format has seen, so that
/// replaying the transition log recomputes it.
///
/// [`Any`] is a supertrait, so that a consumer can downcast a format it knows to its type, e.g. to
/// read the [`Bracket`] of a [`SingleElimination`].
#[axis]
pub trait PhaseFormat: Plugin + DynClone + Any {
    /// The matches and byes to schedule next, given everything the phase has played so far. Empty
    /// if there is nothing to schedule, which is how a finished phase is reported.
    ///
    /// Called when the phase starts and again after every reported match result. Scheduling a bye
    /// does not lead to another call.
    ///
    /// An implementation must only schedule rankings of `phase_data`, must not return a match or a
    /// bye whose id already exists in the phase, and must not put the same ranking into two matches
    /// or byes of the same round. Rounds have to order the matches of each ranking by when they are
    /// played, since metrics such as [`RoundOfFirstLoss`] read a later round as a later match.
    ///
    /// A match is numbered in the order it is scheduled unless it asks for a number with
    /// [`NewMatch::with_number`], e.g. its place in a bracket numbered from the start; a number
    /// asked for must not be one another match of the phase has or asks for.
    ///
    /// Anything a consumer needs to know about a match or a bye beyond its round, e.g. the stage of
    /// a bracket it belongs to, is attached to it with [`Annotated::with_annotation`]. Every
    /// annotation a format writes has to be provided as a
    /// [`MatchAnnotation`](crate::plugins::facts::MatchAnnotation) in its declaration, so that the
    /// plugins reading it can require it.
    ///
    /// [`RoundOfFirstLoss`]: super::metrics_calculator::round_of_first_loss::RoundOfFirstLoss
    /// [`Annotated::with_annotation`]: crate::entities::competition::Annotated::with_annotation
    fn create_new_matches(&mut self, phase_data: &PhaseData) -> Schedule;

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
    /// [`Annotated::annotations`]: crate::entities::competition::Annotated::annotations
    fn stage(&self, annotations: &TypeMap) -> Option<Stage> {
        let _ = annotations;
        None
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
    /// [`MatchResult::Draw`]: crate::entities::competition::MatchResult::Draw
    pub fn allows_draws(&self) -> bool {
        !compatibility::provides_fact::<NoDraws>(self)
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

pub use bracket::{Advance, Bracket, BracketPairing, BracketSide, PairingId, Seat};
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
