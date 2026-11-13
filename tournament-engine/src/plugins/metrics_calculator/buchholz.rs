use plugin_factory::{Plugin, compatibility::Declaration};
use serde::{Deserialize, Serialize};

use super::{Cell, MetricColumn, OldMetrics, ToCell};
use crate::{
    entities::{Bye, Match, MatchResult, PhaseData, RankingId, ResultOfTeam, Team},
    map::Map,
    plugins::{
        facts::{CalculatesMetric, NoDraws},
        metrics_calculator::{Calculator, points::Points, provides_metric},
        ranker::{Comparison, RankingCriterion, requires_metric},
    },
    score::Score,
    type_map::TypeMapValue,
};

#[derive(Default, Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct Buchholz(pub Score);

#[typetag::serde(name = "tournament_engine::buchholz")]
impl TypeMapValue for Buchholz {}

impl ToCell for Buchholz {
    fn to_cell(&self) -> Cell {
        Cell::Number(self.0)
    }
}

impl AsRef<Score> for Buchholz {
    fn as_ref(&self) -> &Score {
        &self.0
    }
}

impl From<Score> for Buchholz {
    fn from(value: Score) -> Self {
        Self(value)
    }
}

/// Ranks by the [`Buchholz`], where more is better.
#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::more_buchholz",
    label = "more Buchholz",
    description = "a tie break favouring those who met stronger opponents",
    declare_static = requires_metric::<Self>
)]
pub struct MoreBuchholz;

impl RankingCriterion for MoreBuchholz {
    type Metric = Buchholz;

    fn compare(&self, first: &Buchholz, second: &Buchholz) -> Comparison {
        Comparison::greater_is_better(first.0.cmp(&second.0))
    }
}

crate::register_criterion!(MoreBuchholz);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::buchholz",
    label = "Buchholz",
    description = "the points of all opponents",
    declare_static = Self::declare_static
)]
pub struct BuchholzCalculator;

impl BuchholzCalculator {
    fn declare_static(declaration: &mut Declaration) {
        provides_metric::<Self>(declaration);
        declaration.require::<CalculatesMetric<Points>>("the points");
    }
}

impl Calculator for BuchholzCalculator {
    type Metric = Buchholz;

    fn after_match(
        &mut self,
        phase_data: &PhaseData,
        r#match: &Match,
        old_metrics: &OldMetrics,
    ) -> Vec<(RankingId, Buchholz)> {
        weighted_opponent_sum_changes::<Points, Buchholz>(
            phase_data,
            Some(r#match),
            old_metrics,
            |_| Score::ONE,
        )
    }

    /// A bye has no opponent to count, but the points it is worth count for the opponents of its
    /// ranking.
    fn after_bye(
        &mut self,
        phase_data: &PhaseData,
        _: &Bye,
        old_metrics: &OldMetrics,
    ) -> Vec<(RankingId, Buchholz)> {
        weighted_opponent_sum_changes::<Points, Buchholz>(phase_data, None, old_metrics, |_| {
            Score::ONE
        })
    }

    fn column(&self) -> Option<MetricColumn> {
        Some(MetricColumn::new::<Buchholz>("Buchholz"))
    }
}

crate::register_calculator!(BuchholzCalculator);

#[derive(Default, Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct BuchholzOfBuchholz(pub Score);

#[typetag::serde(name = "tournament_engine::buchholz_of_buchholz")]
impl TypeMapValue for BuchholzOfBuchholz {}

impl ToCell for BuchholzOfBuchholz {
    fn to_cell(&self) -> Cell {
        Cell::Number(self.0)
    }
}

impl AsRef<Score> for BuchholzOfBuchholz {
    fn as_ref(&self) -> &Score {
        &self.0
    }
}

impl From<Score> for BuchholzOfBuchholz {
    fn from(value: Score) -> Self {
        Self(value)
    }
}

/// Ranks by the [`BuchholzOfBuchholz`], where more is better.
#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::more_buchholz_of_buchholz",
    label = "more Buchholz of Buchholz",
    description = "a tie break for when the Buchholz is level too, by the opponents’ opponents",
    declare_static = requires_metric::<Self>
)]
pub struct MoreBuchholzOfBuchholz;

impl RankingCriterion for MoreBuchholzOfBuchholz {
    type Metric = BuchholzOfBuchholz;

    fn compare(&self, first: &BuchholzOfBuchholz, second: &BuchholzOfBuchholz) -> Comparison {
        Comparison::greater_is_better(first.0.cmp(&second.0))
    }
}

crate::register_criterion!(MoreBuchholzOfBuchholz);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::buchholz_of_buchholz",
    label = "Buchholz of Buchholz",
    description = "the Buchholz of all opponents",
    declare_static = Self::declare_static
)]
pub struct BuchholzOfBuchholzCalculator;

impl BuchholzOfBuchholzCalculator {
    fn declare_static(declaration: &mut Declaration) {
        provides_metric::<Self>(declaration);
        declaration.require::<CalculatesMetric<Buchholz>>("the Buchholz");
    }
}

impl Calculator for BuchholzOfBuchholzCalculator {
    type Metric = BuchholzOfBuchholz;

    fn after_match(
        &mut self,
        phase_data: &PhaseData,
        r#match: &Match,
        old_metrics: &OldMetrics,
    ) -> Vec<(RankingId, BuchholzOfBuchholz)> {
        weighted_opponent_sum_changes::<Buchholz, BuchholzOfBuchholz>(
            phase_data,
            Some(r#match),
            old_metrics,
            |_| Score::ONE,
        )
    }

    /// A bye has no opponent to count, but a change of the buchholz of its ranking counts for the
    /// opponents of that ranking.
    fn after_bye(
        &mut self,
        phase_data: &PhaseData,
        _: &Bye,
        old_metrics: &OldMetrics,
    ) -> Vec<(RankingId, BuchholzOfBuchholz)> {
        weighted_opponent_sum_changes::<Buchholz, BuchholzOfBuchholz>(
            phase_data,
            None,
            old_metrics,
            |_| Score::ONE,
        )
    }

    fn column(&self) -> Option<MetricColumn> {
        Some(MetricColumn::new::<BuchholzOfBuchholz>(
            "Buchholz of Buchholz",
        ))
    }
}

crate::register_calculator!(BuchholzOfBuchholzCalculator);

#[derive(Default, Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct SonnebornBerger(pub Score);

#[typetag::serde(name = "tournament_engine::sonneborn_berger")]
impl TypeMapValue for SonnebornBerger {}

impl ToCell for SonnebornBerger {
    fn to_cell(&self) -> Cell {
        Cell::Number(self.0)
    }
}

impl AsRef<Score> for SonnebornBerger {
    fn as_ref(&self) -> &Score {
        &self.0
    }
}

impl From<Score> for SonnebornBerger {
    fn from(value: Score) -> Self {
        Self(value)
    }
}

/// Ranks by the [`SonnebornBerger`], where more is better.
#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::more_sonneborn_berger",
    label = "more Sonneborn-Berger",
    description = "a tie break favouring those who beat or drew stronger opponents",
    declare_static = requires_metric::<Self>
)]
pub struct MoreSonnebornBerger;

impl RankingCriterion for MoreSonnebornBerger {
    type Metric = SonnebornBerger;

    fn compare(&self, first: &SonnebornBerger, second: &SonnebornBerger) -> Comparison {
        Comparison::greater_is_better(first.0.cmp(&second.0))
    }
}

crate::register_criterion!(MoreSonnebornBerger);

/// Offered with the classical weights: the points of a beaten opponent count fully, those of an
/// opponent that was played to a draw count half.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::sonneborn_berger",
    label = "Sonneborn-Berger",
    description = "the points of the opponents that were beaten or played to a draw",
    declare_static = Self::declare_static
)]
pub struct SonnebornBergerCalculator {
    #[plugin(
        label = "weight of a beaten opponent",
        description = "the points of that opponent are multiplied by this",
        min = 0,
        default = Score::ONE
    )]
    weight_for_win: Score,
    #[plugin(
        label = "weight of an opponent played to a draw",
        description = "the points of that opponent are multiplied by this",
        min = 0,
        default = Score::new(0.5),
        irrelevant_with = NoDraws
    )]
    weight_for_draw: Score,
}

impl SonnebornBergerCalculator {
    /// Counts the points of every beaten opponent with `weight_for_win` and the points of every
    /// opponent that was played to a draw with `weight_for_draw`, the points of an opponent that
    /// won are never counted.
    ///
    /// Requires the `test-util` feature.
    #[cfg(feature = "test-util")]
    pub const fn new(weight_for_win: Score, weight_for_draw: Score) -> Self {
        Self {
            weight_for_win,
            weight_for_draw,
        }
    }

    fn declare_static(declaration: &mut Declaration) {
        provides_metric::<Self>(declaration);
        declaration.require::<CalculatesMetric<Points>>("the points");
    }

    fn weight(&self, result: ResultOfTeam) -> Score {
        match result {
            ResultOfTeam::Win => self.weight_for_win,
            ResultOfTeam::Draw => self.weight_for_draw,
            ResultOfTeam::Loss => Score::ZERO,
        }
    }
}

impl Calculator for SonnebornBergerCalculator {
    type Metric = SonnebornBerger;

    fn after_match(
        &mut self,
        phase_data: &PhaseData,
        r#match: &Match,
        old_metrics: &OldMetrics,
    ) -> Vec<(RankingId, SonnebornBerger)> {
        weighted_opponent_sum_changes::<Points, SonnebornBerger>(
            phase_data,
            Some(r#match),
            old_metrics,
            |result| self.weight(result),
        )
    }

    /// A bye has no opponent to count, but the points it is worth count for the opponents that beat
    /// its ranking or played it to a draw.
    fn after_bye(
        &mut self,
        phase_data: &PhaseData,
        _: &Bye,
        old_metrics: &OldMetrics,
    ) -> Vec<(RankingId, SonnebornBerger)> {
        weighted_opponent_sum_changes::<Points, SonnebornBerger>(
            phase_data,
            None,
            old_metrics,
            |result| self.weight(result),
        )
    }

    fn column(&self) -> Option<MetricColumn> {
        Some(MetricColumn::new::<SonnebornBerger>("Sonneborn-Berger"))
    }
}

crate::register_calculator!(SonnebornBergerCalculator);

/// The result of `match`, a match a ranking played.
fn played(r#match: &Match) -> MatchResult {
    r#match.result().expect("a played match is finished")
}

/// The updates of the metric `B`, which sums the metric `M` of all opponents of a ranking, each
/// multiplied by the `weight` of the result the ranking achieved against that opponent, after the
/// result of `match` was reported, or, without a match, after a bye was scheduled.
fn weighted_opponent_sum_changes<M, B>(
    phase_data: &PhaseData,
    r#match: Option<&Match>,
    old_metrics: &OldMetrics,
    weight: impl Fn(ResultOfTeam) -> Score,
) -> Vec<(RankingId, B)>
where
    M: TypeMapValue + AsRef<Score>,
    B: TypeMapValue + AsRef<Score> + From<Score>,
{
    let mut changes: Map<_, Score> = Map::default();
    for (r_id, old_metric) in old_metrics.of::<M>() {
        let ranking = &phase_data.rankings()[&r_id];
        let new_metric = ranking.metric::<M>().unwrap();
        let m_diff = *new_metric.as_ref() - *old_metric.as_ref();
        if !m_diff.is_zero() {
            // `B` of all opponents of this ranking change, according to their result against it
            for match_id in ranking.played_matches() {
                if r#match.is_some_and(|r#match| match_id == &r#match.id()) {
                    // we don't consider the newly reported match here as we will account for it
                    // later separately
                    continue;
                }
                let match_of_r = &phase_data.matches()[match_id];
                let opponent_team = match_of_r.team_of(&r_id).unwrap().opponent();
                let opponent_weight = weight(played(match_of_r).for_team(opponent_team));
                if !opponent_weight.is_zero() {
                    let change = opponent_weight * m_diff;
                    for opponent in match_of_r.rankings_of(opponent_team) {
                        *changes.entry(*opponent).or_default() += change;
                    }
                }
            }
        }
    }
    // now also account for the new match
    for (r#match, team) in r#match
        .into_iter()
        .flat_map(|r#match| [(r#match, Team::A), (r#match, Team::B)])
    {
        let team_weight = weight(played(r#match).for_team(team));
        if team_weight.is_zero() {
            continue;
        }
        // add metric `M` of the opponent team, weighted, to `B` of team
        let m_sum: Score = r#match
            .rankings_of(team.opponent())
            .iter()
            .map(|r_id| *phase_data.rankings()[r_id].metric::<M>().unwrap().as_ref())
            .sum();
        let change = team_weight * m_sum;
        for r_id in r#match.rankings_of(team) {
            *changes.entry(*r_id).or_default() += change;
        }
    }
    changes
        .into_iter()
        .map(|(r_id, change)| {
            let old_b = *phase_data.rankings()[&r_id].metric::<B>().unwrap().as_ref();
            (r_id, B::from(old_b + change))
        })
        .collect()
}

#[cfg(test)]
mod tests;
