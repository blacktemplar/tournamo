//! The first version of the double elimination, see [`DoubleEliminationV1`]. A frozen copy: never
//! changed, so that what was saved with it keeps replaying as it was played.

use std::num::NonZeroU32;

use plugin_factory::{Plugin, compatibility::Declaration, config::PluginConfig, register};
use serde::{Deserialize, Serialize};

use crate::{
    entities::{DivisionData, DivisionView, Team},
    plugins::{
        facts::{MatchAnnotation, NoDraws},
        metrics_calculator::{
            round_of_elimination::{Eliminates, LaterElimination, RoundOfEliminationCalculator},
            wins_draws_losses::{FewerLosses, WinsDrawsLossesCalculator},
        },
        phase_format::{
            Bracket, BracketBuilder, DoubleEliminationRound, PhaseFormat, PhaseSuggestions,
            Schedule, Source, Stage, phase_round,
        },
    },
    type_map::TypeMap,
};

/// The first version of [`DoubleElimination`], which competitions saved with it keep playing. It
/// plays as the latest does, except that the losers of the winner bracket drop into the loser
/// bracket reversed and as they come, alternately, which from 17 entrants on lets entrants meet
/// again sooner than they need to.
///
/// [`DoubleElimination`]: crate::plugins::phase_format::DoubleElimination
#[derive(Debug, Clone, Deserialize, Serialize, Plugin)]
#[cfg_attr(feature = "test-util", derive(Default))]
#[plugin(
    id = "tournament_engine::double_elimination",
    label = "double elimination",
    description = "an entrant is out after its second loss, draws are not allowed",
    declare_static = Self::declare_static
)]
pub struct DoubleEliminationV1 {
    #[plugin(
        label = "grand final reset",
        description = "if the champion of the winner bracket loses the grand final, the two play \
                       it once more; without it losing the grand final knocks out either side",
        default = true
    )]
    grand_final_reset: bool,
    /// Built from the seeds on the first call.
    #[serde(default)]
    #[plugin(skip)]
    bracket: Option<Bracket>,
}

impl DoubleEliminationV1 {
    /// A double elimination whose grand final is a single match: whoever loses it is out, even the
    /// champion of the winner bracket with its first loss.
    ///
    /// Requires the `test-util` feature.
    #[cfg(feature = "test-util")]
    pub const fn new() -> Self {
        Self {
            grand_final_reset: false,
            bracket: None,
        }
    }

    /// Returns this double elimination with the champion of the winner bracket also needing to
    /// lose twice: if it loses the grand final, the two play it once more.
    ///
    /// Requires the `test-util` feature.
    #[cfg(feature = "test-util")]
    pub const fn with_grand_final_reset(mut self) -> Self {
        self.grand_final_reset = true;
        self
    }

    fn declare_static(declaration: &mut Declaration) {
        // every match has to produce a winner who moves on
        declaration
            .provide(MatchAnnotation::<DoubleEliminationRound>::new())
            .provide(MatchAnnotation::<Eliminates>::new())
            .provide(NoDraws);
    }
}

#[register]
impl PhaseFormat for DoubleEliminationV1 {
    fn suggestions(&self) -> PhaseSuggestions {
        PhaseSuggestions {
            ranking: vec![
                PluginConfig::of::<LaterElimination>(),
                PluginConfig::of::<FewerLosses>(),
            ],
            metrics: vec![
                PluginConfig::of::<RoundOfEliminationCalculator>(),
                PluginConfig::of::<WinsDrawsLossesCalculator>(),
            ],
            across: Vec::new(),
        }
    }

    fn create_new_matches(&mut self, phase: &DivisionView<'_>) -> Schedule {
        let phase_data = phase.data();
        let reset = self.grand_final_reset;
        self.bracket
            .get_or_insert_with(|| bracket(phase_data, reset))
            .create_new_matches(phase_data)
    }

    fn stage(&self, annotations: &TypeMap) -> Option<Stage> {
        annotations
            .get::<DoubleEliminationRound>()
            .copied()
            .map(Stage::from)
    }

    /// The bracket, with the stage of each pairing as a [`DoubleEliminationRound`] annotation.
    fn bracket(&self) -> Option<&Bracket> {
        self.bracket.as_ref()
    }
}

/// The whole double elimination between the rankings of `phase_data`, see [`DoubleEliminationV1`].
fn bracket(phase_data: &DivisionData, grand_final_reset: bool) -> Bracket {
    let mut bracket = BracketBuilder::default();
    let winners = bracket.add_knockout(phase_data, winner_bracket_slot, |round, rounds| {
        TypeMap::new().with(DoubleEliminationRound::Winners { round, rounds })
    });
    let Some(winner_bracket_final) = winners.last() else {
        return bracket.build();
    };
    let winner_rounds = winners.len() as u32;
    let loser_rounds = 2 * winner_rounds - 2;

    let mut losers: Vec<Source> = winners[0].iter().copied().map(Source::LoserOf).collect();
    for round in 1..=loser_rounds {
        let pairs: Vec<[Source; 2]> = if round % 2 == 1 {
            losers.as_chunks().0.to_vec()
        } else {
            // the losers of the next round of the winner bracket drop in
            let drop_in = round / 2;
            let mut dropping: Vec<_> = winners[drop_in as usize]
                .iter()
                .copied()
                .map(Source::LoserOf)
                .collect();
            if drop_in % 2 == 1 {
                dropping.reverse();
            }
            losers
                .into_iter()
                .zip(dropping)
                .map(<[_; 2]>::from)
                .collect()
        };
        let stage = DoubleEliminationRound::Losers {
            round,
            rounds: loser_rounds,
        };
        losers = pairs
            .into_iter()
            .map(|sources| {
                let annotations = TypeMap::new().with(stage).with(Eliminates::Loser);
                Source::WinnerOf(bracket.add_pairing(phase_round(round + 1), sources, annotations))
            })
            .collect();
    }

    let grand_final_slot = phase_round(2 * winner_rounds);
    let grand_final = bracket.add_pairing(
        grand_final_slot,
        [Source::WinnerOf(winner_bracket_final[0]), losers[0]],
        TypeMap::new()
            .with(DoubleEliminationRound::GrandFinal)
            .with(if grand_final_reset {
                Eliminates::Only(Team::B)
            } else {
                Eliminates::Loser
            }),
    );
    if grand_final_reset {
        // only played if the champion of the loser bracket, side B, won the grand final, and then
        // between the same two in the same order
        bracket.add_pairing(
            grand_final_slot.saturating_add(1),
            [Source::LoserOf(grand_final), Source::WinnerOf(grand_final)],
            TypeMap::new()
                .with(DoubleEliminationRound::GrandFinalReset)
                .with(Eliminates::Loser),
        );
        bracket.advance_only_if_won_by(grand_final, Team::B);
    }
    bracket.build()
}

/// The phase round that round `round` of the winner bracket is played in, see the table of
/// [`DoubleElimination`]. Round `i` of the loser bracket is played in phase round `i + 1`.
///
/// [`DoubleElimination`]: crate::plugins::phase_format::DoubleElimination
fn winner_bracket_slot(round: u32) -> NonZeroU32 {
    phase_round(if round == 1 { 1 } else { 2 * round - 2 })
}
