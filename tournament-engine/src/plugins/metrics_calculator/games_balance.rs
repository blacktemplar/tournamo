//! The games a ranking won and lost, as the match format tallied them, and their difference.

use plugin_factory::{Plugin, compatibility::Declaration};
use serde::{Deserialize, Serialize};

use crate::{
    entities::{Bye, Match, PhaseData, RankingId},
    plugins::{
        Phrase,
        facts::{CalculatesMetric, MatchTally},
        match_format::GamesWon,
        metrics_calculator::{
            Calculator, Cell, MetricColumn, OldMetrics, ToCell, derived, provides_metric,
            updated_for_each_team,
        },
        ranker::{Comparison, RankingCriterion, requires_metric},
    },
    score::Score,
    type_map::TypeMapValue,
};

/// The games a ranking won and lost in all its matches.
/// Each total saturates at `u32::MAX`.
#[derive(Default, Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct GamesBalance {
    pub won: u32,
    pub lost: u32,
}

impl GamesBalance {
    fn add(&mut self, won: u32, lost: u32) {
        self.won = self.won.saturating_add(won);
        self.lost = self.lost.saturating_add(lost);
    }

    /// The games won minus those lost.
    pub fn difference(&self) -> GamesDifference {
        GamesDifference(i64::from(self.won) - i64::from(self.lost))
    }
}

impl ToCell for GamesBalance {
    fn to_cell(&self) -> Cell {
        let english = format!("{}:{}", self.won, self.lost);
        Cell::Text(
            Phrase::new("won-lost", english)
                .with("won", self.won)
                .with("lost", self.lost),
        )
    }
}

#[typetag::serde(name = "tournament_engine::games_balance")]
impl TypeMapValue for GamesBalance {}

/// The games a ranking won minus those it lost, see [`GamesBalance::difference`].
#[derive(Default, Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct GamesDifference(pub i64);

impl ToCell for GamesDifference {
    fn to_cell(&self) -> Cell {
        Cell::Signed(Score::new(self.0 as f64))
    }
}

#[typetag::serde(name = "tournament_engine::games_difference")]
impl TypeMapValue for GamesDifference {}

#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::higher_games_difference",
    label = "higher games difference",
    description = "the games won minus those lost",
    declare_static = requires_metric::<Self>
)]
pub struct HigherGamesDifference;

impl RankingCriterion for HigherGamesDifference {
    type Metric = GamesDifference;

    fn compare(&self, first: &GamesDifference, second: &GamesDifference) -> Comparison {
        Comparison::greater_is_better(first.0.cmp(&second.0))
    }
}

crate::register_criterion!(HigherGamesDifference);

#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::more_games_won",
    label = "more games won",
    description = "a tie break counting the games won, not those lost",
    declare_static = requires_metric::<Self>
)]
pub struct MoreGamesWon;

impl RankingCriterion for MoreGamesWon {
    type Metric = GamesBalance;

    fn compare(&self, first: &GamesBalance, second: &GamesBalance) -> Comparison {
        Comparison::greater_is_better(first.won.cmp(&second.won))
    }
}

crate::register_criterion!(MoreGamesWon);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::games_balance",
    label = "games won and lost",
    description = "the games, or sets, of all matches, as the results say",
    declare_static = Self::declare_static
)]
pub struct GamesBalanceCalculator;

impl GamesBalanceCalculator {
    fn declare_static(declaration: &mut Declaration) {
        provides_metric::<Self>(declaration);
        declaration.require::<MatchTally<GamesWon>>("the games won");
    }
}

impl Calculator for GamesBalanceCalculator {
    type Metric = GamesBalance;

    fn after_match(
        &mut self,
        phase_data: &PhaseData,
        r#match: &Match,
        _: &OldMetrics,
    ) -> Vec<(RankingId, GamesBalance)> {
        let GamesWon(games) = *r#match
            .tally::<GamesWon>()
            .expect("the match format tallies the games won");
        updated_for_each_team(phase_data, r#match, |balance: &mut GamesBalance, team| {
            balance.add(*games.of(team), *games.of(team.opponent()));
        })
    }

    /// A bye has no games.
    fn after_bye(
        &mut self,
        _: &PhaseData,
        _: &Bye,
        _: &OldMetrics,
    ) -> Vec<(RankingId, GamesBalance)> {
        Vec::new()
    }

    fn column(&self) -> Option<MetricColumn> {
        Some(MetricColumn::new::<GamesBalance>("games"))
    }
}

crate::register_calculator!(GamesBalanceCalculator);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::games_difference",
    label = "games difference",
    description = "the games won minus those lost",
    declare_static = Self::declare_static
)]
pub struct GamesDifferenceCalculator;

impl GamesDifferenceCalculator {
    fn declare_static(declaration: &mut Declaration) {
        provides_metric::<Self>(declaration);
        declaration.require::<CalculatesMetric<GamesBalance>>("the games won and lost");
    }
}

impl Calculator for GamesDifferenceCalculator {
    type Metric = GamesDifference;

    fn after_match(
        &mut self,
        phase_data: &PhaseData,
        _: &Match,
        old_metrics: &OldMetrics,
    ) -> Vec<(RankingId, GamesDifference)> {
        derived(phase_data, old_metrics, GamesBalance::difference)
    }

    fn after_bye(
        &mut self,
        phase_data: &PhaseData,
        _: &Bye,
        old_metrics: &OldMetrics,
    ) -> Vec<(RankingId, GamesDifference)> {
        derived(phase_data, old_metrics, GamesBalance::difference)
    }

    fn column(&self) -> Option<MetricColumn> {
        Some(MetricColumn::new::<GamesDifference>("games ±"))
    }
}

crate::register_calculator!(GamesDifferenceCalculator);

#[cfg(test)]
mod tests {
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::*;
    use crate::{
        entities::{
            MatchResult, Phase, PhaseBuilder,
            details::{EnteredValue, FieldKey, Part, Sides},
        },
        plugins::{
            match_format::BestOf,
            metrics_calculator::{
                MetricsCalculator,
                test_utils::{MatchSpec, match_id, metric_of, phase_with_byes},
            },
        },
        type_map::TypeMap,
    };

    fn phase(players: u32, matches: Vec<MatchSpec>, byes: Vec<(u32, usize)>) -> Phase {
        phase_with_byes(
            PhaseBuilder::new()
                .match_format(BestOf::new(5))
                .add_metric(GamesBalanceCalculator)
                .add_metric(GamesDifferenceCalculator)
                .add_players(players),
            matches,
            byes,
        )
    }

    /// Reports the match of `a` and `b` in `round` as games, each won by side A if `true`.
    fn report(phase: &mut Phase, round: u32, a: &[usize], b: &[usize], games: &[bool]) {
        let id = match_id(phase, round, a, b);
        let game = |a_wins: bool| {
            let (result, score) = if a_wins {
                (MatchResult::AWins, (11, 5))
            } else {
                (MatchResult::BWins, (5, 11))
            };
            Part::new().with_decision(result).with_value(
                FieldKey::new("score"),
                Sides::new(
                    Some(EnteredValue::Count(score.0)),
                    Some(EnteredValue::Count(score.1)),
                ),
            )
        };
        let won_by_a = games.iter().filter(|&&a_wins| a_wins).count();
        let result = if 2 * won_by_a > games.len() {
            MatchResult::AWins
        } else {
            MatchResult::BWins
        };
        let contest = (games.iter().map(|&a_wins| game(a_wins)))
            .fold(Part::new().with_decision(result), Part::with_part);
        phase.report_match(id, contest);
    }

    fn balance(won: u32, lost: u32) -> GamesBalance {
        GamesBalance { won, lost }
    }

    #[test]
    fn the_games_of_every_match_add_up() {
        let matches = vec![(1, vec![0], vec![1]), (2, vec![0], vec![2])];
        let mut phase = phase(3, matches, Vec::new());

        report(&mut phase, 1, &[0], &[1], &[true, true, true]);
        report(
            &mut phase,
            2,
            &[0],
            &[2],
            &[true, false, false, true, false],
        );

        assert_eq!(metric_of::<GamesBalance>(&phase, 0), balance(5, 3));
        assert_eq!(metric_of::<GamesBalance>(&phase, 1), balance(0, 3));
        assert_eq!(metric_of::<GamesBalance>(&phase, 2), balance(3, 2));
        assert_eq!(metric_of::<GamesDifference>(&phase, 0), GamesDifference(2));
        assert_eq!(metric_of::<GamesDifference>(&phase, 1), GamesDifference(-3));
        assert_eq!(metric_of::<GamesDifference>(&phase, 2), GamesDifference(1));
    }

    #[test]
    fn every_player_of_a_team_wins_what_the_team_won() {
        let mut phase = phase(4, vec![(1, vec![0, 1], vec![2, 3])], Vec::new());

        report(&mut phase, 1, &[0, 1], &[2, 3], &[false, true, true, true]);

        for player in [0, 1] {
            assert_eq!(metric_of::<GamesBalance>(&phase, player), balance(3, 1));
            assert_eq!(
                metric_of::<GamesDifference>(&phase, player),
                GamesDifference(2)
            );
        }
        for player in [2, 3] {
            assert_eq!(metric_of::<GamesBalance>(&phase, player), balance(1, 3));
            assert_eq!(
                metric_of::<GamesDifference>(&phase, player),
                GamesDifference(-2)
            );
        }
    }

    #[test]
    fn a_bye_wins_no_game() {
        let phase = phase(1, Vec::new(), vec![(1, 0)]);

        assert_eq!(metric_of::<GamesBalance>(&phase, 0), balance(0, 0));
        assert_eq!(metric_of::<GamesDifference>(&phase, 0), GamesDifference(0));
    }

    #[test]
    fn the_difference_and_the_games_won_rank_differently() {
        let (more_won, better_difference) = (balance(6, 4), balance(4, 0));

        assert_eq!(
            HigherGamesDifference.compare(&better_difference.difference(), &more_won.difference()),
            Comparison::Better
        );
        assert_eq!(
            MoreGamesWon.compare(&better_difference, &more_won),
            Comparison::Worse
        );
    }

    #[test]
    fn the_column_shows_won_to_lost() {
        assert_eq!(balance(5, 3).to_cell().to_string(), "5:3");
    }

    #[test]
    fn the_difference_shows_its_sign() {
        assert_eq!(
            GamesDifference(-2).to_cell(),
            Cell::Signed(Score::new(-2.0))
        );
    }

    /// The tags are part of every log and snapshot written, see `transition_log_serialization.rs`.
    #[test]
    fn the_calculators_and_their_metrics_are_serialized_under_their_tags() {
        let calculators: Vec<Box<dyn MetricsCalculator>> = vec![
            Box::new(GamesBalanceCalculator),
            Box::new(GamesDifferenceCalculator),
        ];
        let mut metrics = TypeMap::new();
        metrics.insert(GamesBalance::default());
        metrics.insert(GamesDifference::default());

        let tags = [
            "tournament_engine::games_balance",
            "tournament_engine::games_difference",
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
