//! The metrics of a phase in its form, a row per metric: those the operator added
//! ([`PhaseForm::add_metric`]) and those something needs, e.g. a criterion ranking by it. Where
//! they are shown is said apart from that ([`PhaseForm::set_shown`]).
//!
//! After every change the form settles the metrics: it removes those neither added nor needed any
//! more, then adds a calculator for each metric added or needed that none calculates: the one
//! chosen for it before, else the only one, else the one the format suggests, else the only one
//! without fields, as the phase would be completed with it; with none of these, the metric stays
//! without one until the operator chooses ([`PhaseForm::choose_provider`]). What it removes it
//! remembers while it is open, and brings back as it was.

use std::collections::HashMap;

use plugin_factory::{
    catalog::{AnyAxis, PluginEntry},
    config::{ConfigError, Path, PluginConfig, Value},
    draft::Draft,
};
use tournament_engine::{
    PhasePlugins, Table,
    entities::columns::Rule,
    plugins::{metrics_calculator::MetricEntry, ranker::SliceRanker},
};

use super::{
    Because, By, FormView, Input, Item, ListInput, PhaseForm, PluginOption, ShownIn, answered,
    edit::Answering as _, suggested, table_index,
};

/// A metric as a row of the metrics of a phase, see [`ListInput::metrics`].
#[derive(Debug, Clone, PartialEq)]
pub struct MetricRow {
    pub metric: MetricOption,
    /// The calculator calculating it, with its fields and issues; `None` while there are several
    /// to choose from and none is chosen.
    pub item: Option<Item>,
    /// Where several calculators can calculate it, all of them, those the format suggests first,
    /// for [`PhaseForm::choose_provider`]; empty otherwise.
    pub providers: Vec<PluginOption>,
    /// Whether the operator added it, so that it can be removed unless something needs it.
    pub added: bool,
    /// The plugins of the phase that need it, in the order of the fields of the phase: criteria
    /// ranking by it, and calculators of other metrics calculated from it.
    pub needed_by: Vec<PluginOption>,
    /// Whether it offers a column at all; one that does not is shown in no table.
    pub column: bool,
    /// Whether the standings of a division, then the rows compared across divisions, show it.
    /// What the rows compared show does not matter while the phase is played in one division and
    /// compared by nothing, see [`PhasePlugins::ACROSS`].
    pub tables: [ShownIn; 2],
    /// Whether it is added, needed by nothing and shown in no table, so that calculating it does
    /// nothing.
    pub unused: bool,
}

impl MetricRow {
    /// Whether `table` shows it.
    pub fn of(&self, table: Table) -> &ShownIn {
        &self.tables[table_index(table)]
    }
}

/// A metric as a choice, see [`MetricEntry`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MetricOption {
    /// Its id, which the changes of [`PhaseForm`] take, and which its texts are translated by, see
    /// [`MetricText`](crate::i18n::MetricText).
    pub id: &'static str,
    /// The label in English.
    pub label: &'static str,
    /// What it counts, in English.
    pub description: Option<&'static str>,
}

impl MetricOption {
    pub(super) fn of(metric: MetricEntry) -> Self {
        Self {
            id: metric.id(),
            label: metric.label(),
            description: metric.description(),
        }
    }
}

impl FormView {
    /// The metrics the operator shows in `table` against the rule if `on`, else those hidden
    /// there against it, in the order of [`ListInput::metrics`], see [`PhaseForm::set_shown`].
    pub fn overridden(&self, table: Table, on: bool) -> Vec<MetricOption> {
        let metrics = Path::ROOT.field(PhasePlugins::METRICS);
        let Some(Input::List(list)) = self
            .fields
            .iter()
            .find(|field| field.path == metrics)
            .map(|field| &field.input)
        else {
            return Vec::new();
        };
        list.metrics
            .iter()
            .filter(|row| {
                *row.of(table)
                    == ShownIn {
                        on,
                        by: By::Operator,
                    }
            })
            .map(|row| row.metric)
            .collect()
    }
}

/// What a form remembers while it is open of the metrics it removed, to bring them back as they
/// were.
#[derive(Debug, Clone, Default)]
pub(super) struct Memory {
    /// The answer of every calculator removed, by its id.
    answers: HashMap<&'static str, Value>,
    /// The calculator chosen for a metric, by the metric's id.
    providers: HashMap<&'static str, &'static str>,
}

/// A calculator of the list of metrics of a draft.
struct Calculating {
    /// Its position in the list.
    index: usize,
    entry: &'static PluginEntry,
    /// The metric it calculates; `None` for a calculator of no registered metric, which stays.
    metric: Option<MetricEntry>,
}

fn metrics_path() -> Path {
    Path::ROOT.field(PhasePlugins::METRICS)
}

/// The calculators of the list of metrics of `draft`, in order.
fn calculating(draft: &Draft<PhasePlugins>) -> Vec<Calculating> {
    let path = metrics_path();
    draft
        .fields()
        .into_iter()
        .find(|node| node.path == path)
        .map(|node| {
            node.plugins()
                .into_iter()
                .enumerate()
                .map(|(index, plugin)| Calculating {
                    index,
                    entry: plugin.entry,
                    metric: MetricEntry::of_calculator(plugin.entry),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The plugins answering the fields of `draft` other than the list of metrics, in the order of
/// the fields.
fn others(draft: &Draft<PhasePlugins>) -> Vec<&'static PluginEntry> {
    let path = metrics_path();
    draft
        .fields()
        .into_iter()
        .filter(|node| node.path != path)
        .flat_map(|node| node.plugins())
        .map(|plugin| plugin.entry)
        .collect()
}

/// The metrics `draft` needs: those a plugin other than a calculator requires, then those the
/// calculators of the metrics needed or `added` (by id) require, however far back. Read from the
/// static declarations.
fn needed(draft: &Draft<PhasePlugins>, added: &[&str]) -> Vec<MetricEntry> {
    let metrics = MetricEntry::all();
    let others = others(draft);
    let calculating = calculating(draft);
    let mut needed: Vec<MetricEntry> = metrics
        .iter()
        .filter(|metric| others.iter().any(|plugin| metric.is_required_by(plugin)))
        .copied()
        .collect();
    // each round needs one more metric or ends, so this ends
    loop {
        let before = needed.len();
        for calculator in &calculating {
            let kept = calculator
                .metric
                .is_none_or(|metric| needed.contains(&metric) || added.contains(&metric.id()));
            if !kept {
                continue;
            }
            for metric in &metrics {
                if !needed.contains(metric) && metric.is_required_by(calculator.entry) {
                    needed.push(*metric);
                }
            }
        }
        if needed.len() == before {
            return needed;
        }
    }
}

/// The metrics of the calculators of `draft` that the operator is taken to have added: those
/// nothing in it needs.
pub(super) fn added_in(draft: &Draft<PhasePlugins>) -> Vec<&'static str> {
    let mut added: Vec<&'static str> = Vec::new();
    // the metrics nothing requires, then any that only require each other, so that a metric a
    // metric added requires counts as needed
    loop {
        let needed = needed(draft, &added);
        let Some(metric) = calculating(draft)
            .into_iter()
            .filter_map(|calculator| calculator.metric)
            .find(|metric| !needed.contains(metric) && !added.contains(&metric.id()))
        else {
            return added;
        };
        added.push(metric.id());
    }
}

/// `added` (metric ids), as the edit of `before` into `after` changes it: with the metrics of the
/// calculators it adds, without those of the calculators it removes.
pub(super) fn added_by_hand(
    before: &Draft<PhasePlugins>,
    after: &Draft<PhasePlugins>,
    added: &[&'static str],
) -> Vec<&'static str> {
    let metrics = |draft| -> Vec<&'static str> {
        calculating(draft)
            .into_iter()
            .filter_map(|calculator| calculator.metric.map(|metric| metric.id()))
            .collect()
    };
    let (before, after) = (metrics(before), metrics(after));
    added
        .iter()
        .filter(|metric| after.contains(metric) || !before.contains(metric))
        .copied()
        .chain(
            after
                .iter()
                .filter(|metric| !before.contains(metric) && !added.contains(metric))
                .copied(),
        )
        .collect()
}

impl PhaseForm {
    /// Adds the metric `metric` to the metrics of the phase, shown in the standings of the
    /// divisions, calculated by the calculator the form chooses on its own, see the
    /// [module](self); with none, it waits for [`Self::choose_provider`].
    ///
    /// # Errors
    /// Returns an error if no metric of this id is registered.
    pub fn add_metric(&mut self, metric: &str) -> Result<(), ConfigError> {
        let metric = MetricEntry::with_id(metric).ok_or_else(|| ConfigError::UnknownPlugin {
            path: metrics_path(),
        })?;
        let mut form = self.clone();
        if !form.added.contains(&metric.id()) {
            form.added.push(metric.id());
        }
        form.change(&metrics_path(), |_| Ok(()))?;
        *self = form;
        if !self.is_shown(metric.id(), Table::Division) {
            self.set_shown(metric.id(), Table::Division, true);
        }
        Ok(())
    }

    /// Removes the metric `metric`, which the operator added and nothing needs, from the metrics
    /// of the phase, with what was said about its columns; the form remembers its calculator as
    /// it was, see [`Self::add_metric`].
    ///
    /// # Errors
    /// Returns an error if the metric is not added, or something needs it.
    pub fn remove_metric(&mut self, metric: &str) -> Result<(), ConfigError> {
        let rejected = |reason: &str| ConfigError::Rejected {
            path: metrics_path(),
            reason: reason.to_owned(),
        };
        let Some(at) = self.added.iter().position(|added| *added == metric) else {
            return Err(rejected("the metric is not added"));
        };
        if needed(&self.draft, &[])
            .iter()
            .chain(&needed(&self.draft, &self.added))
            .any(|needed| needed.id() == metric)
        {
            return Err(rejected("the metric is needed"));
        }
        let mut form = self.clone();
        form.added.remove(at);
        form.change(&metrics_path(), |_| Ok(()))?;
        *self = form;
        Ok(())
    }

    /// Calculates the metric `metric`, added or needed, by the calculator `calculator`, configured
    /// as it was if the form removed it before, else as it offers; the calculator calculating it
    /// so far is removed and remembered.
    ///
    /// # Errors
    /// Returns an error if the metric is neither added nor needed, or `calculator` does not
    /// calculate it.
    pub fn choose_provider(&mut self, metric: &str, calculator: &str) -> Result<(), ConfigError> {
        let entry = MetricEntry::with_id(metric).filter(|metric| {
            self.added.contains(&metric.id()) || needed(&self.draft, &self.added).contains(metric)
        });
        let calculator = entry
            .and_then(|metric| {
                metric
                    .calculators()
                    .into_iter()
                    .find(|entry| entry.id() == calculator)
            })
            .ok_or_else(|| ConfigError::UnknownPlugin {
                path: metrics_path(),
            })?;
        let metric = entry.map(|metric| metric.id()).unwrap_or_default();
        let mut form = self.clone();
        form.memory.providers.insert(metric, calculator.id());
        let current: Vec<usize> = calculating(&form.draft)
            .into_iter()
            .filter(|calculating| calculating.metric.map(|metric| metric.id()) == Some(metric))
            .map(|calculating| calculating.index)
            .collect();
        // removed before the change, which takes the metric as still wanted and settles it with
        // the calculator chosen
        for index in current.into_iter().rev() {
            let at = metrics_path().index(index);
            form.remember(&at);
            form.draft.unset(&at)?;
        }
        form.change(&metrics_path(), |_| Ok(()))?;
        *self = form;
        Ok(())
    }

    /// Remembers the answer of the calculator at `at`, see [`Memory`].
    fn remember(&mut self, at: &Path) {
        if let Some(plugin) = self.draft.plugin_at(at)
            && let Some(answer) = self.draft.answers().get_at(at)
        {
            self.memory
                .answers
                .insert(plugin.entry.id(), answer.clone());
        }
    }

    /// Whether `table` shows `metric` as the form stands.
    fn is_shown(&self, metric: &str, table: Table) -> bool {
        self.columns.get(metric).of(table).unwrap_or_else(|| {
            self.rule(&self.draft, table)
                .iter()
                .any(|&(ruled, _)| ruled == metric)
        })
    }

    /// `draft` with the calculators of the metrics neither added nor needed removed, and one
    /// added for each metric added or needed that none calculates, where the form knows which,
    /// see the [module](self). What it removes, the form remembers.
    ///
    /// # Errors
    /// Returns an error if a calculator added does not take the answers remembered for it.
    pub(super) fn settle(
        &mut self,
        mut draft: Draft<PhasePlugins>,
    ) -> Result<Draft<PhasePlugins>, ConfigError> {
        let suggested = suggested(draft.clone())
            .map(|suggested| answered(&suggested, PhasePlugins::METRICS))
            .unwrap_or_default();
        let path = metrics_path();
        loop {
            let needed = needed(&draft, &self.added);
            let calculating = calculating(&draft);
            let stale: Vec<&Calculating> = calculating
                .iter()
                .filter(|calculator| {
                    calculator.metric.is_some_and(|metric| {
                        !needed.contains(&metric) && !self.added.contains(&metric.id())
                    })
                })
                .collect();
            if !stale.is_empty() {
                for calculator in stale.into_iter().rev() {
                    let at = path.clone().index(calculator.index);
                    if let Some(answer) = draft.answers().get_at(&at) {
                        self.memory
                            .answers
                            .insert(calculator.entry.id(), answer.clone());
                    }
                    draft.unset(&at)?;
                }
                continue;
            }
            let calculated: Vec<MetricEntry> = calculating
                .iter()
                .filter_map(|calculator| calculator.metric)
                .collect();
            let missing: Vec<MetricEntry> = needed
                .into_iter()
                .chain(self.added.iter().filter_map(|id| MetricEntry::with_id(id)))
                .filter(|metric| !calculated.contains(metric))
                .collect();
            let mut end = calculating.len();
            for metric in missing {
                let Some(calculator) = self.provider_of(metric, &suggested) else {
                    continue;
                };
                let answer = self
                    .memory
                    .answers
                    .get(calculator.id())
                    .cloned()
                    .unwrap_or_else(|| PluginConfig::new(calculator.id()).into());
                draft.set(&path.clone().index(end), answer)?;
                end += 1;
            }
            if end == calculating.len() {
                return draft.with_defaults();
            }
        }
    }

    /// The calculator of `metric` the form chooses on its own: the one chosen before, else the
    /// only one, else the one of `suggested`, the ids of the calculators the format suggests, else
    /// the only one without fields.
    fn provider_of(&self, metric: MetricEntry, suggested: &[&str]) -> Option<&'static PluginEntry> {
        let providers = metric.calculators();
        if let Some(chosen) = self.memory.providers.get(metric.id())
            && let Some(provider) = providers.iter().find(|entry| entry.id() == *chosen)
        {
            return Some(provider);
        }
        if let [only] = providers.as_slice() {
            return Some(only);
        }
        if let Some(provider) = providers
            .iter()
            .find(|provider| suggested.contains(&provider.id()))
        {
            return Some(provider);
        }
        // the only one without fields, which the phase would be completed with, see
        // `Draft::add_implied`
        match providers
            .iter()
            .filter(|provider| provider.schema().fields().is_empty())
            .collect::<Vec<_>>()
            .as_slice()
        {
            [fieldless] => Some(fieldless),
            _ => None,
        }
    }

    /// The rows of the metrics of `list`, the list of metrics as shown, see
    /// [`ListInput::metrics`].
    pub(super) fn metric_rows(&self, list: &ListInput) -> Vec<MetricRow> {
        let draft = &self.draft;
        let needed = needed(draft, &self.added);
        let calculating = calculating(draft);
        let suggested = suggested(draft.clone())
            .map(|suggested| answered(&suggested, PhasePlugins::METRICS))
            .unwrap_or_default();
        let mut metrics: Vec<MetricEntry> = calculating
            .iter()
            .filter_map(|calculator| calculator.metric)
            .chain(needed.iter().copied())
            .chain(self.added.iter().filter_map(|id| MetricEntry::with_id(id)))
            .collect();
        metrics.sort_by_cached_key(|metric| (metric.label().to_lowercase(), metric.id()));
        metrics.dedup();
        let rules = Table::ALL.map(|table| self.rule(draft, table));
        // what needs a metric: the plugins of the other fields, and the calculators of the
        // metrics kept
        let needers: Vec<&'static PluginEntry> = others(draft)
            .into_iter()
            .chain(
                calculating
                    .iter()
                    .filter(|calculator| {
                        calculator.metric.is_none_or(|metric| {
                            needed.contains(&metric) || self.added.contains(&metric.id())
                        })
                    })
                    .map(|calculator| calculator.entry),
            )
            .collect();
        metrics
            .into_iter()
            .map(|metric| {
                let said = self.columns.get(metric.id());
                let column = metric.column().is_some();
                let tables = Table::ALL.map(|table| {
                    let because = rules[table_index(table)]
                        .iter()
                        .find(|&&(ruled, _)| ruled == metric.id())
                        .and_then(|&(_, why)| because(why));
                    match said.of(table) {
                        Some(on) => ShownIn {
                            on: on && column,
                            by: By::Operator,
                        },
                        None => ShownIn {
                            on: because.is_some() && column,
                            by: By::Rule(because),
                        },
                    }
                });
                let calculator = calculating
                    .iter()
                    .find(|calculator| calculator.metric == Some(metric));
                let needed_by: Vec<PluginOption> = needers
                    .iter()
                    .filter(|needer| {
                        calculator.is_none_or(|calculator| calculator.entry.id() != needer.id())
                    })
                    .filter(|needer| metric.is_required_by(needer))
                    .map(|needer| PluginOption::of(needer, false))
                    .collect();
                let added = self.added.contains(&metric.id());
                let providers = metric.calculators();
                let mut providers: Vec<PluginOption> = if providers.len() > 1 {
                    providers
                        .into_iter()
                        .map(|entry| PluginOption::of(entry, suggested.contains(&entry.id())))
                        .collect()
                } else {
                    Vec::new()
                };
                providers.sort_by_cached_key(|option| {
                    (!option.recommended, option.label.to_lowercase())
                });
                MetricRow {
                    metric: MetricOption::of(metric),
                    item: calculator.and_then(|calculator| {
                        list.items
                            .iter()
                            .find(|item| item.plugin.id == calculator.entry.id())
                            .cloned()
                    }),
                    providers,
                    added,
                    unused: added && needed_by.is_empty() && !tables.iter().any(|shown| shown.on),
                    needed_by,
                    column,
                    tables,
                }
            })
            .collect()
    }

    /// The metrics that can be added: those offering a column that are not in `rows`, by label.
    pub(super) fn addable_metrics(rows: &[MetricRow]) -> Vec<MetricOption> {
        let mut addable: Vec<MetricOption> = MetricEntry::all()
            .into_iter()
            .filter(|metric| {
                metric.column().is_some() && !rows.iter().any(|row| row.metric.id == metric.id())
            })
            .map(MetricOption::of)
            .collect();
        addable.sort_by_cached_key(|option| option.label.to_lowercase());
        addable
    }
}

/// What a table showing a metric by rule for `why` says why.
fn because(why: Rule) -> Option<Because> {
    Some(match why {
        Rule::RankedBy(criterion) => Because::RankedBy(PluginOption::of(
            AnyAxis::of::<dyn SliceRanker>().entry(criterion)?,
            false,
        )),
        Rule::CalculatedFrom(metric) => {
            Because::CalculatedFrom(MetricOption::of(MetricEntry::with_id(metric)?))
        }
    })
}
