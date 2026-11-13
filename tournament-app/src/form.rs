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
//! New forms follow suggestions for their ranking and metrics until either is edited. Then the
//! whole section stays as chosen, until [`PhaseForm::apply_suggestions`] replaces it and resumes
//! following. Existing phases start with their settings kept.

pub use plugin_factory::config::Path;
use plugin_factory::{
    catalog::{AnyAxis, PluginEntry},
    config::{ConfigError, PluginConfig, Segment, Value},
    draft::{Draft, FieldNode, Issue, Problem},
    schema::{DecimalRange, FieldKind, IntegerRange, NumberRange},
};
use tournament_engine::PhasePlugins;

use crate::i18n::{Arg, Text};

/// The plugins of a phase being configured, see the [module](self).
#[derive(Clone)]
pub struct PhaseForm {
    draft: Draft<PhasePlugins>,
    following_suggestions: bool,
    /// What was entered into a number field and not taken, with why, by the path of the field.
    /// Dropped when its section is replaced or edited structurally, which may move the fields.
    rejected: Vec<Rejected>,
}

#[derive(Debug, Clone)]
struct Rejected {
    path: Path,
    entered: String,
    problem: Text,
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
    /// every number entered was taken.
    pub complete: bool,
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
    /// An answer of `inner` that may be left out, see [`PhaseForm::answer_optional`].
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
    /// Where it is, which [`PhaseForm::remove`] and the moves take.
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
}

impl PluginOption {
    fn of(entry: &'static PluginEntry, recommended: bool) -> Self {
        Self {
            id: entry.id(),
            label: entry.label(),
            description: entry.description(),
            recommended,
        }
    }
}

impl PhaseForm {
    /// A form preserving the settings of `draft`. [`Self::apply_suggestions`] starts following
    /// suggestions; [`Session::phase_form`](crate::session::Session::phase_form) does so for a
    /// new phase.
    pub fn new(draft: Draft<PhasePlugins>) -> Self {
        Self {
            draft,
            following_suggestions: false,
            rejected: Vec::new(),
        }
    }

    /// The draft as the form answered it.
    pub fn draft(&self) -> &Draft<PhasePlugins> {
        &self.draft
    }

    /// Replaces the ranking and metrics, including their settings, with the suggestions for the
    /// other phase fields, and resumes following suggestions. Those fields and their rejected
    /// numeric input are kept.
    ///
    /// # Errors
    /// Returns an error if a suggested answer does not fit its schema. Nothing changes then.
    pub fn apply_suggestions(&mut self) -> Result<(), ConfigError> {
        self.draft = suggested(self.draft.clone())?;
        self.following_suggestions = true;
        self.rejected
            .retain(|rejected| !is_suggested(&rejected.path));
        Ok(())
    }

    /// What the form shows now.
    pub fn view(&self) -> FormView {
        let mut completed = self.draft.clone();
        completed.add_implied();
        let issues = completed.build().err().unwrap_or_default();
        let nodes = self.draft.fields();
        let completed_nodes = completed.fields();
        let mut fields = self.fields(&nodes, None, &completed_nodes, &completed_nodes);
        let mut unplaced = Vec::new();
        for issue in &issues {
            let text = issue_text(issue, &completed);
            if !place(&mut fields, issue.path(), &text) {
                unplaced.push(text);
            }
        }
        for rejected in &self.rejected {
            place(&mut fields, &rejected.path, &rejected.problem);
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
            complete: issues.is_empty() && self.rejected.is_empty(),
            issues: unplaced,
        }
    }

    /// Answers the field at `path`, which takes yes or no, with `answer`.
    ///
    /// # Errors
    /// Returns an error if there is no such field, or it takes something else.
    pub fn set_bool(&mut self, path: &Path, answer: bool) -> Result<(), ConfigError> {
        self.change(path, |draft| draft.set(path, answer))
    }

    /// Answers the field at `path`, which takes a number, with the number `entered`, as an operator
    /// typed it: with a point or a comma before its decimals, possibly with an exponent (`1e3`).
    /// Something else, nothing, or a number the field does not take, leaves the answer as it was,
    /// and the field shows what was entered with why it was not taken until the next change other
    /// than entering a number; meanwhile the form is not [complete](FormView::complete).
    ///
    /// # Errors
    /// Returns an error if there is no such field, or it takes no number.
    pub fn enter(&mut self, path: &Path, entered: &str) -> Result<(), ConfigError> {
        let Some(range) = self.kind(path)?.range() else {
            return Err(ConfigError::WrongKind {
                path: path.clone(),
                expected: "an answer that is no number",
                found: "a number",
            });
        };
        self.rejected.retain(|rejected| rejected.path != *path);
        let out_of_range = |range| with_range(Text::new("issue-out-of-range"), range);
        let problem = match number(entered, range) {
            Ok(number) => {
                // a number moves no field, so what other number fields did not take stays
                let others = std::mem::take(&mut self.rejected);
                let changed = self.change(path, |draft| draft.set(path, number));
                self.rejected = others;
                match changed {
                    Ok(()) => return Ok(()),
                    Err(ConfigError::OutOfRange { allowed, .. }) => out_of_range(allowed),
                    Err(error) => return Err(error),
                }
            }
            Err(NotTaken::OutOfRange) => out_of_range(range),
            Err(NotTaken::NoNumber) => {
                Text::new("issue-not-a-number").with("entered", entered.trim())
            }
        };
        self.rejected.push(Rejected {
            path: path.clone(),
            entered: entered.to_owned(),
            problem,
        });
        if is_suggested(path) {
            self.following_suggestions = false;
        }
        Ok(())
    }

    /// Why the field at `path`, which takes a number, would not take `entered`, as
    /// [`Self::enter`] would show it, e.g. while it is being typed; `None` if it would take it.
    ///
    /// # Errors
    /// Returns an error if there is no such field, or it takes no number.
    pub fn issue_entering(&self, path: &Path, entered: &str) -> Result<Option<Text>, ConfigError> {
        let mut form = self.clone();
        form.enter(path, entered)?;
        Ok(form
            .rejected
            .into_iter()
            .find(|rejected| rejected.path == *path)
            .map(|rejected| rejected.problem))
    }

    /// Answers the field at `path`, which may be left without an answer, if `answered`, or else
    /// leaves it without one. An answer it is given is what it offers, or else no, or the number of
    /// its range closest to 0.
    ///
    /// # Errors
    /// Returns an error if there is no such field, or it takes an answer that cannot be left out.
    pub fn answer_optional(&mut self, path: &Path, answered: bool) -> Result<(), ConfigError> {
        let FieldKind::Optional(inner) = self.kind(path)? else {
            return Err(ConfigError::WrongKind {
                path: path.clone(),
                expected: "an answer that cannot be left out",
                found: "none",
            });
        };
        if !answered {
            return self.change(path, |draft| draft.set(path, Value::Null));
        }
        let offered = self
            .draft
            .default_value(path)?
            .filter(|offered| !offered.is_null());
        let answer = match (offered, &*inner) {
            (Some(offered), _) => offered,
            (None, FieldKind::Bool) => Value::from(false),
            // 0 is a multiple of anything, and so are the bounds
            (None, FieldKind::Integer(range)) => Value::from(0.clamp(range.min(), range.max())),
            (None, FieldKind::Decimal(range)) => {
                let at_least = range.min().map_or(0.0, |min| min.max(0.0));
                Value::from(range.max().map_or(at_least, |max| at_least.min(max)))
            }
            (None, _) => Value::Null,
        };
        self.change(path, |draft| draft.set(path, answer))
    }

    /// Answers the field at `path`, which takes one plugin, with the plugin `plugin`, configured as
    /// it offers, or with none for `None`.
    ///
    /// # Errors
    /// Returns an error if there is no such field, it takes no plugin, or no plugin of this id;
    /// for `None`, if it has to take one.
    pub fn choose(&mut self, path: &Path, plugin: Option<&str>) -> Result<(), ConfigError> {
        let answer = plugin.map_or(Value::Null, |plugin| PluginConfig::new(plugin).into());
        self.change(path, |draft| draft.set(path, answer))
    }

    /// Adds the plugin `plugin` to the end of the list at `path`, configured as it offers.
    ///
    /// # Errors
    /// Returns an error if there is no such field, it takes no list, or no plugin of this id, or
    /// has it already.
    pub fn add(&mut self, path: &Path, plugin: &str) -> Result<(), ConfigError> {
        let end = self.items(path)?.len();
        self.change(path, |draft| {
            draft.set(&path.clone().index(end), PluginConfig::new(plugin))
        })
    }

    /// Removes the plugin at `item`, a position in a list, from it.
    ///
    /// # Errors
    /// Returns an error if there is no such position.
    pub fn remove(&mut self, item: &Path) -> Result<(), ConfigError> {
        position(item)?;
        self.change(item, |draft| draft.unset(item))
    }

    /// Moves the plugin at `item`, a position in a list, one up, before the plugin before it;
    /// the first stays where it is.
    ///
    /// # Errors
    /// Returns an error if there is no such position.
    pub fn move_up(&mut self, item: &Path) -> Result<(), ConfigError> {
        let (list, index) = position(item)?;
        self.swap(&list, index.checked_sub(1), index)
    }

    /// Moves the plugin at `item`, a position in a list, one down, after the plugin after it; the
    /// last stays where it is.
    ///
    /// # Errors
    /// Returns an error if there is no such position.
    pub fn move_down(&mut self, item: &Path) -> Result<(), ConfigError> {
        let (list, index) = position(item)?;
        self.swap(&list, Some(index), index + 1)
    }

    /// Swaps the plugins at `first` and `second` of the list at `list`, unless one of them is not
    /// in it.
    fn swap(
        &mut self,
        list: &Path,
        first: Option<usize>,
        second: usize,
    ) -> Result<(), ConfigError> {
        let mut items = self.items(list)?;
        let Some(first) = first.filter(|_| second < items.len()) else {
            return Ok(());
        };
        items.swap(first, second);
        self.change(list, |draft| draft.set(list, Value::Array(items)))
    }

    /// The answers in the list at `path`.
    fn items(&self, path: &Path) -> Result<Vec<Value>, ConfigError> {
        match self.draft.answers().get_at(path) {
            Some(Value::Array(items)) => Ok(items.clone()),
            None if self.kind(path)?.axis().is_some() => Ok(Vec::new()),
            _ => Err(ConfigError::WrongKind {
                path: path.clone(),
                expected: "a list of plugins",
                found: "something else",
            }),
        }
    }

    /// The kind of answer the field at `path` takes.
    fn kind(&self, path: &Path) -> Result<FieldKind, ConfigError> {
        self.draft
            .field_at(path)
            .map(|node| node.field.kind().clone())
            .ok_or_else(|| ConfigError::UnknownField { path: path.clone() })
    }

    /// Applies an edit and fills new fields with defaults. Nothing changes if either fails.
    fn change(
        &mut self,
        path: &Path,
        change: impl FnOnce(&mut Draft<PhasePlugins>) -> Result<(), ConfigError>,
    ) -> Result<(), ConfigError> {
        let mut draft = self.draft.clone();
        change(&mut draft)?;
        let following = self.following_suggestions && !is_suggested(path);
        self.draft = if following {
            suggested(draft)?
        } else {
            draft.with_defaults()?
        };
        self.following_suggestions = following;
        if is_suggested(path) || following {
            self.rejected.clear();
        } else {
            // Basic settings do not move customized ranking or metric fields.
            self.rejected
                .retain(|rejected| is_suggested(&rejected.path));
        }
        Ok(())
    }

    /// The enumerated fields, belonging to `plugin` unless they are the phase's own.
    /// `completed` are the same fields with implied plugins appended.
    fn fields(
        &self,
        nodes: &[FieldNode],
        plugin: Option<&'static str>,
        completed: &[FieldNode],
        all_completed: &[FieldNode],
    ) -> Vec<FormField> {
        nodes
            .iter()
            .zip(completed)
            // Rejected input stays visible even when the field becomes irrelevant.
            .filter(|(node, _)| {
                self.draft.is_relevant(&node.path).unwrap_or(true)
                    || self
                        .rejected
                        .iter()
                        .any(|rejected| rejected.path == node.path)
            })
            .map(|(node, completed)| {
                let field = &node.field;
                FormField {
                    input: self.input(field.kind(), node, completed, all_completed),
                    path: node.path.clone(),
                    plugin,
                    name: field.name(),
                    label: field.label(),
                    description: field.description(),
                    issues: Vec::new(),
                    warnings: Vec::new(),
                }
            })
            .collect()
    }

    /// How the enumerated field is answered, with any implied plugins.
    fn input(
        &self,
        kind: &FieldKind,
        node: &FieldNode,
        completed: &FieldNode,
        all_completed: &[FieldNode],
    ) -> Input {
        let path = &node.path;
        let answer = node.answer;
        let entered = || {
            self.rejected
                .iter()
                .find(|rejected| rejected.path == *path)
                .map(|rejected| rejected.entered.clone())
        };
        match kind {
            FieldKind::Bool => Input::Bool(answer.and_then(Value::as_bool)),
            FieldKind::Integer(range) => Input::Integer {
                range: *range,
                value: answer.and_then(Value::as_i64),
                entered: entered(),
                hint: with_range(Text::new("number-range"), NumberRange::Integer(*range)),
            },
            FieldKind::Decimal(range) => Input::Decimal {
                range: *range,
                value: answer.and_then(Value::as_f64),
                entered: entered(),
                hint: with_range(Text::new("number-range"), NumberRange::Decimal(*range)),
            },
            FieldKind::Plugin(axis) => {
                Input::Plugin(self.plugin(*axis, node, false, all_completed))
            }
            FieldKind::List { axis, ordered } => {
                Input::List(self.list(*axis, *ordered, node, completed, all_completed))
            }
            FieldKind::Optional(inner) => match &**inner {
                FieldKind::Plugin(axis) => {
                    Input::Plugin(self.plugin(*axis, node, true, all_completed))
                }
                FieldKind::List { .. } => self.input(inner, node, completed, all_completed),
                _ => Input::Optional {
                    answered: answer.is_some_and(|answer| !answer.is_null()),
                    inner: Box::new(self.input(inner, node, completed, all_completed)),
                },
            },
        }
    }

    /// The field taking one plugin of `axis`.
    fn plugin(
        &self,
        axis: AnyAxis,
        node: &FieldNode,
        optional: bool,
        all_completed: &[FieldNode],
    ) -> PluginInput {
        let chosen = node.plugins().into_iter().next();
        let recommended = self.recommended(axis, &node.path);
        let options = self.options(&node.path, &recommended);
        PluginInput {
            optional,
            chosen: chosen.map(|chosen| {
                let fields = chosen.fields();
                Chosen {
                    plugin: PluginOption::of(
                        chosen.entry,
                        recommended.contains(&chosen.entry.id()),
                    ),
                    // implied plugins are appended only to the composite's lists
                    fields: self.fields(&fields, Some(chosen.entry.id()), &fields, all_completed),
                }
            }),
            options,
        }
    }

    /// The field taking a list of plugins of `axis`.
    fn list(
        &self,
        axis: AnyAxis,
        ordered: bool,
        node: &FieldNode,
        completed: &FieldNode,
        all_completed: &[FieldNode],
    ) -> ListInput {
        let path = &node.path;
        let chosen = node.plugins();
        // implied plugins are appended to what was chosen
        let implied = &completed.plugins()[chosen.len()..];
        let recommended = self.recommended(axis, path);
        let options = self
            .options(&path.clone().index(chosen.len()), &recommended)
            .into_iter()
            .filter(|option| !implied.iter().any(|plugin| plugin.entry.id() == option.id))
            .collect();
        ListInput {
            ordered,
            items: chosen
                .iter()
                .map(|chosen| {
                    let entry = chosen.entry;
                    let fields = chosen.fields();
                    Item {
                        plugin: PluginOption::of(entry, recommended.contains(&entry.id())),
                        fields: self.fields(&fields, Some(entry.id()), &fields, all_completed),
                        path: chosen.path.clone(),
                        issues: Vec::new(),
                    }
                })
                .collect(),
            implied: implied
                .iter()
                .map(|plugin| Implied {
                    plugin: PluginOption::of(
                        plugin.entry,
                        recommended.contains(&plugin.entry.id()),
                    ),
                    needed_by: needed_by(plugin.entry, axis, all_completed),
                })
                .collect(),
            options,
        }
    }

    /// The ids of the plugins the field at `path` offers.
    fn recommended(&self, axis: AnyAxis, path: &Path) -> Vec<&'static str> {
        let offered = PhasePlugins::default_value(&self.draft, path)
            .ok()
            .flatten();
        let answers = match &offered {
            Some(Value::Array(answers)) => answers.iter().collect(),
            Some(answer) => vec![answer],
            None => Vec::new(),
        };
        answers
            .into_iter()
            .filter_map(|answer| axis.entry(PluginConfig::from_answer(answer)?.plugin()))
            .map(PluginEntry::id)
            .collect()
    }

    /// The registered choices at `path`, the plugins of `recommended` first.
    fn options(&self, path: &Path, recommended: &[&str]) -> Vec<PluginOption> {
        let mut options: Vec<_> = self
            .draft
            .candidates(path)
            .unwrap_or_default()
            .into_iter()
            .map(|entry| PluginOption::of(entry, recommended.contains(&entry.id())))
            .collect();
        // the catalog lists plugins by id, which tells an operator nothing
        options.sort_by_cached_key(|option| {
            (
                recommended
                    .iter()
                    .position(|id| *id == option.id)
                    .unwrap_or(usize::MAX),
                option.label.to_lowercase(),
            )
        });
        options
    }
}

fn is_suggested(path: &Path) -> bool {
    let Some(Segment::Field(name)) = path.segments().first() else {
        return false;
    };
    PhasePlugins::SUGGESTED.contains(&name.as_str())
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

/// The number `entered` as the answer of a field of `range`, see [`PhaseForm::enter`].
fn number(entered: &str, range: NumberRange) -> Result<Value, NotTaken> {
    let text = entered.trim().replace(',', ".");
    if text.is_empty() {
        return Err(NotTaken::OutOfRange);
    }
    let number = typed_number(&text)?;
    match range {
        NumberRange::Integer(_) => {
            // exactly, where a float would round
            if let Ok(whole) = text.parse::<i64>() {
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

/// The number `entered` stands for, as an operator typed it: with a point or a comma before its
/// decimals, possibly with an exponent (`1e3`), spaces around it ignored. A number too large for a
/// float is out of range.
pub(crate) fn typed_number(entered: &str) -> Result<f64, NotTaken> {
    let text = entered.trim().replace(',', ".");
    // what Rust reads as a number besides digits, e.g. `inf` and `NaN`, is no number to an
    // operator
    let numeric = text
        .chars()
        .all(|char| char.is_ascii_digit() || matches!(char, '.' | 'e' | 'E' | '+' | '-'));
    let number = text
        .parse::<f64>()
        .ok()
        .filter(|_| numeric && text.contains(|char: char| char.is_ascii_digit()))
        .ok_or(NotTaken::NoNumber)?;
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
fn issue_text(issue: &Issue, draft: &Draft<PhasePlugins>) -> Text {
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
        Problem::Unprovided {
            fact,
            required_by,
            providers,
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
                    .with("fact", *fact)
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
