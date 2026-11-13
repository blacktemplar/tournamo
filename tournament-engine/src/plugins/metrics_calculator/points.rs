use plugin_factory::{Plugin, compatibility::Declaration};
use serde::{Deserialize, Serialize};

use super::{Cell, MetricColumn, OldMetrics, ToCell};
use crate::{
    entities::{Bye, Match, PhaseData, RankingId},
    plugins::{
        facts::{CalculatesMetric, NoDraws},
        metrics_calculator::{Calculator, provides_metric, wins_draws_losses::WinsDrawsLosses},
        ranker::{Comparison, RankingCriterion, requires_metric},
    },
    score::Score,
    type_map::TypeMapValue,
};

#[derive(Default, Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct Points(pub Score);

#[typetag::serde(name = "tournament_engine::points")]
impl TypeMapValue for Points {}

impl ToCell for Points {
    fn to_cell(&self) -> Cell {
        Cell::Number(self.0)
    }
}

impl AsRef<Score> for Points {
    fn as_ref(&self) -> &Score {
        &self.0
    }
}

/// Ranks by the [`Points`], where more points are better.
#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::more_points",
    label = "more points",
    description = "usually the first criterion; the points set what each result is worth",
    declare_static = requires_metric::<Self>
)]
pub struct MorePoints;

impl RankingCriterion for MorePoints {
    type Metric = Points;

    fn compare(&self, first: &Points, second: &Points) -> Comparison {
        Comparison::greater_is_better(first.0.cmp(&second.0))
    }
}

crate::register_criterion!(MorePoints);

/// Calculates the [`Points`] of a ranking from its [`WinsDrawsLosses`], which therefore have to
/// be calculated before.
///
/// Offered with the scoring system of chess and most other games scoring a draw as half a win,
/// with a bye worth as much as a win.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::points",
    label = "points",
    description = "scored for every win, every draw and every bye",
    declare_static = Self::declare_static
)]
pub struct PointsCalculator {
    #[plugin(label = "points for a win", min = 0, default = Score::ONE)]
    points_for_win: Score,
    #[plugin(
        label = "points for a draw",
        min = 0,
        default = Score::new(0.5),
        irrelevant_with = NoDraws
    )]
    points_for_draw: Score,
    #[plugin(
        label = "points for a bye",
        description = "a round without a match, e.g. with an odd number of entrants",
        min = 0,
        default = Score::ONE
    )]
    points_for_bye: Score,
}

impl PointsCalculator {
    /// Scores every win with `points_for_win` and every draw with `points_for_draw` points, a
    /// loss and a bye are worth no points.
    ///
    /// Requires the `test-util` feature.
    #[cfg(feature = "test-util")]
    pub const fn new(points_for_win: Score, points_for_draw: Score) -> Self {
        Self {
            points_for_win,
            points_for_draw,
            points_for_bye: Score::ZERO,
        }
    }

    /// Returns this calculator scoring every bye with `points_for_bye` points.
    ///
    /// Requires the `test-util` feature.
    #[cfg(feature = "test-util")]
    pub const fn with_points_for_bye(mut self, points_for_bye: Score) -> Self {
        self.points_for_bye = points_for_bye;
        self
    }

    fn points_of(&self, ranking_id: &RankingId, phase_data: &PhaseData) -> Points {
        let WinsDrawsLosses {
            wins, draws, byes, ..
        } = *phase_data.rankings()[ranking_id]
            .metric::<WinsDrawsLosses>()
            .unwrap();
        Points(
            self.points_for_win * wins.into()
                + self.points_for_draw * draws.into()
                + self.points_for_bye * byes.into(),
        )
    }

    fn declare_static(declaration: &mut Declaration) {
        provides_metric::<Self>(declaration);
        declaration.require::<CalculatesMetric<WinsDrawsLosses>>("the wins, draws and losses");
    }
}

impl Calculator for PointsCalculator {
    type Metric = Points;

    fn after_match(
        &mut self,
        phase_data: &PhaseData,
        r#match: &Match,
        _: &OldMetrics,
    ) -> Vec<(RankingId, Points)> {
        r#match
            .non_losing_rankings()
            .map(|r_id| (*r_id, self.points_of(r_id, phase_data)))
            .collect()
    }

    fn after_bye(
        &mut self,
        phase_data: &PhaseData,
        bye: &Bye,
        _: &OldMetrics,
    ) -> Vec<(RankingId, Points)> {
        if self.points_for_bye.is_zero() {
            return vec![];
        }
        vec![(bye.ranking(), self.points_of(&bye.ranking(), phase_data))]
    }

    fn column(&self) -> Option<MetricColumn> {
        Some(MetricColumn::new::<Points>("points"))
    }
}

crate::register_calculator!(PointsCalculator);

#[cfg(test)]
mod tests;
