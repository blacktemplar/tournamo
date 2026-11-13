//! The points, or goals, a ranking scored and conceded, as the match format tallied them, and their
//! difference.

use plugin_factory::{Plugin, compatibility::Declaration, register};
use serde::{Deserialize, Serialize};

use crate::{
    entities::competition::{Bye, Match, PhaseData, RankingId},
    plugins::{
        Phrase,
        facts::{CalculatesMetric, MatchTally},
        match_format::PointsScored,
        metrics_calculator::{
            Cell, Metric, MetricColumn, MetricsCalculator, OldMetrics, ToCell, derived,
            updated_for_each_team,
        },
        ranker::{Comparison, RankingCriterion, requires_metric},
    },
    score::Score,
    type_map::TypeMapValue,
};

/// The points, or goals, a ranking scored and conceded in all its matches.
/// Each total saturates at `u32::MAX`.
#[derive(Default, Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct PointsBalance {
    pub scored: u32,
    pub conceded: u32,
}

impl PointsBalance {
    fn add(&mut self, scored: u32, conceded: u32) {
        self.scored = self.scored.saturating_add(scored);
        self.conceded = self.conceded.saturating_add(conceded);
    }

    /// The points scored minus those conceded.
    pub fn difference(&self) -> PointsDifference {
        PointsDifference(i64::from(self.scored) - i64::from(self.conceded))
    }
}

impl ToCell for PointsBalance {
    fn to_cell(&self) -> Cell {
        let english = format!("{}:{}", self.scored, self.conceded);
        Cell::Text(
            Phrase::new("scored-conceded", english)
                .with("scored", self.scored)
                .with("conceded", self.conceded),
        )
    }
}

#[typetag::serde(name = "tournament_engine::points_balance")]
impl TypeMapValue for PointsBalance {}

/// The points, or goals, a ranking scored minus those it conceded, see
/// [`PointsBalance::difference`].
#[derive(Default, Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct PointsDifference(pub i64);

impl ToCell for PointsDifference {
    fn to_cell(&self) -> Cell {
        Cell::Signed(Score::new(self.0 as f64))
    }
}

#[typetag::serde(name = "tournament_engine::points_difference")]
impl TypeMapValue for PointsDifference {}

#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::higher_points_difference",
    label = "higher points difference",
    description = "the points or goals scored minus those conceded",
    declare_static = requires_metric::<Self>
)]
pub struct HigherPointsDifference;

impl RankingCriterion for HigherPointsDifference {
    type Metric = PointsDifference;

    fn compare(&self, first: &PointsDifference, second: &PointsDifference) -> Comparison {
        Comparison::greater_is_better(first.0.cmp(&second.0))
    }
}

crate::register_criterion!(HigherPointsDifference);

#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::more_points_scored",
    label = "more points scored",
    description = "a tie break that ranks attacking above defending",
    declare_static = requires_metric::<Self>
)]
pub struct MorePointsScored;

impl RankingCriterion for MorePointsScored {
    type Metric = PointsBalance;

    fn compare(&self, first: &PointsBalance, second: &PointsBalance) -> Comparison {
        Comparison::greater_is_better(first.scored.cmp(&second.scored))
    }
}

crate::register_criterion!(MorePointsScored);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::points_balance",
    label = "points scored and conceded",
    description = "the points or goals of all matches, as the results say",
    declare_static = Self::declare_static
)]
pub struct PointsBalanceCalculator;

impl PointsBalanceCalculator {
    fn declare_static(declaration: &mut Declaration) {
        declaration
            .provide(CalculatesMetric::<PointsBalance>::new())
            .require::<MatchTally<PointsScored>>("the points scored");
    }
}

#[register]
impl MetricsCalculator for PointsBalanceCalculator {
    fn default_metric_value(&self) -> Metric {
        Box::new(PointsBalance::default())
    }

    fn update_after_match_result(
        &mut self,
        phase_data: &PhaseData,
        r#match: &Match,
        _: &OldMetrics,
    ) -> Vec<(RankingId, Metric)> {
        let PointsScored(points) = *r#match
            .tally::<PointsScored>()
            .expect("the match format tallies the points scored");
        updated_for_each_team(phase_data, r#match, |balance: &mut PointsBalance, team| {
            balance.add(*points.of(team), *points.of(team.opponent()));
        })
    }

    /// A bye has no score.
    fn update_after_bye(
        &mut self,
        _: &PhaseData,
        _: &Bye,
        _: &OldMetrics,
    ) -> Vec<(RankingId, Metric)> {
        Vec::new()
    }

    fn column(&self) -> Option<MetricColumn> {
        Some(MetricColumn::new::<PointsBalance>("+:−"))
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::points_difference",
    label = "points difference",
    description = "the points or goals scored minus those conceded",
    declare_static = Self::declare_static
)]
pub struct PointsDifferenceCalculator;

impl PointsDifferenceCalculator {
    fn declare_static(declaration: &mut Declaration) {
        declaration
            .provide(CalculatesMetric::<PointsDifference>::new())
            .require::<CalculatesMetric<PointsBalance>>("the points scored and conceded");
    }
}

#[register]
impl MetricsCalculator for PointsDifferenceCalculator {
    fn default_metric_value(&self) -> Metric {
        Box::new(PointsDifference::default())
    }

    fn update_after_match_result(
        &mut self,
        phase_data: &PhaseData,
        _: &Match,
        old_metrics: &OldMetrics,
    ) -> Vec<(RankingId, Metric)> {
        derived(phase_data, old_metrics, PointsBalance::difference)
    }

    fn update_after_bye(
        &mut self,
        phase_data: &PhaseData,
        _: &Bye,
        old_metrics: &OldMetrics,
    ) -> Vec<(RankingId, Metric)> {
        derived(phase_data, old_metrics, PointsBalance::difference)
    }

    fn column(&self) -> Option<MetricColumn> {
        Some(MetricColumn::new::<PointsDifference>("±"))
    }
}

#[cfg(test)]
mod tests {
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::*;
    use crate::{
        entities::{
            competition::{MatchResult, Phase, PhaseBuilder},
            details::{EnteredValue, FieldKey, Part, Sides},
        },
        plugins::{
            match_format::Scored,
            metrics_calculator::test_utils::{MatchSpec, match_id, metric_of, phase_with_byes},
        },
        type_map::TypeMap,
    };

    fn phase(players: u32, matches: Vec<MatchSpec>, byes: Vec<(u32, usize)>) -> Phase {
        phase_with_byes(
            PhaseBuilder::new()
                .match_format(Scored::default())
                .add_metric(PointsBalanceCalculator)
                .add_metric(PointsDifferenceCalculator)
                .add_players(players),
            matches,
            byes,
        )
    }

    fn report(phase: &mut Phase, round: u32, a: &[usize], b: &[usize], score: (u16, u16)) {
        let id = match_id(phase, round, a, b);
        let values = Sides::new(
            Some(EnteredValue::Count(score.0)),
            Some(EnteredValue::Count(score.1)),
        );
        let result = if score.0 > score.1 {
            MatchResult::AWins
        } else {
            MatchResult::BWins
        };
        let contest = Part::new()
            .with_decision(result)
            .with_value(FieldKey::new("score"), values);
        phase.report_match(id, contest);
    }

    fn balance(scored: u32, conceded: u32) -> PointsBalance {
        PointsBalance { scored, conceded }
    }

    #[test]
    fn points_accumulated_across_matches_saturate_at_the_aggregate_limit() {
        let mut points = PointsBalance::default();
        points.add(u32::MAX - 1, u32::MAX - 2);
        points.add(2, 1);
        assert_eq!(points, balance(u32::MAX, u32::MAX - 1));
        assert_eq!(points.difference(), PointsDifference(1));
        points.add(1, 2);
        assert_eq!(points, balance(u32::MAX, u32::MAX));
        assert_eq!(points.difference(), PointsDifference(0));
    }

    #[test]
    fn the_points_of_every_match_add_up() {
        let matches = vec![(1, vec![0], vec![1]), (2, vec![0], vec![2])];
        let mut phase = phase(3, matches, Vec::new());

        report(&mut phase, 1, &[0], &[1], (21, 17));
        report(&mut phase, 2, &[0], &[2], (5, 21));

        assert_eq!(metric_of::<PointsBalance>(&phase, 0), balance(26, 38));
        assert_eq!(metric_of::<PointsBalance>(&phase, 1), balance(17, 21));
        assert_eq!(metric_of::<PointsBalance>(&phase, 2), balance(21, 5));
        assert_eq!(
            metric_of::<PointsDifference>(&phase, 0),
            PointsDifference(-12)
        );
        assert_eq!(
            metric_of::<PointsDifference>(&phase, 1),
            PointsDifference(-4)
        );
        assert_eq!(
            metric_of::<PointsDifference>(&phase, 2),
            PointsDifference(16)
        );
    }

    #[test]
    fn every_player_of_a_team_scores_what_the_team_scored() {
        let mut phase = phase(4, vec![(1, vec![0, 1], vec![2, 3])], Vec::new());

        report(&mut phase, 1, &[0, 1], &[2, 3], (10, 7));

        for player in [0, 1] {
            assert_eq!(metric_of::<PointsBalance>(&phase, player), balance(10, 7));
            assert_eq!(
                metric_of::<PointsDifference>(&phase, player),
                PointsDifference(3)
            );
        }
        for player in [2, 3] {
            assert_eq!(metric_of::<PointsBalance>(&phase, player), balance(7, 10));
            assert_eq!(
                metric_of::<PointsDifference>(&phase, player),
                PointsDifference(-3)
            );
        }
    }

    #[test]
    fn a_bye_scores_nothing() {
        let phase = phase(1, Vec::new(), vec![(1, 0)]);

        assert_eq!(metric_of::<PointsBalance>(&phase, 0), balance(0, 0));
        assert_eq!(
            metric_of::<PointsDifference>(&phase, 0),
            PointsDifference(0)
        );
    }

    #[test]
    fn the_difference_and_the_points_scored_rank_differently() {
        let (more_scored, better_difference) = (balance(30, 28), balance(20, 10));

        assert_eq!(
            HigherPointsDifference
                .compare(&better_difference.difference(), &more_scored.difference()),
            Comparison::Better
        );
        assert_eq!(
            MorePointsScored.compare(&better_difference, &more_scored),
            Comparison::Worse
        );
    }

    #[test]
    fn the_column_shows_scored_to_conceded() {
        assert_eq!(balance(26, 38).to_cell().to_string(), "26:38");
    }

    #[test]
    fn the_difference_shows_its_sign() {
        assert_eq!(PointsDifference(4).to_cell(), Cell::Signed(Score::new(4.0)));
    }

    /// The tags are part of every log and snapshot written, see `transition_log_serialization.rs`.
    #[test]
    fn the_calculators_and_their_metrics_are_serialized_under_their_tags() {
        let calculators: Vec<Box<dyn MetricsCalculator>> = vec![
            Box::new(PointsBalanceCalculator),
            Box::new(PointsDifferenceCalculator),
        ];
        let mut metrics = TypeMap::new();
        metrics.insert(PointsBalance::default());
        metrics.insert(PointsDifference::default());

        let tags = [
            "tournament_engine::points_balance",
            "tournament_engine::points_difference",
        ];
        for (index, tag) in tags.into_iter().enumerate() {
            assert_eq!(
                serde_json::to_value(&calculators).unwrap()[index]["type"],
                tag
            );
            assert_eq!(serde_json::to_value(&metrics).unwrap()[index]["type"], tag);
        }
    }
}
