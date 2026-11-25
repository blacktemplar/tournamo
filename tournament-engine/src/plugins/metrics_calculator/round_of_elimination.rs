use std::num::NonZeroU32;

use plugin_factory::{Plugin, compatibility::Declaration};
use serde::{Deserialize, Serialize};

use crate::{
    entities::{Annotated, Bye, DivisionData, Match, MatchResult, RankingId, Team},
    plugins::{
        Phrase,
        facts::MatchAnnotation,
        metrics_calculator::{
            Calculator, Cell, Metric, MetricColumn, OldMetrics, ToCell, provides_metric,
        },
        ranker::{Comparison, RankingCriterion, requires_metric},
    },
    type_map::TypeMapValue,
};

/// Who is knocked out of the phase by losing a match, attached to the match as an annotation by
/// the [`PhaseFormat`](crate::plugins::phase_format::PhaseFormat) that schedules it.
///
/// A match without this annotation knocks out nobody, and neither does a draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Eliminates {
    /// The loser is out, whichever side it is.
    Loser,
    /// Only this side is out if it loses; the other side plays on after a loss.
    Only(Team),
}

#[typetag::serde(name = "tournament_engine::eliminates")]
impl TypeMapValue for Eliminates {}

/// The round in which a ranking was knocked out of the phase, see [`Eliminates`], or `None` while
/// it is still in.
#[derive(Debug, PartialEq, Eq, Clone, Copy, Default, Serialize, Deserialize)]
pub struct RoundOfElimination(pub Option<NonZeroU32>);

/// Shows the round as a [`Cell::Round`], and the phrase `still-in`, `-`, while the ranking is still
/// in.
impl ToCell for RoundOfElimination {
    fn to_cell(&self) -> Cell {
        match self.0 {
            Some(round) => Cell::Round(round),
            None => Cell::Text(Phrase::new("still-in", "-")),
        }
    }
}

impl Metric for RoundOfElimination {
    const LABEL: &'static str = "round of elimination";
    const DESCRIPTION: Option<&'static str> = Some("the round in which an entrant was knocked out");

    fn column() -> Option<MetricColumn> {
        Some(MetricColumn::new::<Self>("out in"))
    }
}

crate::metric!(
    RoundOfElimination,
    id = "tournament_engine::round_of_elimination"
);

/// Ranks by [`RoundOfElimination`]: the later a ranking was knocked out the better, still being in
/// is best.
#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::later_elimination",
    label = "later elimination",
    description = "by the round of elimination; still being in is best",
    declare_static = requires_metric::<Self>
)]
pub struct LaterElimination;

impl RankingCriterion for LaterElimination {
    type Metric = RoundOfElimination;

    fn compare(&self, first: &RoundOfElimination, second: &RoundOfElimination) -> Comparison {
        Comparison::none_or_greater_is_better(first.0, second.0)
    }
}

crate::register_criterion!(LaterElimination);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::round_of_elimination",
    label = "round of elimination",
    description = "the round in which an entrant was knocked out",
    declare_static = Self::declare_static
)]
pub struct RoundOfEliminationCalculator;

impl RoundOfEliminationCalculator {
    fn declare_static(declaration: &mut Declaration) {
        provides_metric::<Self>(declaration);
        declaration.require::<MatchAnnotation<Eliminates>>(Phrase::new(
            "eliminations",
            "a format saying which matches eliminate",
        ));
    }
}

impl Calculator for RoundOfEliminationCalculator {
    type Metric = RoundOfElimination;

    fn after_match(
        &mut self,
        phase_data: &DivisionData,
        r#match: &Match,
        _: &OldMetrics,
    ) -> Vec<(RankingId, RoundOfElimination)> {
        let Some(eliminates) = r#match.annotation::<Eliminates>() else {
            return vec![];
        };
        let Some(loser) = r#match.result().and_then(MatchResult::loser) else {
            return vec![];
        };
        if matches!(eliminates, Eliminates::Only(team) if *team != loser) {
            return vec![];
        }
        r#match
            .rankings_of(loser)
            .iter()
            // a ranking knocked out twice is out since the earlier round, whichever result came
            // first
            .filter_map(|r_id| {
                let out = phase_data.rankings()[r_id].metric::<RoundOfElimination>();
                let earlier = out.unwrap().0.filter(|round| *round <= r#match.round());
                earlier
                    .is_none()
                    .then(|| (*r_id, RoundOfElimination(Some(r#match.round()))))
            })
            .collect()
    }

    /// A bye knocks out nobody.
    fn after_bye(
        &mut self,
        _: &DivisionData,
        _: &Bye,
        _: &OldMetrics,
    ) -> Vec<(RankingId, RoundOfElimination)> {
        vec![]
    }
}

crate::register_calculator!(RoundOfEliminationCalculator);

#[cfg(test)]
mod tests;
