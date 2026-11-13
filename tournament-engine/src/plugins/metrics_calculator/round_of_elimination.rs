use std::num::NonZeroU32;

use plugin_factory::{Plugin, compatibility::Declaration};
use serde::{Deserialize, Serialize};

use crate::{
    entities::{Annotated, Bye, Match, MatchResult, PhaseData, RankingId, Team},
    plugins::{
        Phrase,
        facts::MatchAnnotation,
        metrics_calculator::{Calculator, Cell, MetricColumn, OldMetrics, ToCell, provides_metric},
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

#[typetag::serde(name = "tournament_engine::round_of_elimination")]
impl TypeMapValue for RoundOfElimination {}

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
        declaration
            .require::<MatchAnnotation<Eliminates>>("an elimination annotation on the matches");
    }
}

impl Calculator for RoundOfEliminationCalculator {
    type Metric = RoundOfElimination;

    fn after_match(
        &mut self,
        phase_data: &PhaseData,
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
            // a ranking that is already out stays out since the round it was knocked out in
            .filter(|r_id| {
                let eliminated = phase_data.rankings()[*r_id].metric::<RoundOfElimination>();
                eliminated.unwrap().0.is_none()
            })
            .map(|r_id| {
                let eliminated = RoundOfElimination(Some(r#match.round()));
                (*r_id, eliminated)
            })
            .collect()
    }

    /// A bye knocks out nobody.
    fn after_bye(
        &mut self,
        _: &PhaseData,
        _: &Bye,
        _: &OldMetrics,
    ) -> Vec<(RankingId, RoundOfElimination)> {
        vec![]
    }

    fn column(&self) -> Option<MetricColumn> {
        Some(MetricColumn::new::<RoundOfElimination>("out in"))
    }
}

crate::register_calculator!(RoundOfEliminationCalculator);

#[cfg(test)]
mod tests;
