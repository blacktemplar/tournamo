use std::fmt::{self, Display, Formatter};

use plugin_factory::{Plugin, compatibility::Declaration, config::PluginConfig, register};
use serde::{Deserialize, Serialize};

use crate::{
    entities::{PhaseData, PhaseView},
    plugins::{
        facts::{MatchAnnotation, NoDraws},
        metrics_calculator::{
            round_of_first_loss::{LaterFirstLoss, RoundOfFirstLossCalculator},
            wins_draws_losses::{FewerLosses, WinsDrawsLossesCalculator},
        },
        phase_format::{
            PhaseFormat, PhaseSuggestions, Schedule, Stage,
            bracket::{Bracket, BracketBuilder, Source, phase_round},
        },
    },
    type_map::{TypeMap, TypeMapValue},
};

/// A knockout: the loser of a match is out, so every round halves the field until the final.
///
/// A match is scheduled as soon as both of its entrants are known, without waiting for the rest of
/// its round. Seeding follows the initial ranks: in the first round the best seed plays the worst,
/// the best two seeds can meet in the final at the earliest, and where the field is not a power of
/// two the best seeds get the byes.
///
/// Optionally the two entrants knocked out in the semifinals play one more match, for third place,
/// scheduled in the same round as the final. Both of them lost their first match in the semifinal
/// whichever way it goes, so [`LaterFirstLoss`] ties them and the ranking needs a further criterion
/// to tell the two apart, e.g. [`FewerLosses`]. A bracket of three entrants has nobody to play the
/// match against and so is not given it.
#[derive(Debug, Clone, Deserialize, Serialize, Plugin)]
#[cfg_attr(feature = "test-util", derive(Default))]
#[plugin(
    id = "tournament_engine::single_elimination",
    label = "single elimination",
    description = "the loser of a match is out, draws are not allowed",
    declare_static = Self::declare_static
)]
pub struct SingleElimination {
    #[plugin(
        label = "match for third place",
        description = "the two entrants knocked out in the semifinals play it alongside the \
                       final; without it they stay third equal",
        default = true
    )]
    third_place_match: bool,
    /// Built from the seeds on the first call.
    #[serde(default)]
    #[plugin(skip)]
    bracket: Option<Bracket>,
}

impl SingleElimination {
    /// A knockout that ends with the final, which leaves the two entrants knocked out in the
    /// semifinals as far as the bracket takes them, i.e. third equal.
    ///
    /// Requires the `test-util` feature.
    #[cfg(feature = "test-util")]
    pub const fn new() -> Self {
        Self {
            third_place_match: false,
            bracket: None,
        }
    }

    /// Returns this knockout with the two entrants knocked out in the semifinals playing for third
    /// place.
    ///
    /// Requires the `test-util` feature.
    #[cfg(feature = "test-util")]
    pub const fn with_third_place_match(mut self) -> Self {
        self.third_place_match = true;
        self
    }

    /// The bracket of the knockout, with the stage of each pairing as a [`KnockoutRound`]
    /// annotation; `None` before the phase starts.
    pub fn bracket(&self) -> Option<&Bracket> {
        self.bracket.as_ref()
    }

    fn declare_static(declaration: &mut Declaration) {
        // every match has to produce a winner who moves on to the next round
        declaration
            .provide(MatchAnnotation::<KnockoutRound>::new())
            .provide(NoDraws);
    }
}

/// The stage of a [`SingleElimination`] knockout a match belongs to, attached to every match it
/// schedules as an annotation.
///
/// # Examples
/// ```
/// # use tournament_engine::{Annotated, Match, plugins::phase_format::KnockoutRound};
/// fn stage_name(r#match: &Match) -> String {
///     match r#match.annotation::<KnockoutRound>() {
///         Some(KnockoutRound::RoundOf(2)) => "final".to_owned(),
///         Some(KnockoutRound::RoundOf(4)) => "semifinal".to_owned(),
///         Some(KnockoutRound::RoundOf(n)) => format!("round of {n}"),
///         Some(KnockoutRound::ThirdPlace) => "match for third place".to_owned(),
///         None => format!("round {}", r#match.round()),
///     }
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KnockoutRound {
    /// The round that leaves this many entrants in a full bracket: 2 is the final, 4 the
    /// semifinals, 8 the quarterfinals and so on. A bye counts as a match won, so the first round
    /// of six entrants is the round of 8.
    RoundOf(u32),
    /// The match for third place, if the knockout is configured with one.
    ThirdPlace,
}

#[typetag::serde(name = "tournament_engine::knockout_round")]
impl TypeMapValue for KnockoutRound {}

impl KnockoutRound {
    /// Which fraction of the final this round is: 1 for the final, 2 for the semifinals, 4 for the
    /// quarterfinals and so on. `None` for the match for third place, which is no such fraction.
    ///
    /// # Examples
    /// ```
    /// # use tournament_engine::plugins::phase_format::KnockoutRound;
    /// assert_eq!(KnockoutRound::RoundOf(16).fraction_of_final(), Some(8));
    /// assert_eq!(KnockoutRound::ThirdPlace.fraction_of_final(), None);
    /// ```
    pub const fn fraction_of_final(self) -> Option<u32> {
        match self {
            Self::RoundOf(entrants) => Some(entrants / 2),
            Self::ThirdPlace => None,
        }
    }
}

/// Shows the round by its fraction of the final, e.g. `Final`, `Quarterfinals`, `Round of 32` (the
/// sixteenth-finals, of 32 entrants), or `Third place`.
impl Display for KnockoutRound {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&Stage::from(*self), f)
    }
}

/// The stage named as [`Display`] shows it, with the keys `third-place`, `final`, `semifinals`,
/// `quarterfinals`, and `fraction-finals` with the number `fraction` for the others, e.g. 8 for the
/// eighth-finals.
impl From<KnockoutRound> for Stage {
    fn from(round: KnockoutRound) -> Self {
        match round.fraction_of_final() {
            None => Self::new("third-place", "Third place"),
            Some(1) => Self::new("final", "Final"),
            Some(2) => Self::new("semifinals", "Semifinals"),
            Some(4) => Self::new("quarterfinals", "Quarterfinals"),
            Some(fraction) => Self::new("fraction-finals", format!("Round of {}", 2 * fraction))
                .with("fraction", fraction),
        }
    }
}

#[register]
impl PhaseFormat for SingleElimination {
    fn suggestions(&self) -> PhaseSuggestions {
        let mut ranking = vec![PluginConfig::of::<LaterFirstLoss>()];
        // Both semifinal losers have the same first-loss round; losses distinguish third place.
        if self.third_place_match {
            ranking.push(PluginConfig::of::<FewerLosses>());
        }
        PhaseSuggestions {
            ranking,
            metrics: vec![
                PluginConfig::of::<RoundOfFirstLossCalculator>(),
                PluginConfig::of::<WinsDrawsLossesCalculator>(),
            ],
        }
    }

    fn create_new_matches(&mut self, phase: &PhaseView<'_>) -> Schedule {
        let phase_data = phase.data();
        let third_place_match = self.third_place_match;
        self.bracket
            .get_or_insert_with(|| bracket(phase_data, third_place_match))
            .create_new_matches(phase_data)
    }

    fn stage(&self, annotations: &TypeMap) -> Option<Stage> {
        annotations.get::<KnockoutRound>().copied().map(Stage::from)
    }
}

/// The whole knockout between the rankings of `phase_data`, see [`SingleElimination`].
fn bracket(phase_data: &PhaseData, third_place_match: bool) -> Bracket {
    let mut bracket = BracketBuilder::default();
    let knockout = bracket.add_knockout(phase_data, phase_round, |round, rounds| {
        TypeMap::new().with(KnockoutRound::RoundOf(1 << (rounds - round + 1)))
    });
    if third_place_match && let [.., semifinals, _] = &knockout[..] {
        // In a bracket of three, one of the semifinals is a bye, whose loser is nobody, so the
        // match for third place is decided without being played.
        bracket.add_pairing(
            phase_round(knockout.len() as u32),
            [
                Source::LoserOf(semifinals[1]),
                Source::LoserOf(semifinals[0]),
            ],
            TypeMap::new().with(KnockoutRound::ThirdPlace),
        );
    }
    bracket.build()
}

#[cfg(test)]
mod tests;
