use std::{
    fmt::{self, Display, Formatter},
    num::NonZeroU32,
};

use plugin_factory::{Plugin, compatibility::Declaration, config::PluginConfig, register};
use serde::{Deserialize, Serialize};

use crate::{
    entities::{PhaseData, PhaseView, Team},
    plugins::{
        facts::{MatchAnnotation, NoDraws},
        metrics_calculator::{
            round_of_elimination::{Eliminates, LaterElimination, RoundOfEliminationCalculator},
            wins_draws_losses::{FewerLosses, WinsDrawsLossesCalculator},
        },
        phase_format::{
            PhaseFormat, PhaseSuggestions, Schedule, Stage,
            bracket::{Bracket, BracketBuilder, Source, phase_round},
        },
    },
    type_map::{TypeMap, TypeMapValue},
};

/// A knockout in which an entrant is only out after its second loss: the winner bracket plays like
/// a single elimination, and whoever loses there drops into the loser bracket. The champions of
/// both brackets meet in the grand final.
///
/// The loser bracket takes in the losers of every round of the winner bracket. So it plays two
/// rounds for every round of the winner bracket after the first: one in which its own survivors
/// play each other, and one in which they meet the entrants that just dropped down. In a full
/// bracket of `2^k` entrants that is `k` rounds of the winner bracket and `2k - 2` of the loser
/// bracket. The rounds of a phase interleave them, so that every entrant plays its matches in the
/// order of their rounds:
///
/// | round          | 1 | 2 | 3 | 4 | 5 | … | 2k          | 2k + 1 |
/// |----------------|---|---|---|---|---|---|-------------|--------|
/// | winner bracket | 1 | 2 |   | 3 |   | … |             |        |
/// | loser bracket  |   | 1 | 2 | 3 | 4 | … |             |        |
/// | grand final    |   |   |   |   |   |   | grand final | reset  |
///
/// A match is scheduled as soon as both of its entrants are known, without waiting for the rest of
/// its round. Seeding follows the initial ranks: in the first round the best seed plays the worst,
/// and the best seeds get the byes of a bracket that is not full, as in a [`SingleElimination`].
/// Entrants dropping down from the winner bracket enter the loser bracket in reversed and in
/// original order, alternately, which keeps those apart that met in the winner bracket for longer.
///
/// Every match is annotated with its [`DoubleEliminationRound`] and every match that can knock an
/// entrant out with [`Eliminates`]. Draws are not allowed.
///
/// [`SingleElimination`]: super::single_elimination::SingleElimination
#[derive(Debug, Clone, Deserialize, Serialize, Plugin)]
#[cfg_attr(feature = "test-util", derive(Default))]
#[plugin(
    id = "tournament_engine::double_elimination",
    label = "double elimination",
    description = "an entrant is out after its second loss, draws are not allowed",
    declare_static = Self::declare_static
)]
pub struct DoubleElimination {
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

impl DoubleElimination {
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

    /// The bracket of the double elimination, with the stage of each pairing as a
    /// [`DoubleEliminationRound`] annotation; `None` before the phase starts.
    pub fn bracket(&self) -> Option<&Bracket> {
        self.bracket.as_ref()
    }

    fn declare_static(declaration: &mut Declaration) {
        // every match has to produce a winner who moves on
        declaration
            .provide(MatchAnnotation::<DoubleEliminationRound>::new())
            .provide(MatchAnnotation::<Eliminates>::new())
            .provide(NoDraws);
    }
}

/// The stage of a [`DoubleElimination`] a match belongs to, attached to every match it schedules as
/// an annotation.
///
/// # Examples
/// ```
/// # use tournament_engine::{Annotated, Match, plugins::phase_format::DoubleEliminationRound};
/// fn stage_name(r#match: &Match) -> String {
///     match r#match.annotation::<DoubleEliminationRound>() {
///         Some(DoubleEliminationRound::Winners { round, rounds }) if round == rounds => {
///             "winner bracket final".to_owned()
///         }
///         Some(DoubleEliminationRound::Winners { round, .. }) => format!("winners {round}"),
///         Some(DoubleEliminationRound::Losers { round, .. }) => format!("losers {round}"),
///         Some(DoubleEliminationRound::GrandFinal) => "grand final".to_owned(),
///         Some(DoubleEliminationRound::GrandFinalReset) => "grand final reset".to_owned(),
///         None => format!("round {}", r#match.round()),
///     }
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DoubleEliminationRound {
    /// Round `round` of the `rounds` rounds of the winner bracket, the last one being its final.
    Winners { round: u32, rounds: u32 },
    /// Round `round` of the `rounds` rounds of the loser bracket, the last one being its final.
    Losers { round: u32, rounds: u32 },
    /// The match between the champions of both brackets.
    GrandFinal,
    /// The grand final played once more, because the champion of the winner bracket lost it, if
    /// the double elimination is configured with a grand final reset.
    GrandFinalReset,
}

#[typetag::serde(name = "tournament_engine::double_elimination_round")]
impl TypeMapValue for DoubleEliminationRound {}

/// Shows the round by its bracket, e.g. `Winners round 2`, `Losers final` or `Grand final`.
impl Display for DoubleEliminationRound {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&Stage::from(*self), f)
    }
}

/// The stage named as [`Display`] shows it, with the keys `winners-round` and `losers-round`, each
/// with the number `round`, `winners-final`, `losers-final`, `grand-final` and
/// `grand-final-reset`.
impl From<DoubleEliminationRound> for Stage {
    fn from(round: DoubleEliminationRound) -> Self {
        match round {
            DoubleEliminationRound::Winners { round, rounds } if round == rounds => {
                Self::new("winners-final", "Winners final")
            }
            DoubleEliminationRound::Winners { round, .. } => {
                Self::new("winners-round", format!("Winners round {round}")).with("round", round)
            }
            DoubleEliminationRound::Losers { round, rounds } if round == rounds => {
                Self::new("losers-final", "Losers final")
            }
            DoubleEliminationRound::Losers { round, .. } => {
                Self::new("losers-round", format!("Losers round {round}")).with("round", round)
            }
            DoubleEliminationRound::GrandFinal => Self::new("grand-final", "Grand final"),
            DoubleEliminationRound::GrandFinalReset => {
                Self::new("grand-final-reset", "Grand final reset")
            }
        }
    }
}

#[register]
impl PhaseFormat for DoubleElimination {
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
        }
    }

    fn create_new_matches(&mut self, phase: &PhaseView<'_>) -> Schedule {
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
}

/// The whole double elimination between the rankings of `phase_data`, see [`DoubleElimination`].
fn bracket(phase_data: &PhaseData, grand_final_reset: bool) -> Bracket {
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
fn winner_bracket_slot(round: u32) -> NonZeroU32 {
    phase_round(if round == 1 { 1 } else { 2 * round - 2 })
}

#[cfg(test)]
mod tests;
