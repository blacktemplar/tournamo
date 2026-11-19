use std::{
    any::{Any, TypeId},
    cmp::Ordering,
    fmt::{self, Debug, Display, Formatter},
    num::NonZeroU32,
};

use dyn_clone::DynClone;
use plugin_factory::{
    Plugin, axis,
    catalog::{AnyAxis, PluginEntry},
    compatibility::Declaration,
};

use crate::{
    Score,
    entities::{Bye, DivisionData, Match, Ranking, RankingId, Team},
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
pub use points::{
    MorePoints, MorePointsPerMatch, Points, PointsCalculator, PointsPerMatch,
    PointsPerMatchCalculator,
};
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

/// A metric of a ranking, boxed, of any type.
pub type BoxedMetric = Box<dyn TypeMapValue>;

/// The values of metrics that the calculators which already ran for an update replaced, keyed by
/// metric type.
#[derive(Debug, Default)]
pub struct OldMetrics(Map<TypeId, Vec<(RankingId, BoxedMetric)>>);

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
    pub(crate) fn push(&mut self, ranking_id: RankingId, metric: BoxedMetric) {
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
    fn default_metric_value(&self) -> BoxedMetric;

    /// The new values of all rankings whose metric the result just reported for `match` changed,
    /// see [`Calculator::after_match`].
    fn update_after_match_result(
        &mut self,
        phase_data: &DivisionData,
        r#match: &Match,
        old_metrics: &OldMetrics,
    ) -> Vec<(RankingId, BoxedMetric)>;

    /// The new values of all rankings whose metric the `bye` changed, see
    /// [`Calculator::after_bye`].
    fn update_after_bye(
        &mut self,
        phase_data: &DivisionData,
        bye: &Bye,
        old_metrics: &OldMetrics,
    ) -> Vec<(RankingId, BoxedMetric)>;

    /// The metric this calculator calculates, the type of its [`Self::default_metric_value`].
    fn metric(&self) -> MetricEntry;
}

dyn_clone::clone_trait_object!(MetricsCalculator);

/// A [`MetricsCalculator`] of the single [`Metric`] [`Self::Metric`], which every ranking starts
/// out with as its default value.
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
///     Bye, DivisionData, Match, RankingId, metric,
///     plugins::metrics_calculator::{Calculator, Metric, OldMetrics, provides_metric},
///     register_calculator,
/// };
///
/// /// How many matches a ranking played.
/// #[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
/// struct MatchesPlayed(u32);
///
/// impl Metric for MatchesPlayed {
///     const LABEL: &'static str = "matches played";
///     const DESCRIPTION: Option<&'static str> = Some("how many matches an entrant played");
/// }
///
/// metric!(MatchesPlayed, id = "my_crate::matches_played");
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
///         phase_data: &DivisionData,
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
///         _: &DivisionData,
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
    type Metric: Metric;

    /// The new values of all rankings whose metric the result just reported for `match` changed.
    /// A match is reported once, so `match` was not finished before.
    ///
    /// `old_metrics` holds the values replaced by the calculators that already ran, so that a
    /// calculator deriving its metric from another one can see what changed there.
    fn after_match(
        &mut self,
        phase_data: &DivisionData,
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
        phase_data: &DivisionData,
        bye: &Bye,
        old_metrics: &OldMetrics,
    ) -> Vec<(RankingId, Self::Metric)>;
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
pub fn default_metric<C: Calculator>() -> BoxedMetric {
    Box::new(C::Metric::default())
}

/// `metrics`, each boxed, for [`register_calculator!`].
#[doc(hidden)]
pub fn boxed<M: TypeMapValue>(metrics: Vec<(RankingId, M)>) -> Vec<(RankingId, BoxedMetric)> {
    metrics
        .into_iter()
        .map(|(ranking_id, metric)| (ranking_id, Box::new(metric) as BoxedMetric))
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
            fn default_metric_value(&self) -> $crate::plugins::metrics_calculator::BoxedMetric {
                $crate::plugins::metrics_calculator::default_metric::<Self>()
            }

            fn update_after_match_result(
                &mut self,
                phase_data: &$crate::DivisionData,
                r#match: &$crate::Match,
                old_metrics: &$crate::plugins::metrics_calculator::OldMetrics,
            ) -> ::std::vec::Vec<(
                $crate::RankingId,
                $crate::plugins::metrics_calculator::BoxedMetric,
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
                phase_data: &$crate::DivisionData,
                bye: &$crate::Bye,
                old_metrics: &$crate::plugins::metrics_calculator::OldMetrics,
            ) -> ::std::vec::Vec<(
                $crate::RankingId,
                $crate::plugins::metrics_calculator::BoxedMetric,
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

            fn metric(&self) -> $crate::plugins::metrics_calculator::MetricEntry {
                $crate::plugins::metrics_calculator::MetricEntry::of::<
                    <Self as $crate::plugins::metrics_calculator::Calculator>::Metric,
                >()
            }
        }
    };
}

/// The metric `M` of every ranking of `match`, as `update` changes it for the ranking's team.
fn updated_for_each_team<M: TypeMapValue + Copy>(
    phase_data: &DivisionData,
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
    phase_data: &DivisionData,
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

/// The id of a metric, see [`Metric`]. Implemented by [`metric!`](crate::metric) alone.
pub trait MetricId {
    /// What identifies the metric, also its tag as a [`TypeMapValue`]: a persistence contract,
    /// never changed. Its texts are translated by it.
    const ID: &'static str;
}

/// What a [`Calculator`] calculates of every ranking of a phase, which a table of standings may
/// show and a criterion may rank by, e.g. the points.
///
/// A metric is a type that implements `Metric` and is declared with [`metric!`](crate::metric),
/// which gives it its [id](MetricId::ID). Its label and description follow "Labelling a plugin"
/// in the [`plugins`](crate::plugins) docs, as a calculator's do.
///
/// # Examples
/// ```
/// use serde::{Deserialize, Serialize};
/// use tournament_engine::{
///     Score, metric,
///     plugins::metrics_calculator::{Cell, Metric, MetricColumn, ToCell},
/// };
///
/// /// How many matches a ranking played.
/// #[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
/// struct MatchesPlayed(u32);
///
/// impl Metric for MatchesPlayed {
///     const LABEL: &'static str = "matches played";
///     const DESCRIPTION: Option<&'static str> = Some("how many matches an entrant played");
///
///     fn column() -> Option<MetricColumn> {
///         Some(MetricColumn::new::<Self>("played"))
///     }
/// }
///
/// impl ToCell for MatchesPlayed {
///     fn to_cell(&self) -> Cell {
///         Cell::Number(Score::new(self.0.into()))
///     }
/// }
///
/// metric!(MatchesPlayed, id = "my_crate::matches_played");
/// ```
pub trait Metric: MetricId + TypeMapValue + Default {
    /// What an operator choosing the metric reads, as a column of a ranking would be named.
    const LABEL: &'static str;

    /// What an operator choosing the metric reads about what it counts, if anything.
    const DESCRIPTION: Option<&'static str>;

    /// The column showing the metric in a table of standings, or `None` if it is not meant to be
    /// shown.
    fn column() -> Option<MetricColumn> {
        None
    }
}

/// Declares the type `$metric`, which implements [`Metric`], a metric of the id `$id`: implements
/// [`MetricId`] and [`TypeMapValue`] (tagged with the id) for it and registers it, so that
/// [`MetricEntry::all`] lists it. The crate calling it has to depend on `typetag`.
#[macro_export]
macro_rules! metric {
    ($metric:ty, id = $id:literal) => {
        impl $crate::plugins::metrics_calculator::MetricId for $metric {
            const ID: &'static str = $id;
        }

        #[::typetag::serde(name = $id)]
        impl $crate::TypeMapValue for $metric {}

        $crate::__private::inventory::submit! {
            $crate::plugins::metrics_calculator::MetricEntry::of::<$metric>()
        }
    };
}

/// A [`Metric`] as registered by [`metric!`](crate::metric): what is known of it without a value
/// of it.
#[derive(Clone, Copy)]
pub struct MetricEntry {
    id: &'static str,
    label: &'static str,
    description: Option<&'static str>,
    column: fn() -> Option<MetricColumn>,
    // `TypeId::of` is not a `const fn` yet (it is unstable)
    type_id: fn() -> TypeId,
    fact: fn() -> TypeId,
}

inventory::collect!(MetricEntry);

impl MetricEntry {
    /// The entry of the metric `M`.
    pub const fn of<M: Metric>() -> Self {
        Self {
            id: M::ID,
            label: M::LABEL,
            description: M::DESCRIPTION,
            column: M::column,
            type_id: TypeId::of::<M>,
            fact: TypeId::of::<CalculatesMetric<M>>,
        }
    }

    /// Every metric registered that a registered [`MetricsCalculator`] calculates, in the order
    /// of their ids.
    pub fn all() -> Vec<MetricEntry> {
        let mut all: Vec<_> = inventory::iter::<MetricEntry>
            .into_iter()
            .filter(|metric| !metric.calculators().is_empty())
            .copied()
            .collect();
        all.sort_by_key(|metric| metric.id);
        all
    }

    /// The metric of the id `id`, if it is registered and calculated, see [`Self::all`].
    pub fn with_id(id: &str) -> Option<MetricEntry> {
        Self::all().into_iter().find(|metric| metric.id == id)
    }

    /// The metric the calculator `calculator` calculates whatever its configuration, if it is
    /// registered, read from the calculator's static declaration.
    pub fn of_calculator(calculator: &PluginEntry) -> Option<MetricEntry> {
        inventory::iter::<MetricEntry>
            .into_iter()
            .find(|metric| calculator.provides((metric.fact)()))
            .copied()
    }

    /// See [`MetricId::ID`].
    pub fn id(&self) -> &'static str {
        self.id
    }

    /// See [`Metric::LABEL`].
    pub fn label(&self) -> &'static str {
        self.label
    }

    /// See [`Metric::DESCRIPTION`].
    pub fn description(&self) -> Option<&'static str> {
        self.description
    }

    /// See [`Metric::column`].
    pub fn column(&self) -> Option<MetricColumn> {
        (self.column)()
    }

    /// The type of the metric.
    pub fn type_id(&self) -> TypeId {
        (self.type_id)()
    }

    /// The registered calculators that calculate the metric whatever their configuration, in the
    /// order of their ids, read from their static declarations.
    pub fn calculators(&self) -> Vec<&'static PluginEntry> {
        let fact = (self.fact)();
        AnyAxis::of::<dyn MetricsCalculator>()
            .entries()
            .filter(|calculator| calculator.provides(fact))
            .collect()
    }
}

impl PartialEq for MetricEntry {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for MetricEntry {}

impl Debug for MetricEntry {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.id)
    }
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
    pub fn new<M: Metric + ToCell>(header: &'static str) -> Self {
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
    /// A phrase of the metric's, which a translation finds by the metric's id.
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
    use plugin_factory::PluginType;
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::*;

    #[test]
    fn a_metric_is_found_by_its_id_and_by_its_calculators() {
        let points = MetricEntry::of::<Points>();
        let calculator = AnyAxis::of::<dyn MetricsCalculator>()
            .entry(PointsCalculator::ID)
            .unwrap();

        assert_eq!(points.id(), "tournament_engine::points");
        assert_eq!(MetricEntry::with_id(points.id()), Some(points));
        assert_eq!(MetricEntry::of_calculator(calculator), Some(points));
        let calculators: Vec<_> = points
            .calculators()
            .iter()
            .map(|entry| entry.id())
            .collect();
        assert_eq!(calculators, [PointsCalculator::ID]);
        assert!(MetricEntry::all().contains(&points));
        assert_eq!(
            PointsCalculator::new(Score::ONE, Score::ZERO).metric(),
            points
        );
    }

    #[test]
    fn a_signed_cell_shows_the_sign_of_every_number_but_zero() {
        let shown = |number| Cell::Signed(Score::new(number)).to_string();

        assert_eq!(shown(5.0), "+5");
        assert_eq!(shown(-3.0), "\u{2212}3");
        assert_eq!(shown(-0.5), "\u{2212}0.5");
        assert_eq!(shown(0.0), "0");
    }
}
