use plugin_factory::Plugin;
use serde::{Deserialize, Serialize};

use crate::{
    entities::{Bye, Match, PhaseData, RankingId, ResultOfTeam},
    plugins::{
        Phrase,
        metrics_calculator::{
            Calculator, Cell, MetricColumn, OldMetrics, ToCell, provides_metric,
            updated_for_each_team,
        },
        ranker::{Comparison, RankingCriterion, requires_metric},
    },
    type_map::TypeMapValue,
};

/// How many of its matches a ranking won, drew and lost, and how many byes it had.
///
/// Each of the first three counts orders a ranking on its own, as does the number of matches they
/// add up to, see [`MoreWins`], [`MoreDraws`], [`FewerLosses`] and [`FewerMatchesPlayed`]. A bye is
/// not a match, so it counts as none of them.
#[derive(Default, Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct WinsDrawsLosses {
    pub wins: u32,
    pub draws: u32,
    pub losses: u32,
    pub byes: u32,
}

impl WinsDrawsLosses {
    /// How many matches the ranking played, i.e. how many of its matches got a result reported,
    /// byes not included.
    ///
    /// The same count as the length of [`Ranking::played_matches`](crate::Ranking::played_matches).
    pub const fn matches_played(&self) -> u32 {
        self.wins + self.draws + self.losses
    }
}

/// Shows the wins, draws and losses as the phrase `wins-draws-losses`, e.g. `2-1-0`, without the
/// byes.
impl ToCell for WinsDrawsLosses {
    fn to_cell(&self) -> Cell {
        let english = format!("{}-{}-{}", self.wins, self.draws, self.losses);
        Cell::Text(
            Phrase::new("wins-draws-losses", english)
                .with("wins", self.wins)
                .with("draws", self.draws)
                .with("losses", self.losses),
        )
    }
}

#[typetag::serde(name = "tournament_engine::wins_draws_losses")]
impl TypeMapValue for WinsDrawsLosses {}

/// Ranks by the wins of [`WinsDrawsLosses`], where more wins are better.
#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::more_wins",
    label = "more wins",
    description = "a tie break that ranks winning above drawing",
    declare_static = requires_metric::<Self>
)]
pub struct MoreWins;

impl RankingCriterion for MoreWins {
    type Metric = WinsDrawsLosses;

    fn compare(&self, first: &WinsDrawsLosses, second: &WinsDrawsLosses) -> Comparison {
        Comparison::greater_is_better(first.wins.cmp(&second.wins))
    }
}

crate::register_criterion!(MoreWins);

/// Ranks by the draws of [`WinsDrawsLosses`], where more draws are better.
///
/// Rank by [`FewerLosses`] instead where drawing a match should not rank above winning one.
#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::more_draws",
    label = "more draws",
    description = "a tie break that ranks drawing above both winning and losing",
    declare_static = requires_metric::<Self>
)]
pub struct MoreDraws;

impl RankingCriterion for MoreDraws {
    type Metric = WinsDrawsLosses;

    fn compare(&self, first: &WinsDrawsLosses, second: &WinsDrawsLosses) -> Comparison {
        Comparison::greater_is_better(first.draws.cmp(&second.draws))
    }
}

crate::register_criterion!(MoreDraws);

/// Ranks by the losses of [`WinsDrawsLosses`], where fewer losses are better.
#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::fewer_losses",
    label = "fewer losses",
    description = "a tie break that ranks drawing above losing",
    declare_static = requires_metric::<Self>
)]
pub struct FewerLosses;

impl RankingCriterion for FewerLosses {
    type Metric = WinsDrawsLosses;

    fn compare(&self, first: &WinsDrawsLosses, second: &WinsDrawsLosses) -> Comparison {
        Comparison::less_is_better(first.losses.cmp(&second.losses))
    }
}

crate::register_criterion!(FewerLosses);

/// Ranks by the matches a ranking played, see [`WinsDrawsLosses::matches_played`], where having
/// played fewer of them is better.
///
/// Meant as a tie break: of two rankings with the same result, the one that needed fewer matches
/// for it ranks higher. A phase that means to reward lasting longer instead, e.g. a knockout, needs
/// a criterion of its own that sorts the other way round.
#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::fewer_matches_played",
    label = "fewer matches played",
    description = "the same result out of fewer matches ranks higher",
    declare_static = requires_metric::<Self>
)]
pub struct FewerMatchesPlayed;

impl RankingCriterion for FewerMatchesPlayed {
    type Metric = WinsDrawsLosses;

    fn compare(&self, first: &WinsDrawsLosses, second: &WinsDrawsLosses) -> Comparison {
        Comparison::less_is_better(first.matches_played().cmp(&second.matches_played()))
    }
}

crate::register_criterion!(FewerMatchesPlayed);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::wins_draws_losses",
    label = "win-draw-loss record",
    description = "how many matches were won, drawn and lost",
    declare_static = provides_metric::<Self>
)]
pub struct WinsDrawsLossesCalculator;

impl Calculator for WinsDrawsLossesCalculator {
    type Metric = WinsDrawsLosses;

    fn after_match(
        &mut self,
        phase_data: &PhaseData,
        r#match: &Match,
        _: &OldMetrics,
    ) -> Vec<(RankingId, WinsDrawsLosses)> {
        let result = r#match.result().expect("a reported match is finished");
        updated_for_each_team(
            phase_data,
            r#match,
            |metric: &mut WinsDrawsLosses, team| match result.for_team(team) {
                ResultOfTeam::Win => metric.wins += 1,
                ResultOfTeam::Draw => metric.draws += 1,
                ResultOfTeam::Loss => metric.losses += 1,
            },
        )
    }

    fn after_bye(
        &mut self,
        phase_data: &PhaseData,
        bye: &Bye,
        _: &OldMetrics,
    ) -> Vec<(RankingId, WinsDrawsLosses)> {
        let mut metric = *phase_data.rankings()[&bye.ranking()]
            .metric::<WinsDrawsLosses>()
            .unwrap();
        metric.byes += 1;
        vec![(bye.ranking(), metric)]
    }

    fn column(&self) -> Option<MetricColumn> {
        Some(MetricColumn::new::<WinsDrawsLosses>("W-D-L"))
    }
}

crate::register_calculator!(WinsDrawsLossesCalculator);

#[cfg(test)]
mod tests;
