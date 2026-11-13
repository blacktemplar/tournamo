use std::num::NonZeroU32;

use plugin_factory::Plugin;
use serde::{Deserialize, Serialize};

use crate::{
    entities::{Bye, Match, PhaseData, RankingId},
    plugins::{
        Phrase,
        metrics_calculator::{Calculator, Cell, MetricColumn, OldMetrics, ToCell, provides_metric},
        ranker::{Comparison, RankingCriterion, requires_metric},
    },
    type_map::TypeMapValue,
};

#[derive(Debug, PartialEq, Eq, Clone, Copy, Default, Serialize, Deserialize)]
pub struct RoundOfFirstLoss(pub Option<NonZeroU32>);

/// Shows the round as a [`Cell::Round`], and the phrase `unbeaten`, `-`, if there was no loss.
impl ToCell for RoundOfFirstLoss {
    fn to_cell(&self) -> Cell {
        match self.0 {
            Some(round) => Cell::Round(round),
            None => Cell::Text(Phrase::new("unbeaten", "-")),
        }
    }
}

#[typetag::serde(name = "tournament_engine::round_of_first_loss")]
impl TypeMapValue for RoundOfFirstLoss {}

/// Ranks by the [`RoundOfFirstLoss`]: the later the better, never having lost is best.
#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::later_first_loss",
    label = "later first loss",
    description = "by the round of the first loss; never having lost is best",
    declare_static = requires_metric::<Self>
)]
pub struct LaterFirstLoss;

impl RankingCriterion for LaterFirstLoss {
    type Metric = RoundOfFirstLoss;

    fn compare(&self, first: &RoundOfFirstLoss, second: &RoundOfFirstLoss) -> Comparison {
        Comparison::none_or_greater_is_better(first.0, second.0)
    }
}

crate::register_criterion!(LaterFirstLoss);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::round_of_first_loss",
    label = "round of the first loss",
    description = "the round in which an entrant lost for the first time",
    declare_static = provides_metric::<Self>
)]
pub struct RoundOfFirstLossCalculator;

impl Calculator for RoundOfFirstLossCalculator {
    type Metric = RoundOfFirstLoss;

    fn after_match(
        &mut self,
        phase_data: &PhaseData,
        r#match: &Match,
        _: &OldMetrics,
    ) -> Vec<(RankingId, RoundOfFirstLoss)> {
        let result = r#match.result().expect("a reported match is finished");
        let Some(loser) = result.loser() else {
            return vec![]; // nothing to update
        };
        r#match
            .rankings_of(loser)
            .iter()
            .map(|r_id| {
                let old_value = phase_data.rankings()[r_id]
                    .metric::<RoundOfFirstLoss>()
                    .unwrap();
                let new_value = RoundOfFirstLoss(Some(match old_value.0 {
                    None => r#match.round(),
                    Some(x) => x.min(r#match.round()),
                }));
                (*r_id, new_value)
            })
            .collect()
    }

    /// A bye is never lost.
    fn after_bye(
        &mut self,
        _: &PhaseData,
        _: &Bye,
        _: &OldMetrics,
    ) -> Vec<(RankingId, RoundOfFirstLoss)> {
        vec![]
    }

    fn column(&self) -> Option<MetricColumn> {
        Some(MetricColumn::new::<RoundOfFirstLoss>("lost in"))
    }
}

crate::register_calculator!(RoundOfFirstLossCalculator);

#[cfg(test)]
mod tests;
