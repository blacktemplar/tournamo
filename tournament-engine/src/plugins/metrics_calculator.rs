use std::{
    any::{Any, TypeId},
    cmp::Ordering,
    fmt::{self, Debug, Display, Formatter},
    num::NonZeroU32,
};

use dyn_clone::DynClone;
use plugin_factory::{Plugin, axis, compatibility::Declaration};

use crate::{
    Score,
    entities::{Bye, Match, PhaseData, Ranking, RankingId, Team},
    map::Map,
    plugins::{Phrase, facts::CalculatesMetric},
    type_map::{TypeMapValue, downcast_unchecked_ref},
};

pub mod buchholz;
pub mod games_balance;
pub mod points;
pub mod points_balance;
pub mod round_of_elimination;
pub mod round_of_first_loss;
#[cfg(test)]
pub(crate) mod test_utils;
pub mod wins_draws_losses;

pub use buchholz::{
    Buchholz, BuchholzCalculator, BuchholzOfBuchholz, BuchholzOfBuchholzCalculator, MoreBuchholz,
    MoreBuchholzOfBuchholz, MoreSonnebornBerger, SonnebornBerger, SonnebornBergerCalculator,
};
pub use games_balance::{
    GamesBalance, GamesBalanceCalculator, GamesDifference, GamesDifferenceCalculator,
    HigherGamesDifference, MoreGamesWon,
};
pub use points::{MorePoints, Points, PointsCalculator};
pub use points_balance::{
    HigherPointsDifference, MorePointsScored, PointsBalance, PointsBalanceCalculator,
    PointsDifference, PointsDifferenceCalculator,
};
pub use round_of_elimination::{
    Eliminates, LaterElimination, RoundOfElimination, RoundOfEliminationCalculator,
};
pub use round_of_first_loss::{LaterFirstLoss, RoundOfFirstLoss, RoundOfFirstLossCalculator};
pub use wins_draws_losses::{
    FewerLosses, FewerMatchesPlayed, MoreDraws, MoreWins, WinsDrawsLosses,
    WinsDrawsLossesCalculator,
};

pub type MetricId = TypeId;

pub type Metric = Box<dyn TypeMapValue>;

/// The values of metrics that the calculators which already ran for an update replaced, keyed by
/// metric type.
#[derive(Debug, Default)]
pub struct OldMetrics(Map<MetricId, Vec<(RankingId, Metric)>>);

impl OldMetrics {
    /// The replaced values of the metric `M`, each with the ranking it belonged to, in the order
    /// they were replaced.
    pub fn of<M: TypeMapValue>(&self) -> impl Iterator<Item = (RankingId, &M)> {
        self.0
            .get(&TypeId::of::<M>())
            .into_iter()
            .flatten()
            .map(|(ranking_id, metric)| {
                let metric: &dyn Any = &**metric;
                // SAFETY: `push` stores every metric under the type id of its concrete type, so
                // the metrics at key `TypeId::of::<M>()` are `M`s.
                (*ranking_id, unsafe { downcast_unchecked_ref::<M>(metric) })
            })
    }

    /// Records `metric` as the value of `ranking_id` that got replaced.
    pub(crate) fn push(&mut self, ranking_id: RankingId, metric: Metric) {
        let metric_id = (&*metric as &dyn Any).type_id();
        self.0
            .entry(metric_id)
            .or_default()
            .push((ranking_id, metric));
    }
}

/// Calculates one metric of every ranking of a phase.
///
/// A calculator of a single metric is best written as a [`Calculator`], which states its metric as
/// a type. Implemented directly, every metric a calculator returns must have the type of its
/// [`Self::default_metric_value`], and it has to declare that it provides the [`CalculatesMetric`]
/// of that metric.
///
/// A calculator has to declare that it requires every fact it reads, in particular the
/// [`CalculatesMetric`] of every other metric it reads. It runs after the calculators providing the
/// facts it requires.
///
/// [`CalculatesMetric`]: super::facts::CalculatesMetric
///
/// A calculator may keep state as a [`PhaseFormat`](super::phase_format::PhaseFormat) does.
#[axis]
pub trait MetricsCalculator: Plugin + DynClone {
    /// The value every ranking of a phase starts out with, whose type is the metric this
    /// calculator calculates.
    fn default_metric_value(&self) -> Metric;

    /// The new values of all rankings whose metric the result just reported for `match` changed,
    /// see [`Calculator::after_match`].
    fn update_after_match_result(
        &mut self,
        phase_data: &PhaseData,
        r#match: &Match,
        old_metrics: &OldMetrics,
    ) -> Vec<(RankingId, Metric)>;

    /// The new values of all rankings whose metric the `bye` changed, see
    /// [`Calculator::after_bye`].
    fn update_after_bye(
        &mut self,
        phase_data: &PhaseData,
        bye: &Bye,
        old_metrics: &OldMetrics,
    ) -> Vec<(RankingId, Metric)>;

    /// The column showing this calculator's metric in a table of standings, or `None` if the
    /// metric is not meant to be shown.
    fn column(&self) -> Option<MetricColumn> {
        None
    }
}

dyn_clone::clone_trait_object!(MetricsCalculator);

/// A [`MetricsCalculator`] of the single metric [`Self::Metric`], which every ranking starts out
/// with as its default value.
///
/// A calculator declares that it provides its metric with [`provides_metric`], as its static
/// declaration or as part of it, and registers as a [`MetricsCalculator`] with
/// [`register_calculator!`](crate::register_calculator).
///
/// # Examples
/// ```
/// use plugin_factory::Plugin;
/// use serde::{Deserialize, Serialize};
/// use tournament_engine::{
///     Bye, Match, PhaseData, RankingId, TypeMapValue,
///     plugins::metrics_calculator::{Calculator, OldMetrics, provides_metric},
///     register_calculator,
/// };
///
/// /// How many matches a ranking played.
/// #[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
/// struct MatchesPlayed(u32);
///
/// #[typetag::serde(name = "my_crate::matches_played")]
/// impl TypeMapValue for MatchesPlayed {}
///
/// #[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
/// #[plugin(
///     id = "my_crate::matches_played",
///     label = "matches played",
///     description = "how many matches an entrant played",
///     declare_static = provides_metric::<Self>
/// )]
/// struct MatchesPlayedCalculator;
///
/// impl Calculator for MatchesPlayedCalculator {
///     type Metric = MatchesPlayed;
///
///     fn after_match(
///         &mut self,
///         phase_data: &PhaseData,
///         r#match: &Match,
///         _: &OldMetrics,
///     ) -> Vec<(RankingId, MatchesPlayed)> {
///         r#match
///             .rankings()
///             .map(|r_id| {
///                 let played = phase_data.rankings()[r_id].played_matches().len();
///                 (*r_id, MatchesPlayed(played as u32))
///             })
///             .collect()
///     }
///
///     /// A bye is not a match.
///     fn after_bye(
///         &mut self,
///         _: &PhaseData,
///         _: &Bye,
///         _: &OldMetrics,
///     ) -> Vec<(RankingId, MatchesPlayed)> {
///         Vec::new()
///     }
/// }
///
/// register_calculator!(MatchesPlayedCalculator);
/// ```
pub trait Calculator: Debug + Clone {
    /// The metric this calculator calculates.
    type Metric: TypeMapValue + Default;

    /// The new values of all rankings whose metric the result just reported for `match` changed.
    /// A match is reported once, so `match` was not finished before.
    ///
    /// `old_metrics` holds the values replaced by the calculators that already ran, so that a
    /// calculator deriving its metric from another one can see what changed there.
    fn after_match(
        &mut self,
        phase_data: &PhaseData,
        r#match: &Match,
        old_metrics: &OldMetrics,
    ) -> Vec<(RankingId, Self::Metric)>;

    /// The new values of all rankings whose metric the `bye` changed, which its ranking has just
    /// [reached](Bye::is_reached).
    ///
    /// A bye is not a match: it has no opponent and no result, and once reached it never changes.
    /// Whether it counts for this metric at all is up to the calculator.
    ///
    /// `old_metrics` is as for [`Self::after_match`].
    fn after_bye(
        &mut self,
        phase_data: &PhaseData,
        bye: &Bye,
        old_metrics: &OldMetrics,
    ) -> Vec<(RankingId, Self::Metric)>;

    /// The column showing [`Self::Metric`] in a table of standings, or `None` if it is not meant to
    /// be shown.
    fn column(&self) -> Option<MetricColumn> {
        None
    }
}

/// Declares that the calculator `C` provides the [`CalculatesMetric`] of its metric. Given to the
/// derive of a calculator as `declare_static = provides_metric::<Self>`, or called by its static
/// declaration.
///
/// [`CalculatesMetric`]: super::facts::CalculatesMetric
pub fn provides_metric<C: Calculator>(declaration: &mut Declaration) {
    declaration.provide(CalculatesMetric::<C::Metric>::new());
}

/// The default value of the metric of `C`, boxed, for [`register_calculator!`].
#[doc(hidden)]
pub fn default_metric<C: Calculator>() -> Metric {
    Box::new(C::Metric::default())
}

/// `metrics`, each boxed, for [`register_calculator!`].
#[doc(hidden)]
pub fn boxed<M: TypeMapValue>(metrics: Vec<(RankingId, M)>) -> Vec<(RankingId, Metric)> {
    metrics
        .into_iter()
        .map(|(ranking_id, metric)| (ranking_id, Box::new(metric) as Metric))
        .collect()
}

/// Registers the [`Calculator`] `$calculator` as a [`MetricsCalculator`], as
/// [`plugin_factory::register`] registers a plugin; the crate calling it has to depend on
/// `plugin_factory` and `typetag`. See [`Calculator`] for an example.
#[macro_export]
macro_rules! register_calculator {
    ($calculator:ty) => {
        #[::plugin_factory::register]
        impl $crate::plugins::metrics_calculator::MetricsCalculator for $calculator {
            fn default_metric_value(&self) -> $crate::plugins::metrics_calculator::Metric {
                $crate::plugins::metrics_calculator::default_metric::<Self>()
            }

            fn update_after_match_result(
                &mut self,
                phase_data: &$crate::PhaseData,
                r#match: &$crate::Match,
                old_metrics: &$crate::plugins::metrics_calculator::OldMetrics,
            ) -> ::std::vec::Vec<(
                $crate::RankingId,
                $crate::plugins::metrics_calculator::Metric,
            )> {
                $crate::plugins::metrics_calculator::boxed(
                    $crate::plugins::metrics_calculator::Calculator::after_match(
                        self,
                        phase_data,
                        r#match,
                        old_metrics,
                    ),
                )
            }

            fn update_after_bye(
                &mut self,
                phase_data: &$crate::PhaseData,
                bye: &$crate::Bye,
                old_metrics: &$crate::plugins::metrics_calculator::OldMetrics,
            ) -> ::std::vec::Vec<(
                $crate::RankingId,
                $crate::plugins::metrics_calculator::Metric,
            )> {
                $crate::plugins::metrics_calculator::boxed(
                    $crate::plugins::metrics_calculator::Calculator::after_bye(
                        self,
                        phase_data,
                        bye,
                        old_metrics,
                    ),
                )
            }

            fn column(
                &self,
            ) -> ::std::option::Option<$crate::plugins::metrics_calculator::MetricColumn> {
                $crate::plugins::metrics_calculator::Calculator::column(self)
            }
        }
    };
}

/// The metric `M` of every ranking of `match`, as `update` changes it for the ranking's team.
fn updated_for_each_team<M: TypeMapValue + Copy>(
    phase_data: &PhaseData,
    r#match: &Match,
    update: impl Fn(&mut M, Team),
) -> Vec<(RankingId, M)> {
    let mut updated = Vec::new();
    for team in [Team::A, Team::B] {
        for r_id in r#match.rankings_of(team) {
            let mut metric = *phase_data.rankings()[r_id].metric::<M>().unwrap();
            update(&mut metric, team);
            updated.push((*r_id, metric));
        }
    }
    updated
}

/// The metric `D`, as `derive` makes it of the metric `M`, of every ranking whose `M` the
/// calculators that ran before changed.
fn derived<M: TypeMapValue, D>(
    phase_data: &PhaseData,
    old_metrics: &OldMetrics,
    derive: impl Fn(&M) -> D,
) -> Vec<(RankingId, D)> {
    old_metrics
        .of::<M>()
        .map(|(r_id, _)| {
            (
                r_id,
                derive(phase_data.rankings()[&r_id].metric::<M>().unwrap()),
            )
        })
        .collect()
}

/// A column of a table of standings showing one metric: its header and the cell of every
/// ranking's value.
#[derive(Debug, Clone, Copy)]
pub struct MetricColumn {
    header: &'static str,
    cell: fn(&Ranking) -> Option<Cell>,
}

impl MetricColumn {
    /// The column headed `header` that shows the metric `M` of a ranking as its [`ToCell`].
    pub fn new<M: TypeMapValue + ToCell>(header: &'static str) -> Self {
        Self {
            header,
            cell: |ranking| ranking.metric::<M>().map(M::to_cell),
        }
    }

    pub fn header(&self) -> &'static str {
        self.header
    }

    /// The cell of the metric of `ranking`, or `None` if `ranking` has no such metric.
    pub fn cell(&self, ranking: &Ranking) -> Option<Cell> {
        (self.cell)(ranking)
    }
}

/// A metric shown in a [`MetricColumn`].
pub trait ToCell {
    /// What its cell shows.
    fn to_cell(&self) -> Cell;
}

/// The sign a [`Cell::Signed`] shows before the magnitude of `number`: none for zero.
pub fn sign(number: Score) -> &'static str {
    match number.cmp(&Score::ZERO) {
        Ordering::Greater => "+",
        Ordering::Less => "\u{2212}",
        Ordering::Equal => "",
    }
}

/// What a cell of a [`MetricColumn`] shows. Shows as it does in English.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Cell {
    /// A number, which a frontend writes as its language writes numbers.
    Number(Score),
    /// A number with its sign, such as a difference: `+5`, `−3` (with a minus sign) or `0`.
    /// A frontend writes the number without its sign as its language writes numbers.
    Signed(Score),
    /// A round of the phase, e.g. the one a ranking was knocked out in. A frontend may name it by
    /// the stage of the ranking's match or bye in that round, where the format names one.
    Round(NonZeroU32),
    /// A phrase of the calculator's, which a translation finds by the calculator's id.
    Text(Phrase),
}

impl Display for Cell {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Number(number) => Display::fmt(number, f),
            Self::Signed(number) => write!(f, "{}{}", sign(*number), number.get().abs()),
            Self::Round(round) => write!(f, "round {round}"),
            Self::Text(phrase) => Display::fmt(phrase, f),
        }
    }
}

#[cfg(test)]
mod tests {
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::*;

    #[test]
    fn a_signed_cell_shows_the_sign_of_every_number_but_zero() {
        let shown = |number| Cell::Signed(Score::new(number)).to_string();

        assert_eq!(shown(5.0), "+5");
        assert_eq!(shown(-3.0), "\u{2212}3");
        assert_eq!(shown(-0.5), "\u{2212}0.5");
        assert_eq!(shown(0.0), "0");
    }
}
