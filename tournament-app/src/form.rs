//! The plugins of a phase being configured on a screen, as a form generated from their schema.
//!
//! A [`PhaseForm`] holds a [`Draft`] of a phase and hands out a [`FormView`] of it: every field of
//! the phase and of the plugins chosen, by kind and with its answer, the plugins each field can
//! take, the plugins the answers imply, and the issues of the answers at the fields where they are
//! fixed. A screen lays it out and hands what the operator does back to the form, by the
//! [`Path`] of the field it was done to; once done, it hands [`PhaseForm::draft`] to
//! [`Session::add_phase`](crate::session::Session::add_phase) or
//! [`Session::edit_phase`](crate::session::Session::edit_phase).
//!
//! New forms follow suggestions for their ranking, metrics and comparison across groups until one
//! is edited. Then the whole section stays as chosen, until [`PhaseForm::apply_suggestions`]
//! replaces it and resumes following. Existing phases start with their settings kept.
//!
//! The metrics are a row per metric: those the operator added ([`PhaseForm::add_metric`]) and
//! those something needs, which come and go with it, see [`metrics`]. Where they are shown is said
//! apart from that ([`PhaseForm::set_shown`]), and kept beside the draft, see
//! [`PhaseForm::columns`].

pub(crate) use entry::{Context, Groups};
pub use entry::{End, EntryView, SourceChoice, SourceRowView};
pub use plugin_factory::config::Path;
use plugin_factory::{
    PluginType,
    catalog::{AnyAxis, PluginEntry},
    config::{Config, ConfigError, PluginConfig, Segment, Value},
    draft::{Draft, FieldNode, Issue, Problem},
    schema::{Configurable, DecimalRange, IntegerRange, Level, NumberRange},
};
use tournament_engine::{
    Columns, PhasePlugins, SourceOf, Table,
    entities::columns::{self, Rule},
    plugins::{
        metrics_calculator::{MetricEntry, MetricsCalculator},
        phase_format::PhaseFormat,
        seeding::SeededInOrder,
        teams::Keep,
    },
};

use crate::{
    i18n::{Arg, Language, Text},
    rejected::Rejections,
};

mod edit;
mod entry;
pub mod metrics;
pub(crate) mod template;

use edit::{Answering, Generator};
pub use metrics::{MetricOption, MetricRow};
pub use template::TemplateForm;

/// The plugins of a phase being configured, see the [module](self), with its name and where it
/// takes its entrants from, see [`Self::entry_view`].
#[derive(Clone)]
pub struct PhaseForm {
    draft: Draft<PhasePlugins>,
    following_suggestions: bool,
    /// What was entered into a number field and not taken, with why, by the path of the field.
    /// Dropped when its section is replaced or edited structurally, which may move the fields.
    rejected: Rejections<Path>,
    entry: entry::Entry,
    context: entry::Context,
    /// What the operator said about the columns of the tables of standings, only for metrics
    /// calculated.
    columns: Columns,
    /// The metrics the operator added, by id, see [`Self::add_metric`]; the others are calculated
    /// since something needs them.
    added: Vec<&'static str>,
    /// What the form removed of the metrics, to bring it back as it was.
    memory: metrics::Memory,
    /// The language numbers are typed in, see [`Self::in_language`].
    language: Language,
    /// What the hidden fields are compared with where it answers them, see
    /// [`Self::compared_with`].
    baseline: Option<Config>,
}

/// What the basic view of a phase shows besides the fields of the basic level, see
/// [`PhaseForm::basic_fields`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BasicFields {
    /// Whether it shows the groups.
    pub divisions: bool,
    /// Whether it shows where the entrants come from.
    pub sources: bool,
}

/// What a [`PhaseForm`] shows: plain data for a screen to lay out.
#[derive(Debug, Clone, PartialEq)]
pub struct FormView {
    /// The fields of the phase, in schema order.
    pub fields: Vec<FormField>,
    /// Whether changes to the basic settings refresh the whole ranking and scoring section.
    pub following_suggestions: bool,
    /// What is wrong with the answers that no field in particular fixes.
    pub issues: Vec<Text>,
    /// Whether nothing is wrong with the answers, so that a phase can be played with them, and
    /// every number entered was taken, its sources included.
    pub complete: bool,
    /// Its name and where it takes its entrants from.
    pub entry: EntryView,
    /// The plugins chosen that are an earlier version, wherever in the form, each once, in the
    /// order of the fields; [`PhaseForm::update_plugins`] replaces them by their latest versions.
    pub outdated: Vec<PluginOption>,
    /// The level of the fields shown, see [`PhaseForm::view_at`].
    pub level: Level,
    /// The fields the level hides that matter and are answered otherwise than the phase of the
    /// template the phase was added as, else otherwise than they offer.
    pub hidden: Hidden,
}

/// The fields a view hides that differ from what is expected of them, see [`FormView::hidden`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hidden {
    /// How many: a field hidden counts once, whatever the plugins answering it hold.
    pub count: usize,
    /// The lowest level showing every one of them; [`Level::Basic`] where there are none.
    pub shown_at: Level,
    /// Whether they differ from the phase of the template the phase was added as, else from what
    /// they offer, see [`PhaseForm::compared_with`].
    pub of_template: bool,
}

impl Default for Hidden {
    fn default() -> Self {
        Self {
            count: 0,
            shown_at: Level::Basic,
            of_template: false,
        }
    }
}

impl Hidden {
    /// One field of `level`, if it `differs`, else none.
    fn of(differs: bool, level: Level) -> Self {
        if differs {
            Self {
                count: 1,
                shown_at: level,
                of_template: false,
            }
        } else {
            Self::default()
        }
    }

    /// These fields and `other`.
    fn and(self, other: Self) -> Self {
        Self {
            count: self.count + other.count,
            shown_at: self.shown_at.max(other.shown_at),
            of_template: self.of_template || other.of_template,
        }
    }
}

impl FormView {
    /// The fields of the plugin `id`, as answered, if it is chosen for a field or in a list,
    /// wherever in the form; an implied plugin has none. Where several fields chose it, those
    /// of the first in schema order.
    ///
    /// ```
    /// use plugin_factory::PluginType;
    /// use tournament_app::session::Session;
    /// use tournament_engine::plugins::metrics_calculator::PointsCalculator;
    ///
    /// let view = Session::new("cup").phase_form().unwrap().view();
    /// let points = view.fields_of(PointsCalculator::ID).unwrap();
    /// assert_eq!(points[0].name, "points_for_win");
    /// ```
    pub fn fields_of(&self, id: &str) -> Option<&[FormField]> {
        fields_of(&self.fields, id)
    }

    /// The fields a summary of the phase shows: those the form shows, but neither the teams where
    /// everyone plays as they come in, nor the divisions where it is played in one, so that it
    /// reads as a phase without teams and divisions, nor the seeding where it is in order.
    pub fn summary(&self) -> Vec<FormField> {
        let chosen = |field: &FormField, id: &str| {
            matches!(
                &field.input,
                Input::Plugin(PluginInput { chosen: Some(chosen), .. }) if chosen.plugin.id == id
            )
        };
        self.fields
            .iter()
            .filter(|field| match field.name {
                PhasePlugins::TEAMS => !chosen(field, <Keep as PluginType>::ID),
                PhasePlugins::DIVISIONS => {
                    !matches!(field.input, Input::Integer { value: Some(1), .. })
                }
                PhasePlugins::SEEDING => !chosen(field, <SeededInOrder as PluginType>::ID),
                _ => true,
            })
            .cloned()
            .collect()
    }
}

fn fields_of<'a>(fields: &'a [FormField], id: &str) -> Option<&'a [FormField]> {
    fields.iter().find_map(|field| match &field.input {
        Input::Plugin(PluginInput {
            chosen: Some(chosen),
            ..
        }) if chosen.plugin.id == id => Some(&*chosen.fields),
        Input::Plugin(PluginInput {
            chosen: Some(chosen),
            ..
        }) => fields_of(&chosen.fields, id),
        Input::List(list) => list.items.iter().find_map(|item| {
            if item.plugin.id == id {
                Some(&*item.fields)
            } else {
                fields_of(&item.fields, id)
            }
        }),
        _ => None,
    })
}

/// A field of the phase or of a plugin chosen.
#[derive(Debug, Clone, PartialEq)]
pub struct FormField {
    /// Where the field is, which the changes of [`PhaseForm`] take.
    pub path: Path,
    /// The id of the plugin whose field it is, or `None` for a field of the phase itself. Its
    /// texts are translated by it, see
    /// [`Translator::field_label`](crate::i18n::Translator::field_label).
    pub plugin: Option<&'static str>,
    pub name: &'static str,
    /// The label in English.
    pub label: &'static str,
    /// The description in English.
    pub description: Option<&'static str>,
    pub input: Input,
    /// What is wrong with the answer, to be fixed here.
    pub issues: Vec<Text>,
    /// What the answer leads to that the operator may not want, although the phase can be played
    /// with it.
    pub warnings: Vec<Text>,
}

/// How a field is answered, with its answer.
#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    /// Yes or no, `None` while unanswered.
    Bool(Option<bool>),
    /// A whole number of `range`.
    Integer {
        range: IntegerRange,
        value: Option<i64>,
        /// What was entered last and not taken, which the field shows instead of `value`.
        entered: Option<String>,
        /// The numbers it takes, e.g. "a whole number of at least 1".
        hint: Text,
    },
    /// A number of `range`.
    Decimal {
        range: DecimalRange,
        value: Option<f64>,
        /// What was entered last and not taken, which the field shows instead of `value`.
        entered: Option<String>,
        /// The numbers it takes, e.g. "a number of at least 0".
        hint: Text,
    },
    /// An answer of `inner` that may be left out, see [`Editable::answer_optional`].
    Optional { answered: bool, inner: Box<Input> },
    /// One plugin.
    Plugin(PluginInput),
    /// A list of plugins.
    List(ListInput),
}

/// A field taking one plugin.
#[derive(Debug, Clone, PartialEq)]
pub struct PluginInput {
    /// Whether it may take no plugin.
    pub optional: bool,
    /// The plugin chosen, with its fields; `None` while there is none.
    pub chosen: Option<Chosen>,
    /// The plugins it can take, the chosen one included: the plugins recommended first, in the
    /// order they are recommended in, then the others by label.
    pub options: Vec<PluginOption>,
}

/// A plugin chosen, with its fields.
#[derive(Debug, Clone, PartialEq)]
pub struct Chosen {
    pub plugin: PluginOption,
    pub fields: Vec<FormField>,
}

/// A field taking a list of plugins.
#[derive(Debug, Clone, PartialEq)]
pub struct ListInput {
    /// Whether the order of the plugins matters, e.g. that of the criteria the phase is ranked by;
    /// that of a set of metrics does not.
    pub ordered: bool,
    /// The plugins chosen, in order.
    pub items: Vec<Item>,
    /// The plugins added after them since the others need them, see [`Draft::add_implied`]. They
    /// have nothing to configure, and go when nothing needs them any more.
    pub implied: Vec<Implied>,
    /// The plugins that can be added, ordered as [`PluginInput::options`].
    pub options: Vec<PluginOption>,
    /// For the list of metrics of the phase, a row per metric, added or needed, by label; empty
    /// for every other list. The calculators of [`Self::items`] are found there too.
    pub metrics: Vec<MetricRow>,
    /// For the list of metrics of the phase, the metrics that can be added, by label, see
    /// [`PhaseForm::add_metric`]; empty for every other list.
    pub addable: Vec<MetricOption>,
}

impl ListInput {
    /// Every plugin of the list, chosen, implied or offered, in an order that neither choosing one
    /// or leaving it out nor another recommendation, e.g. of another format, changes, for a list
    /// whose order does not matter: by label.
    pub fn set(&self) -> Vec<SetEntry<'_>> {
        let mut entries: Vec<_> = self
            .items
            .iter()
            .map(SetEntry::Chosen)
            .chain(self.implied.iter().map(SetEntry::Implied))
            .chain(self.options.iter().map(SetEntry::Offered))
            .collect();
        entries.sort_by_cached_key(|entry| entry.plugin().label.to_lowercase());
        entries
    }
}

fn table_index(table: Table) -> usize {
    match table {
        Table::Division => 0,
        Table::Across => 1,
    }
}

/// Whether a table shows a metric, see [`MetricRow::tables`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShownIn {
    pub on: bool,
    pub by: By,
}

/// Who decided whether a table shows a metric.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum By {
    /// The rule, see [`columns::by_rule`]: why it is shown, `None` if it is not.
    Rule(Option<Because>),
    /// The operator, against the rule, see [`PhaseForm::set_shown`].
    Operator,
}

/// Why a table shows a metric by rule.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Because {
    /// The table is ranked by it, by this criterion.
    RankedBy(PluginOption),
    /// This metric, shown by rule, is calculated from it.
    CalculatedFrom(MetricOption),
}

/// A plugin added to a list since others need it, see [`ListInput::implied`].
#[derive(Debug, Clone, PartialEq)]
pub struct Implied {
    pub plugin: PluginOption,
    /// The plugins of the phase that need it, e.g. the criterion ranking by the metric it
    /// calculates, in the order of the fields of the phase.
    pub needed_by: Vec<PluginOption>,
}

/// A plugin of a list, see [`ListInput::set`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SetEntry<'a> {
    /// One of [`ListInput::items`].
    Chosen(&'a Item),
    /// One of [`ListInput::implied`].
    Implied(&'a Implied),
    /// One of [`ListInput::options`].
    Offered(&'a PluginOption),
}

impl SetEntry<'_> {
    pub fn plugin(&self) -> &PluginOption {
        match self {
            Self::Chosen(item) => &item.plugin,
            Self::Implied(implied) => &implied.plugin,
            Self::Offered(plugin) => plugin,
        }
    }
}

/// A plugin chosen in a list.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    /// Where it is, which [`Editable::remove`] and the moves take.
    pub path: Path,
    pub plugin: PluginOption,
    pub fields: Vec<FormField>,
    /// What is wrong with it, to be fixed by removing or moving it.
    pub issues: Vec<Text>,
}

/// A plugin as a choice of a field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PluginOption {
    /// Its id, which the changes of [`PhaseForm`] take, and which its texts are translated by, see
    /// [`PluginText`](crate::i18n::PluginText).
    pub id: &'static str,
    /// The label in English.
    pub label: &'static str,
    /// What is said about it in English, translated as its
    /// [`PluginText::Description`](crate::i18n::PluginText::Description).
    pub description: Option<&'static str>,
    /// Whether the configured format suggests it here.
    pub recommended: bool,
    /// Whether it is an earlier version of a plugin, which a phase added with it keeps until its
    /// plugins are updated, see [`PhaseForm::update_plugins`]; never offered otherwise.
    pub outdated: bool,
}

impl PluginOption {
    fn of(entry: &'static PluginEntry, recommended: bool) -> Self {
        Self {
            id: entry.id(),
            label: entry.label(),
            description: entry.description(),
            recommended,
            outdated: entry.successor().is_some(),
        }
    }
}

/// The edits of the fields of a form, by the [`Path`] of the field, as a [`PhaseForm`] takes them.
pub trait Editable {
    /// Answers the field at `path`, which takes yes or no, with `answer`.
    ///
    /// # Errors
    /// Returns an error if there is no such field, or it takes something else.
    fn set_bool(&mut self, path: &Path, answer: bool) -> Result<(), ConfigError>;

    /// Answers the field at `path`, which takes a number, with the number `entered`, as an operator
    /// typed it: with a point or a comma before its decimals, possibly with an exponent (`1e3`).
    /// Something else, nothing, or a number the field does not take, leaves the answer as it was,
    /// and the field shows what was entered with why it was not taken until the next change other
    /// than entering a number; meanwhile the form is not complete (e.g.
    /// [`FormView::complete`]).
    ///
    /// # Errors
    /// Returns an error if there is no such field, or it takes no number.
    fn enter(&mut self, path: &Path, entered: &str) -> Result<(), ConfigError>;

    /// Why the field at `path`, which takes a number, would not take `entered`, as
    /// [`Self::enter`] would show it, e.g. while it is being typed; `None` if it would take it.
    ///
    /// # Errors
    /// Returns an error if there is no such field, or it takes no number.
    fn issue_entering(&self, path: &Path, entered: &str) -> Result<Option<Text>, ConfigError>;

    /// Answers the field at `path`, which may be left without an answer, if `answered`, or else
    /// leaves it without one. An answer it is given is what it offers, or else no, or the number of
    /// its range closest to 0.
    ///
    /// # Errors
    /// Returns an error if there is no such field, or it takes an answer that cannot be left out.
    fn answer_optional(&mut self, path: &Path, answered: bool) -> Result<(), ConfigError>;

    /// Answers the field at `path`, which takes one plugin, with the plugin `plugin`, configured as
    /// it offers, or with none for `None`.
    ///
    /// # Errors
    /// Returns an error if there is no such field, it takes no plugin, or no plugin of this id;
    /// for `None`, if it has to take one.
    fn choose(&mut self, path: &Path, plugin: Option<&str>) -> Result<(), ConfigError>;

    /// Adds the plugin `plugin` to the end of the list at `path`, configured as it offers.
    ///
    /// # Errors
    /// Returns an error if there is no such field, it takes no list, or no plugin of this id, or
    /// has it already.
    fn add(&mut self, path: &Path, plugin: &str) -> Result<(), ConfigError>;

    /// Removes the plugin at `item`, a position in a list, from it.
    ///
    /// # Errors
    /// Returns an error if there is no such position.
    fn remove(&mut self, item: &Path) -> Result<(), ConfigError>;

    /// Moves the plugin at `item`, a position in a list, one up, before the plugin before it;
    /// the first stays where it is.
    ///
    /// # Errors
    /// Returns an error if there is no such position.
    fn move_up(&mut self, item: &Path) -> Result<(), ConfigError>;

    /// Moves the plugin at `item`, a position in a list, one down, after the plugin after it; the
    /// last stays where it is.
    ///
    /// # Errors
    /// Returns an error if there is no such position.
    fn move_down(&mut self, item: &Path) -> Result<(), ConfigError>;
}

// a form answering a draft is edited by the functions of `edit`
impl<F: Answering> Editable for F {
    fn set_bool(&mut self, path: &Path, answer: bool) -> Result<(), ConfigError> {
        edit::set_bool(self, path, answer)
    }

    fn enter(&mut self, path: &Path, entered: &str) -> Result<(), ConfigError> {
        edit::enter(self, path, entered)
    }

    fn issue_entering(&self, path: &Path, entered: &str) -> Result<Option<Text>, ConfigError> {
        edit::issue_entering(self, path, entered)
    }

    fn answer_optional(&mut self, path: &Path, answered: bool) -> Result<(), ConfigError> {
        edit::answer_optional(self, path, answered)
    }

    fn choose(&mut self, path: &Path, plugin: Option<&str>) -> Result<(), ConfigError> {
        edit::choose(self, path, plugin)
    }

    fn add(&mut self, path: &Path, plugin: &str) -> Result<(), ConfigError> {
        edit::add(self, path, plugin)
    }

    fn remove(&mut self, item: &Path) -> Result<(), ConfigError> {
        edit::remove(self, item)
    }

    fn move_up(&mut self, item: &Path) -> Result<(), ConfigError> {
        edit::move_up(self, item)
    }

    fn move_down(&mut self, item: &Path) -> Result<(), ConfigError> {
        edit::move_down(self, item)
    }
}

impl PhaseForm {
    /// Whether every number field took what was typed into it, so that the draft holds what the
    /// form shows.
    pub(crate) fn all_taken(&self) -> bool {
        self.rejected.is_empty()
    }
}

impl Answering for PhaseForm {
    type Of = PhasePlugins;

    fn language(&self) -> Language {
        self.language
    }

    fn draft(&self) -> &Draft<PhasePlugins> {
        &self.draft
    }

    fn rejected(&self) -> &Rejections<Path> {
        &self.rejected
    }

    fn rejected_mut(&mut self) -> &mut Rejections<Path> {
        &mut self.rejected
    }

    fn change(
        &mut self,
        path: &Path,
        change: impl FnOnce(&mut Draft<PhasePlugins>) -> Result<(), ConfigError>,
    ) -> Result<(), ConfigError> {
        let mut draft = self.draft.clone();
        change(&mut draft)?;
        let following = self.following_suggestions && !is_suggested(path);
        let draft = if following {
            suggested(draft)?
        } else {
            draft.with_defaults()?
        };
        // following suggestions, the operator has added nothing; else a calculator added or
        // removed by hand adds or removes its metric
        let added = if following {
            metrics::added_in(&draft)
        } else {
            metrics::added_by_hand(&self.draft, &draft, &self.added)
        };
        let earlier = std::mem::replace(&mut self.added, added);
        self.draft = match self.settle(draft) {
            Ok(draft) => draft,
            Err(error) => {
                self.added = earlier;
                return Err(error);
            }
        };
        // a metric that leaves the list takes what was said about its columns with it
        let calculated = metrics_of(&answered(&self.draft, PhasePlugins::METRICS));
        self.columns.retain(|id| calculated.contains(&id));
        self.following_suggestions = following;
        if is_suggested(path) || following {
            self.rejected.clear();
        } else {
            // Basic settings do not move customized ranking or metric fields.
            self.rejected.retain(is_suggested);
        }
        Ok(())
    }

    fn not_taken(&mut self, path: &Path) {
        if is_suggested(path) {
            self.following_suggestions = false;
        }
    }
}

impl PhaseForm {
    /// A form preserving the settings of `draft`, of the first phase of a competition, without a
    /// name. [`Self::apply_suggestions`] starts following suggestions;
    /// [`Session::phase_form`](crate::session::Session::phase_form) does so for a new phase, and
    /// knows the phases of its competition.
    pub fn new(draft: Draft<PhasePlugins>) -> Self {
        Self {
            added: metrics::added_in(&draft),
            memory: metrics::Memory::default(),
            draft,
            following_suggestions: false,
            rejected: Rejections::default(),
            entry: entry::Entry::default(),
            context: entry::Context::default(),
            columns: Columns::new(),
            language: Language::default(),
            baseline: None,
        }
    }

    /// This form telling the hidden fields that differ from `answers`, the answers of the phase
    /// of a template it was added as, rather than those differing from what they offer, where
    /// `answers` answer them, see [`FormView::hidden`].
    #[must_use]
    pub fn compared_with(mut self, answers: Config) -> Self {
        self.baseline = Some(answers);
        self
    }

    /// This form with the numbers typed into it read as `language` writes them, see
    /// [`Editable::enter`]; English if not set.
    #[must_use]
    pub fn in_language(mut self, language: Language) -> Self {
        self.language = language;
        self
    }

    /// This form of a phase of the competition `context` describes, named `name`, taking its
    /// entrants from `sources` and showing what `columns` say.
    pub(crate) fn of_phase(
        mut self,
        context: Context,
        name: Option<&str>,
        sources: Vec<tournament_engine::Source>,
        columns: Columns,
    ) -> Self {
        self.context = context;
        self.columns = columns;
        self.entry = entry::Entry {
            name: name.unwrap_or_default().to_owned(),
            sources,
            rejected: Rejections::default(),
            some_places: false,
        };
        self.redraft();
        self
    }

    /// The draft as the form answered it.
    pub fn draft(&self) -> &Draft<PhasePlugins> {
        &self.draft
    }

    /// Whether the form says anything other than `earlier`, an earlier clone of it, said: other
    /// answers, name, sources or columns, or text typed that a field did not take. A change
    /// undone by hand is no change, as far as the answers are the same again.
    pub fn changed_since(&self, earlier: &Self) -> bool {
        self.draft.answers() != earlier.draft.answers()
            || self.columns != earlier.columns
            || self.entry.name != earlier.entry.name
            || self.entry.sources != earlier.entry.sources
            || self.entry.some_places != earlier.entry.some_places
            || !self.rejected.is_empty()
            || !self.entry.rejected.is_empty()
    }

    /// What the operator said about the columns of the tables of standings, see
    /// [`Self::set_shown`].
    pub fn columns(&self) -> &Columns {
        &self.columns
    }

    /// Shows what `columns` say instead, as far as they are about metrics calculated.
    pub fn set_columns(&mut self, mut columns: Columns) {
        let calculated = metrics_of(&answered(&self.draft, PhasePlugins::METRICS));
        columns.retain(|id| calculated.contains(&id));
        self.columns = columns;
    }

    /// Replaces the ranking, metrics and comparison across groups, including their settings, with
    /// the suggestions for the other phase fields, and resumes following suggestions. Those fields
    /// and their rejected numeric input are kept.
    ///
    /// # Errors
    /// Returns an error if a suggested answer does not fit its schema. Nothing changes then.
    pub fn apply_suggestions(&mut self) -> Result<(), ConfigError> {
        let draft = suggested(self.draft.clone())?;
        self.added = metrics::added_in(&draft);
        self.draft = self.settle(draft)?;
        self.following_suggestions = true;
        self.rejected.retain(|path| !is_suggested(path));
        self.columns = Columns::new();
        Ok(())
    }

    /// Shows the metric `metric` in `table` if `shown`, else hides it there. It neither adds nor
    /// removes the metric, see [`Self::add_metric`]. The operator's choice is kept only where it
    /// differs from the rule for the draft as it is, see [`columns::by_rule`].
    pub fn set_shown(&mut self, metric: &str, table: Table, shown: bool) {
        let by_rule = self
            .rule(&self.draft, table)
            .iter()
            .any(|&(ruled, _)| ruled == metric);
        self.columns
            .set(metric, table, (shown != by_rule).then_some(shown));
    }

    /// The plugin a plugin of the issue at `required_by` is there for, if it is the calculator of
    /// a metric only needed: what needs that metric, however far back, up to a plugin that is no
    /// such calculator.
    fn root_needer(&self, required_by: &Path) -> Option<&'static PluginEntry> {
        let calculator = self.draft.plugin_at(required_by)?.entry;
        let mut metric = MetricEntry::of_calculator(calculator)?;
        let mut needer = None;
        let mut seen = Vec::new();
        while !self.added.contains(&metric.id()) && !seen.contains(&metric) {
            seen.push(metric);
            let plugins = self
                .draft
                .fields()
                .into_iter()
                .flat_map(|node| node.plugins());
            let next = plugins
                .map(|plugin| plugin.entry)
                .find(|plugin| plugin.id() != calculator.id() && metric.is_required_by(plugin))?;
            needer = Some(next);
            match MetricEntry::of_calculator(next) {
                Some(next) => metric = next,
                None => break,
            }
        }
        needer
    }

    /// What the fields of the draft up to `level`, and those of `anyway`, are shown from, those
    /// of `elsewhere` left out.
    fn generator<'a>(
        &'a self,
        level: Level,
        anyway: &'a [Path],
        elsewhere: &'a [Path],
    ) -> Generator<'a, PhasePlugins> {
        Generator {
            draft: &self.draft,
            rejected: &self.rejected,
            offered: PhasePlugins::default_value,
            level,
            anyway,
            elsewhere,
            baseline: self.baseline.as_ref(),
        }
    }

    /// What the basic view of the phase shows besides the fields of the basic level, as it would
    /// open now: the groups where there are several or its format says they are basic (see
    /// [`PhaseFormat::has_basic_divisions`]), and where its entrants come from unless it takes
    /// every entrant registered. A screen decides it once when it opens, so that a field does not
    /// go while it is being edited, e.g. the groups set to one.
    pub fn basic_fields(&self) -> BasicFields {
        let answers = self.draft.answers();
        let several = answers
            .get(PhasePlugins::DIVISIONS)
            .and_then(Value::as_u64)
            .is_some_and(|divisions| divisions > 1);
        let basic = answers
            .get(PhasePlugins::FORMAT)
            .and_then(PluginConfig::from_answer)
            .and_then(|config| config.build::<dyn PhaseFormat>().ok())
            .is_some_and(|format| format.has_basic_divisions());
        BasicFields {
            divisions: several || basic,
            sources: !self.entry_view().everyone_registered,
        }
    }

    /// What the form shows in the basic view laid out by `basic`: the fields of the basic level,
    /// and the groups if `basic` shows them. Where the entrants come from is
    /// [`FormView::entry`], which a screen shows if `basic` says so. The match format, and the
    /// teams of a phase taking places of the registration, are neither shown nor counted as
    /// hidden: the basic view shows them for every phase at once.
    pub fn view_basic(&self, basic: BasicFields) -> FormView {
        let divisions = [Path::ROOT.field(PhasePlugins::DIVISIONS)];
        let anyway: &[Path] = if basic.divisions { &divisions } else { &[] };
        let mut elsewhere = vec![Path::ROOT.field(PhasePlugins::MATCH_FORMAT)];
        if self
            .sources()
            .iter()
            .any(|source| source.of() == SourceOf::Registration)
        {
            elsewhere.push(Path::ROOT.field(PhasePlugins::TEAMS));
        }
        self.view_showing(Level::Basic, anyway, &elsewhere)
    }

    /// The field of the match format, with the fields of its plugin up to `level`, as the view
    /// at `level` would show it if it showed the field, with what is wrong with it.
    pub fn match_format_field(&self, level: Level) -> Option<FormField> {
        let path = Path::ROOT.field(PhasePlugins::MATCH_FORMAT);
        self.view_showing(level, std::slice::from_ref(&path), &[])
            .fields
            .into_iter()
            .find(|field| field.path == path)
    }

    /// The draft with the plugins it implies, see [`Draft::add_implied`].
    fn completed(&self) -> Draft<PhasePlugins> {
        let mut completed = self.draft.clone();
        completed.add_implied();
        completed
    }

    /// What `table` shows by rule for `completed`, a draft with the plugins it implies.
    fn rule(&self, completed: &Draft<PhasePlugins>, table: Table) -> Vec<(&'static str, Rule)> {
        let criteria = match table {
            Table::Division => PhasePlugins::RANKING,
            Table::Across => PhasePlugins::ACROSS,
        };
        columns::by_rule(
            &answered(completed, criteria),
            &answered(completed, PhasePlugins::METRICS),
        )
    }

    /// What the form shows now, every field, as [`Self::view_at`] shows it at [`Level::Expert`].
    pub fn view(&self) -> FormView {
        self.view_at(Level::Expert)
    }

    /// What the form shows now to an operator of `level`: the fields up to it, of the plugins
    /// chosen too. A field of a higher level is hidden, with every field of the plugins answering
    /// it, and stays as it is answered; what is wrong with it is said with the form as a whole.
    /// Only what is shown depends on the level, not what the form does, e.g. whether it follows
    /// suggestions.
    pub fn view_at(&self, level: Level) -> FormView {
        if level == Level::Basic {
            return self.view_basic(self.basic_fields());
        }
        self.view_showing(level, &[], &[])
    }

    /// What the form shows to an operator of `level`, and the fields of `anyway` whatever their
    /// level, without those of `elsewhere`.
    fn view_showing(&self, level: Level, anyway: &[Path], elsewhere: &[Path]) -> FormView {
        let completed = self.completed();
        let issues = completed.build().err().unwrap_or_default();
        let nodes = self.draft.fields();
        let completed_nodes = completed.fields();
        let mut irrelevant = self.draft.irrelevant();
        // comparing across divisions only matters with several, and is shown while it compares by
        // something, so that it can be emptied
        let answers = self.draft.answers();
        let one_division = answers
            .get(PhasePlugins::DIVISIONS)
            .is_none_or(|divisions| divisions.as_u64() == Some(1));
        let compares_by_nothing = answers
            .get(PhasePlugins::ACROSS)
            .is_none_or(|across| across.as_array().is_none_or(Vec::is_empty));
        if one_division && compares_by_nothing {
            irrelevant.push(Path::ROOT.field(PhasePlugins::ACROSS));
        }
        // the draw only matters with several divisions, the seeding to a format reading it
        if one_division {
            irrelevant.push(Path::ROOT.field(PhasePlugins::DRAW));
        }
        let reads_no_seeding = answers
            .get(PhasePlugins::FORMAT)
            .and_then(PluginConfig::from_answer)
            .and_then(|config| config.build::<dyn PhaseFormat>().ok())
            .is_some_and(|format| !format.reads_seeding());
        if reads_no_seeding {
            irrelevant.push(Path::ROOT.field(PhasePlugins::SEEDING));
        }
        let whole = Whole {
            completed: &completed_nodes,
            irrelevant: &irrelevant,
        };
        let generator = self.generator(level, anyway, elsewhere);
        let mut fields = generator.fields(&nodes, None, &completed_nodes, &whole);
        let hidden = Hidden {
            of_template: self.baseline.is_some(),
            ..generator.hidden(&nodes, &whole)
        };
        let mut unplaced = Vec::new();
        for issue in &issues {
            let needer = match issue.problem() {
                Problem::Unprovided { required_by, .. } => self.root_needer(required_by),
                _ => None,
            };
            let text = issue_text(issue, &completed, needer);
            if !place(&mut fields, issue.path(), &text) {
                unplaced.push(text);
            }
        }
        for (path, rejected) in self.rejected.iter() {
            place(&mut fields, path, &rejected.problem);
        }
        // after the issues, which the rows of the metrics take along with the fields of their
        // calculators
        if let Some(FormField {
            input: Input::List(metrics),
            ..
        }) = fields
            .iter_mut()
            .find(|field| field.path == Path::ROOT.field(PhasePlugins::METRICS))
        {
            metrics.metrics = self.metric_rows(metrics);
            metrics.addable = Self::addable_metrics(&metrics.metrics);
        }
        // a phase ranked by nothing is played all the same, with every entrant tied
        if let Some(ranking) = fields
            .iter_mut()
            .find(|field| field.name == PhasePlugins::RANKING)
            && let Input::List(list) = &ranking.input
            && list.items.is_empty()
            && list.implied.is_empty()
        {
            ranking.warnings.push(Text::new("warning-unranked"));
        }
        FormView {
            fields,
            following_suggestions: self.following_suggestions,
            complete: issues.is_empty()
                && self.rejected.is_empty()
                && self.sources_issue().is_none(),
            issues: unplaced,
            entry: self.entry_view(),
            outdated: self.outdated(),
            level,
            hidden,
        }
    }

    /// See [`FormView::outdated`].
    fn outdated(&self) -> Vec<PluginOption> {
        let mut outdated: Vec<_> = Vec::new();
        for plugin in self.draft.outdated() {
            let option = PluginOption::of(plugin.plugin, false);
            if !outdated.contains(&option) {
                outdated.push(option);
            }
        }
        outdated
    }

    /// Replaces every plugin chosen that is an earlier version by its latest version, keeping the
    /// answers that version takes, see [`Draft::update`]; whether the phase can be played with it
    /// once results are reported is found out on [`Session::edit_phase`].
    ///
    /// # Errors
    /// Returns an error if the latest version of a plugin does not take an answer the earlier one
    /// did. Nothing is changed then.
    ///
    /// [`Session::edit_phase`]: crate::session::Session::edit_phase
    pub fn update_plugins(&mut self) -> Result<(), ConfigError> {
        self.change(&Path::ROOT, Draft::update)
    }
}

/// The id of the metric the calculator of the id `calculator` calculates, if both are registered.
fn metric_of(calculator: &str) -> Option<&'static str> {
    let entry = AnyAxis::of::<dyn MetricsCalculator>().entry(calculator)?;
    Some(MetricEntry::of_calculator(entry)?.id())
}

/// The ids of the metrics the calculators of the ids `calculators` calculate, in order, as far as
/// both are registered.
fn metrics_of(calculators: &[&str]) -> Vec<&'static str> {
    calculators.iter().filter_map(|id| metric_of(id)).collect()
}

/// The ids of the plugins answering the field `field` of the phase in `draft`, in order.
fn answered(draft: &Draft<PhasePlugins>, field: &str) -> Vec<&'static str> {
    let path = Path::ROOT.field(field);
    draft
        .fields()
        .into_iter()
        .find(|node| node.path == path)
        .map(|node| {
            node.plugins()
                .into_iter()
                .map(|plugin| plugin.entry.id())
                .collect()
        })
        .unwrap_or_default()
}

fn is_suggested(path: &Path) -> bool {
    let Some(Segment::Field(name)) = path.segments().first() else {
        return false;
    };
    PhasePlugins::SUGGESTED.contains(&name.as_str())
}

/// What the whole form shows its fields with, found once per [`PhaseForm::view`].
struct Whole<'a> {
    /// The fields of the phase with the implied plugins appended.
    completed: &'a [FieldNode<'a>],
    /// The fields that do not matter, see [`Draft::irrelevant`].
    irrelevant: &'a [Path],
}

/// The plugins answering the fields of the phase in `completed` that need `entry` of `axis`, in
/// the order of the fields, by their static declarations, see [`PluginEntry::providers_in`].
fn needed_by(entry: &PluginEntry, axis: AnyAxis, completed: &[FieldNode]) -> Vec<PluginOption> {
    completed
        .iter()
        .flat_map(FieldNode::plugins)
        .map(|plugin| plugin.entry)
        .filter(|plugin| {
            plugin
                .providers_in(axis)
                .iter()
                .any(|provider| provider.id() == entry.id())
        })
        .map(|plugin| PluginOption::of(plugin, false))
        .collect()
}

fn suggested(mut draft: Draft<PhasePlugins>) -> Result<Draft<PhasePlugins>, ConfigError> {
    for field in PhasePlugins::SUGGESTED {
        draft.unset(&Path::ROOT.field(field))?;
    }
    PhasePlugins::with_suggestions(draft)
}

/// Why a number entered is not taken, see [`number`] and [`typed_number`].
pub(crate) enum NotTaken {
    /// It is a number, but none of the kind the field takes, or nothing was entered.
    OutOfRange,
    NoNumber,
}

/// The number `entered` as the answer of a field of `range`, typed in `language`, see
/// [`Editable::enter`].
pub(crate) fn number(
    entered: &str,
    range: NumberRange,
    language: Language,
) -> Result<Value, NotTaken> {
    if entered.trim().is_empty() {
        return Err(NotTaken::OutOfRange);
    }
    let typed = crate::number::read(entered, language, true).ok_or(NotTaken::NoNumber)?;
    let number = typed.get();
    if !number.is_finite() {
        return Err(NotTaken::OutOfRange);
    }
    match range {
        NumberRange::Integer(_) => {
            // exactly, where a float would round
            if let Ok(whole) = typed.plain().parse::<i64>() {
                return Ok(Value::from(whole));
            }
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_precision_loss,
                reason = "a whole number within the bounds of i64"
            )]
            let whole =
                (number.fract() == 0.0 && number >= i64::MIN as f64 && number < i64::MAX as f64)
                    .then_some(number as i64);
            whole.map(Value::from).ok_or(NotTaken::OutOfRange)
        }
        NumberRange::Decimal(_) => Ok(Value::from(number)),
    }
}

/// The number `entered` stands for, as an operator in `language` typed it, see
/// [`crate::number::read`]. A number too large for a float is out of range.
pub(crate) fn typed_number(entered: &str, language: Language) -> Result<f64, NotTaken> {
    let number = crate::number::read(entered, language, true)
        .ok_or(NotTaken::NoNumber)?
        .get();
    // a number too large for a float is a number all the same
    if !number.is_finite() {
        return Err(NotTaken::OutOfRange);
    }
    Ok(number)
}

/// Puts `text` at the deepest field or item of `fields` that `path` is within; `false` if it is
/// within none of them.
fn place(fields: &mut [FormField], path: &Path, text: &Text) -> bool {
    let Some(field) = fields
        .iter_mut()
        .find(|field| path.segments().starts_with(field.path.segments()))
    else {
        return false;
    };
    let placed = match &mut field.input {
        Input::Plugin(PluginInput {
            chosen: Some(chosen),
            ..
        }) => place(&mut chosen.fields, path, text),
        Input::List(list) => list
            .items
            .iter_mut()
            .find(|item| path.segments().starts_with(item.path.segments()))
            .is_some_and(|item| {
                place(&mut item.fields, path, text) || {
                    item.issues.push(text.clone());
                    true
                }
            }),
        _ => false,
    };
    if !placed {
        field.issues.push(text.clone());
    }
    true
}

/// What `issue` of the enumerated fields says.
/// What `issue` of `draft` says; an issue of a plugin's needing what nothing provides names
/// `needer`, if given, as what needs that plugin, see [`PhaseForm::root_needer`].
fn issue_text<T: Configurable>(
    issue: &Issue,
    draft: &Draft<T>,
    needer: Option<&'static PluginEntry>,
) -> Text {
    match issue.problem() {
        Problem::Invalid(ConfigError::Unanswered { .. }) => Text::new("issue-unanswered"),
        Problem::Invalid(ConfigError::OutOfRange { allowed, .. }) => {
            with_range(Text::new("issue-out-of-range"), *allowed)
        }
        Problem::Invalid(ConfigError::Duplicate { .. }) => Text::new("issue-duplicate"),
        Problem::Invalid(ConfigError::Rejected { reason, .. }) => {
            Text::new("issue-rejected").with("reason", reason.as_str())
        }
        Problem::Invalid(error) => Text::new("issue-invalid").with("reason", error.to_string()),
        Problem::Incompatible(incompatibility) => {
            Text::new("issue-incompatible").with("reason", incompatibility.to_string())
        }
        Problem::Unmet {
            condition,
            provided,
            provider,
            choices_at,
            fitting,
        } => {
            let plugin = |path: &Path| {
                draft.plugin_at(path).map_or_else(
                    || Arg::from(path.to_string()),
                    |plugin| plugin_arg(plugin.entry),
                )
            };
            // the context of a phase is what comes in from its sources
            let unmet =
                match (provider, provided) {
                    (Some(path), provided) => {
                        let provider = Text::new("issue-provider").with("plugin", plugin(path));
                        let provider = match field_arg(draft, path) {
                            Some(field) => provider.with("field", field),
                            None => provider,
                        };
                        match provided {
                            Some(provided) => Text::new("issue-unmet")
                                .with("provided", Arg::Fact(provided.clone())),
                            None => Text::new("issue-unmet-unsaid"),
                        }
                        .with("provider", provider)
                    }
                    (None, Some(provided)) => Text::new("issue-unmet-coming-in")
                        .with("provided", Arg::Fact(provided.clone())),
                    (None, None) => Text::new("issue-unmet-context"),
                }
                .with("plugin", plugin(issue.path()))
                .with("condition", Arg::Fact(condition.clone()));
            let fitting: Vec<_> = draft
                .candidates(choices_at)
                .unwrap_or_default()
                .into_iter()
                .filter(|entry| fitting.contains(&entry.id()))
                .map(|entry| (entry.id(), entry.label()))
                .collect();
            match field_arg(draft, choices_at) {
                Some(field) if !fitting.is_empty() => Text::new("issue-fitting")
                    .with("issue", unmet)
                    .with("field", field)
                    .with("count", fitting.len() as f64)
                    .with("choices", Arg::Plugins(fitting)),
                _ => unmet,
            }
        }
        Problem::Unprovided {
            fact,
            required_by,
            providers,
            lacking,
        } => {
            let plugin = draft
                .plugin_at(required_by)
                .map(|plugin| plugin.entry)
                .map_or_else(|| Arg::from(required_by.to_string()), plugin_arg);
            // the providers are of the axis of the field the issue is at
            let axis = draft
                .field_at(issue.path())
                .and_then(|node| node.field.kind().axis());
            let providers: Vec<_> = providers
                .iter()
                .filter_map(|id| axis?.entry(id))
                .map(|entry| (entry.id(), entry.label()))
                .collect();
            if providers.is_empty() {
                Text::new("issue-unprovided")
                    .with("plugin", plugin)
                    .with("fact", Arg::Fact(fact.clone()))
            } else if let Some(lacking) = lacking {
                Text::new("issue-unprovided-lacking")
                    .with("plugin", plugin)
                    .with("count", providers.len())
                    .with("providers", Arg::Plugins(providers))
                    .with("lacking", Arg::Fact(lacking.clone()))
            } else if let Some(needer) = needer {
                Text::new("issue-unprovided-for")
                    .with("needer", plugin_arg(needer))
                    .with("plugin", plugin)
                    .with("providers", Arg::Plugins(providers))
            } else {
                Text::new("issue-unprovided-by")
                    .with("plugin", plugin)
                    .with("providers", Arg::Plugins(providers))
            }
        }
    }
}

/// `text` with the variables describing `range`: `whole` (`yes` or `no`), `bounds` (`both`,
/// `min`, `max` or `none`), `min`, `max`, and `multiple`, which is 0 for any number.
#[expect(
    clippy::cast_precision_loss,
    reason = "a bound an operator reads is far below 2^52"
)]
pub(crate) fn with_range(text: Text, range: NumberRange) -> Text {
    let (whole, min, max, multiple) = match range {
        NumberRange::Integer(range) => (
            "yes",
            range.stated_min().map(|min| min as f64),
            range.stated_max().map(|max| max as f64),
            Some(range.multiple_of() as f64).filter(|multiple| *multiple != 1.0),
        ),
        NumberRange::Decimal(range) => ("no", range.min(), range.max(), range.multiple_of()),
    };
    let bounds = match (min, max) {
        (Some(_), Some(_)) => "both",
        (Some(_), None) => "min",
        (None, Some(_)) => "max",
        (None, None) => "none",
    };
    text.with("whole", whole)
        .with("bounds", bounds)
        .with("min", min.unwrap_or_default())
        .with("max", max.unwrap_or_default())
        .with("multiple", multiple.unwrap_or_default())
}

/// The label of the field at `path`, or of the list at whose position it is, as a variable of a
/// [`Text`]; `None` if there is none.
fn field_arg<T: Configurable>(draft: &Draft<T>, path: &Path) -> Option<Arg> {
    let field = match path.segments().split_last() {
        Some((Segment::Index(_), list)) => of(list),
        _ => path.clone(),
    };
    let node = draft.field_at(&field)?;
    let plugin = match field.segments().split_last() {
        Some((_, parent)) if !parent.is_empty() => Some(draft.plugin_at(&of(parent))?.entry.id()),
        _ => None,
    };
    Some(Arg::Field {
        plugin,
        name: node.field.name(),
        english: node.field.label(),
    })
}

/// The label of the plugin `entry`, as a variable of a [`Text`].
fn plugin_arg(entry: &'static PluginEntry) -> Arg {
    Arg::Plugin {
        id: entry.id(),
        english: entry.label(),
    }
}

/// The list `item` is a position of, and the position.
fn position(item: &Path) -> Result<(Path, usize), ConfigError> {
    match item.segments().split_last() {
        Some((Segment::Index(index), list)) => Ok((of(list), *index)),
        _ => Err(ConfigError::UnknownField { path: item.clone() }),
    }
}

/// The path of `segments`.
fn of(segments: &[Segment]) -> Path {
    segments
        .iter()
        .fold(Path::ROOT, |path, segment| match segment {
            Segment::Field(name) => path.field(name.clone()),
            Segment::Index(index) => path.index(*index),
        })
}

#[cfg(test)]
mod tests;
