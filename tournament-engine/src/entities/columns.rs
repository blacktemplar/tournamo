//! What the tables of standings of a phase show: a column per metric, see [`Phase::shown`].
//!
//! By rule a table shows the metrics its criteria rank by and the metrics these are calculated
//! from, however far back ([`by_rule`]); what the operator said otherwise is kept by the phase as
//! its [`Columns`].
//!
//! [`Phase::shown`]: crate::Phase::shown

use plugin_factory::catalog::{AnyAxis, PluginEntry};
use serde::{Deserialize, Serialize};

use crate::plugins::{
    metrics_calculator::{MetricEntry, MetricsCalculator},
    ranker::SliceRanker,
};

/// A table of standings of a phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Table {
    /// The standings of a division, ranked by the phase's
    /// [`ranking`](crate::PhasePlugins::ranking).
    Division,
    /// The rows of one place compared across parallel divisions, ranked by the phase's
    /// [`across`](crate::PhasePlugins::across).
    Across,
}

impl Table {
    /// Both tables.
    pub const ALL: [Self; 2] = [Self::Division, Self::Across];
}

/// Whether the tables show a metric, where the operator said so; `None` follows the rule.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shown {
    pub division: Option<bool>,
    pub across: Option<bool>,
}

impl Shown {
    /// What the operator said about `table`.
    pub fn of(self, table: Table) -> Option<bool> {
        match table {
            Table::Division => self.division,
            Table::Across => self.across,
        }
    }

    /// Whether both tables follow the rule.
    pub fn is_rule(self) -> bool {
        self.division.is_none() && self.across.is_none()
    }

    fn set(&mut self, table: Table, shown: Option<bool>) {
        match table {
            Table::Division => self.division = shown,
            Table::Across => self.across = shown,
        }
    }
}

/// What the operator said about the columns of a phase: at most one [`Shown`] per metric, by its
/// [id](crate::plugins::metrics_calculator::MetricId::ID), none following the rule in both
/// tables. Serialized as a sequence of `(id, Shown)`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "Vec<(String, Shown)>")]
pub struct Columns(Vec<(String, Shown)>);

impl Columns {
    /// Columns following the rule everywhere.
    pub fn new() -> Self {
        Self::default()
    }

    /// What the operator said about the metric `metric`.
    pub fn get(&self, metric: &str) -> Shown {
        self.0
            .iter()
            .find(|(id, _)| id == metric)
            .map_or_else(Shown::default, |&(_, shown)| shown)
    }

    /// The operator says that `table` shows the metric `metric` if `shown` is `Some`, or that it
    /// follows the rule.
    pub fn set(&mut self, metric: &str, table: Table, shown: Option<bool>) {
        let mut entry = self.get(metric);
        entry.set(table, shown);
        let at = self.0.iter().position(|(id, _)| id == metric);
        match (at, entry.is_rule()) {
            (Some(at), true) => {
                self.0.remove(at);
            }
            (Some(at), false) => self.0[at].1 = entry,
            (None, true) => {}
            (None, false) => self.0.push((metric.to_owned(), entry)),
        }
    }

    /// The metrics the operator said something about, by id, in the order they were first named.
    pub fn iter(&self) -> impl Iterator<Item = (&str, Shown)> {
        self.0.iter().map(|(id, shown)| (id.as_str(), *shown))
    }

    /// Whether the rule holds everywhere.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// These columns without the entries of metrics that `keep` refuses.
    pub fn retain(&mut self, mut keep: impl FnMut(&str) -> bool) {
        self.0.retain(|(id, _)| keep(id));
    }
}

impl From<Vec<(String, Shown)>> for Columns {
    fn from(entries: Vec<(String, Shown)>) -> Self {
        let mut columns = Self::new();
        for (id, shown) in entries {
            for table in Table::ALL {
                columns.set(&id, table, shown.of(table));
            }
        }
        columns
    }
}

/// Why a table shows a metric by rule, see [`by_rule`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rule {
    /// The criterion of this id ranks the table by the metric.
    RankedBy(&'static str),
    /// The metric of this id, shown by rule, is calculated from the metric.
    CalculatedFrom(&'static str),
}

/// The metrics of the calculators `calculators` that a table ranked by the slice rankers
/// `criteria` shows by rule, each by its id with why: those a criterion ranks by, in the order of
/// the criteria, then those a metric so shown is calculated from, however far back, in the order
/// of their calculators in `calculators`. Plugins are named by id; one that is not registered, or
/// a calculator of no registered metric, is left out. Whether a metric offers a column is not
/// asked.
///
/// What a plugin ranks by or is calculated from is read from the static declarations, see
/// [`PluginEntry::providers_in`].
pub fn by_rule(criteria: &[&str], calculators: &[&str]) -> Vec<(&'static str, Rule)> {
    let axis = AnyAxis::of::<dyn MetricsCalculator>();
    let rankers = AnyAxis::of::<dyn SliceRanker>();
    // the registered calculators of `calculators`, in order, each with its metric
    let calculated: Vec<(&'static PluginEntry, MetricEntry)> = calculators
        .iter()
        .filter_map(|id| {
            let calculator = axis.entry(id)?;
            Some((calculator, MetricEntry::of_calculator(calculator)?))
        })
        .collect();
    // the metrics calculated that `entry` requires
    let required = |entry: &PluginEntry| -> Vec<&'static str> {
        let providers = entry.providers_in(axis);
        calculated
            .iter()
            .filter(|(calculator, _)| {
                providers
                    .iter()
                    .any(|provider| provider.id() == calculator.id())
            })
            .map(|(_, metric)| metric.id())
            .collect()
    };
    let mut shown: Vec<(&'static str, Rule)> = Vec::new();
    for criterion in criteria.iter().filter_map(|id| rankers.entry(id)) {
        for metric in required(criterion) {
            if !shown.iter().any(|(id, _)| *id == metric) {
                shown.push((metric, Rule::RankedBy(criterion.id())));
            }
        }
    }
    let ranked = shown.len();
    let mut next = 0;
    while let Some(&(id, _)) = shown.get(next) {
        next += 1;
        let Some((calculator, _)) = calculated.iter().find(|(_, metric)| metric.id() == id) else {
            continue;
        };
        for metric in required(calculator) {
            if !shown.iter().any(|(id, _)| *id == metric) {
                shown.push((metric, Rule::CalculatedFrom(id)));
            }
        }
    }
    shown[ranked..]
        .sort_by_key(|(id, _)| calculated.iter().position(|(_, metric)| metric.id() == *id));
    shown
}
