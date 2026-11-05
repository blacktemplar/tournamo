//! The points, or goals, a ranking scored and conceded, as the match rules tallied them.

use plugin_factory::{Plugin, compatibility::Declaration, register};
use serde::{Deserialize, Serialize};

use crate::{
    entities::competition::{Bye, Match, PhaseData, RankingId, Team},
    plugins::{
        Phrase,
        facts::{CalculatesMetric, MatchTally},
        match_rules::PointsScored,
        metrics_calculator::{Cell, Metric, MetricColumn, MetricsCalculator, OldMetrics, ToCell},
        ranker::{Comparison, RankingCriterion, requires_metric},
    },
    type_map::TypeMapValue,
};

/// The points, or goals, a ranking scored and conceded in all its matches.
#[derive(Default, Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct PointsBalance {
    pub scored: u32,
    pub conceded: u32,
}

impl PointsBalance {
    /// The points scored minus those conceded.
    pub fn difference(&self) -> i64 {
        i64::from(self.scored) - i64::from(self.conceded)
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

#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::higher_points_difference",
    label = "higher points difference",
    description = "the points or goals scored minus those conceded",
    declare_static = requires_metric::<Self>
)]
pub struct HigherPointsDifference;

impl RankingCriterion for HigherPointsDifference {
    type Metric = PointsBalance;

    fn compare(&self, first: &PointsBalance, second: &PointsBalance) -> Comparison {
        Comparison::greater_is_better(first.difference().cmp(&second.difference()))
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
            .expect("the match rules tally the points scored");
        [Team::A, Team::B]
            .into_iter()
            .flat_map(|team| {
                let (scored, conceded) = (*points.of(team), *points.of(team.opponent()));
                r#match.rankings_of(team).iter().map(move |r_id| {
                    let mut metric = *phase_data.rankings()[r_id]
                        .metric::<PointsBalance>()
                        .unwrap();
                    metric.scored += scored;
                    metric.conceded += conceded;
                    (*r_id, Box::new(metric) as Metric)
                })
            })
            .collect()
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

#[cfg(test)]
mod tests {
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::*;
    use crate::{
        entities::{
            competition::{MatchResult, Phase, PhaseBuilder},
            details::{Decision, FieldKey, Sides, Value},
        },
        plugins::{
            match_rules::Scored,
            metrics_calculator::test_utils::{MatchSpec, match_id, metric_of, phase_with_byes},
        },
    };

    fn phase(players: u32, matches: Vec<MatchSpec>, byes: Vec<(u32, usize)>) -> Phase {
        phase_with_byes(
            PhaseBuilder::new()
                .results(Scored::default())
                .add_metric(PointsBalanceCalculator)
                .add_players(players),
            matches,
            byes,
        )
    }

    fn report(phase: &mut Phase, round: u32, a: &[usize], b: &[usize], score: (u32, u32)) {
        let id = match_id(phase, round, a, b);
        let values = vec![(
            FieldKey::new("score"),
            Sides::new(Some(Value::Count(score.0)), Some(Value::Count(score.1))),
        )];
        let result = if score.0 > score.1 {
            MatchResult::AWins
        } else {
            MatchResult::BWins
        };
        phase.report_match(id, Decision::new(result), values, Vec::new());
    }

    fn balance(scored: u32, conceded: u32) -> PointsBalance {
        PointsBalance { scored, conceded }
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
    }

    #[test]
    fn every_player_of_a_team_scores_what_the_team_scored() {
        let mut phase = phase(4, vec![(1, vec![0, 1], vec![2, 3])], Vec::new());

        report(&mut phase, 1, &[0, 1], &[2, 3], (10, 7));

        for player in [0, 1] {
            assert_eq!(metric_of::<PointsBalance>(&phase, player), balance(10, 7));
        }
        for player in [2, 3] {
            assert_eq!(metric_of::<PointsBalance>(&phase, player), balance(7, 10));
        }
    }

    #[test]
    fn a_bye_scores_nothing() {
        let phase = phase(1, Vec::new(), vec![(1, 0)]);

        assert_eq!(metric_of::<PointsBalance>(&phase, 0), balance(0, 0));
    }

    #[test]
    fn the_difference_and_the_points_scored_rank_differently() {
        let (more_scored, better_difference) = (balance(30, 28), balance(20, 10));

        assert_eq!(
            HigherPointsDifference.compare(&better_difference, &more_scored),
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
}
