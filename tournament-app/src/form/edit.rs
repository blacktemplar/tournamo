//! What forms of composites share: the edits of their fields and the fields they show, generated
//! from the schema of their draft.

use plugin_factory::{
    catalog::{AnyAxis, PluginEntry},
    config::{ConfigError, PluginConfig, Value},
    draft::{Draft, FieldNode},
    schema::{Configurable, FieldKind, NumberRange},
};

use super::{
    Chosen, FormField, Implied, Input, Item, ListInput, NotTaken, Path, PluginInput, PluginOption,
    Whole, needed_by, number, position, with_range,
};
use crate::{
    i18n::{Language, Text},
    rejected::Rejections,
};

/// A form answering a draft of the composite `Self::Of`, whose fields the functions of this module
/// edit.
pub(crate) trait Answering: Clone {
    type Of: Configurable;

    fn draft(&self) -> &Draft<Self::Of>;

    /// The language numbers are typed in.
    fn language(&self) -> Language;

    /// What was entered into a number field and not taken, with why, by the path of the field.
    fn rejected(&self) -> &Rejections<Path>;

    fn rejected_mut(&mut self) -> &mut Rejections<Path>;

    /// Applies `change` to the draft at `path` and fills new fields as the form does. Nothing
    /// changes if either fails.
    fn change(
        &mut self,
        path: &Path,
        change: impl FnOnce(&mut Draft<Self::Of>) -> Result<(), ConfigError>,
    ) -> Result<(), ConfigError>;

    /// Called once what was entered into the number field at `path` was not taken.
    fn not_taken(&mut self, path: &Path) {
        let _ = path;
    }
}

/// See [`Editable::set_bool`](super::Editable::set_bool).
pub(crate) fn set_bool(
    form: &mut impl Answering,
    path: &Path,
    answer: bool,
) -> Result<(), ConfigError> {
    form.change(path, |draft| draft.set(path, answer))
}

/// See [`Editable::enter`](super::Editable::enter).
pub(crate) fn enter(
    form: &mut impl Answering,
    path: &Path,
    entered: &str,
) -> Result<(), ConfigError> {
    let Some(range) = kind(form, path)?.range() else {
        return Err(ConfigError::WrongKind {
            path: path.clone(),
            expected: "an answer that is no number",
            found: "a number",
        });
    };
    form.rejected_mut().forget(path);
    let out_of_range = |range| with_range(Text::new("issue-out-of-range"), range);
    let problem = match number(entered, range, form.language()) {
        Ok(number) => {
            // a number moves no field, so what other number fields did not take stays
            let others = std::mem::take(form.rejected_mut());
            let changed = form.change(path, |draft| draft.set(path, number));
            *form.rejected_mut() = others;
            match changed {
                Ok(()) => return Ok(()),
                Err(ConfigError::OutOfRange { allowed, .. }) => out_of_range(allowed),
                Err(error) => return Err(error),
            }
        }
        Err(NotTaken::OutOfRange) => out_of_range(range),
        Err(NotTaken::NoNumber) => Text::new("issue-not-a-number").with("entered", entered.trim()),
    };
    form.rejected_mut().reject(path.clone(), entered, problem);
    form.not_taken(path);
    Ok(())
}

/// See [`Editable::issue_entering`](super::Editable::issue_entering).
pub(crate) fn issue_entering(
    form: &impl Answering,
    path: &Path,
    entered: &str,
) -> Result<Option<Text>, ConfigError> {
    let mut form = form.clone();
    enter(&mut form, path, entered)?;
    Ok(form
        .rejected()
        .get(path)
        .map(|rejected| rejected.problem.clone()))
}

/// See [`Editable::answer_optional`](super::Editable::answer_optional).
pub(crate) fn answer_optional(
    form: &mut impl Answering,
    path: &Path,
    answered: bool,
) -> Result<(), ConfigError> {
    let FieldKind::Optional(inner) = kind(form, path)? else {
        return Err(ConfigError::WrongKind {
            path: path.clone(),
            expected: "an answer that cannot be left out",
            found: "none",
        });
    };
    if !answered {
        return form.change(path, |draft| draft.set(path, Value::Null));
    }
    let offered = form
        .draft()
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
    form.change(path, |draft| draft.set(path, answer))
}

/// See [`Editable::choose`](super::Editable::choose).
pub(crate) fn choose(
    form: &mut impl Answering,
    path: &Path,
    plugin: Option<&str>,
) -> Result<(), ConfigError> {
    let answer = plugin.map_or(Value::Null, |plugin| PluginConfig::new(plugin).into());
    form.change(path, |draft| draft.set(path, answer))
}

/// See [`Editable::add`](super::Editable::add).
pub(crate) fn add(form: &mut impl Answering, path: &Path, plugin: &str) -> Result<(), ConfigError> {
    let end = items(form, path)?.len();
    form.change(path, |draft| {
        draft.set(&path.clone().index(end), PluginConfig::new(plugin))
    })
}

/// See [`Editable::remove`](super::Editable::remove).
pub(crate) fn remove(form: &mut impl Answering, item: &Path) -> Result<(), ConfigError> {
    position(item)?;
    form.change(item, |draft| draft.unset(item))
}

/// See [`Editable::move_up`](super::Editable::move_up).
pub(crate) fn move_up(form: &mut impl Answering, item: &Path) -> Result<(), ConfigError> {
    let (list, index) = position(item)?;
    swap(form, &list, index.checked_sub(1), index)
}

/// See [`Editable::move_down`](super::Editable::move_down).
pub(crate) fn move_down(form: &mut impl Answering, item: &Path) -> Result<(), ConfigError> {
    let (list, index) = position(item)?;
    swap(form, &list, Some(index), index + 1)
}

/// Swaps the plugins at `first` and `second` of the list at `list`, unless one of them is not in
/// it.
fn swap(
    form: &mut impl Answering,
    list: &Path,
    first: Option<usize>,
    second: usize,
) -> Result<(), ConfigError> {
    let mut items = items(form, list)?;
    let Some(first) = first.filter(|_| second < items.len()) else {
        return Ok(());
    };
    items.swap(first, second);
    form.change(list, |draft| draft.set(list, Value::Array(items)))
}

/// The answers in the list at `path`.
fn items(form: &impl Answering, path: &Path) -> Result<Vec<Value>, ConfigError> {
    match form.draft().answers().get_at(path) {
        Some(Value::Array(items)) => Ok(items.clone()),
        None if kind(form, path)?.axis().is_some() => Ok(Vec::new()),
        _ => Err(ConfigError::WrongKind {
            path: path.clone(),
            expected: "a list of plugins",
            found: "something else",
        }),
    }
}

/// The kind of answer the field at `path` takes.
fn kind(form: &impl Answering, path: &Path) -> Result<FieldKind, ConfigError> {
    form.draft()
        .field_at(path)
        .map(|node| node.field.kind().clone())
        .ok_or_else(|| ConfigError::UnknownField { path: path.clone() })
}

/// What the fields of a draft are shown from.
pub(super) struct Generator<'a, T> {
    pub(super) draft: &'a Draft<T>,
    pub(super) rejected: &'a Rejections<Path>,
    /// The answer the field at a path offers, e.g. [`PhasePlugins::default_value`].
    ///
    /// [`PhasePlugins::default_value`]: tournament_engine::PhasePlugins::default_value
    pub(super) offered: fn(&Draft<T>, &Path) -> Result<Option<Value>, ConfigError>,
}

impl<T: Configurable> Generator<'_, T> {
    /// The enumerated fields, belonging to `plugin` unless they are the phase's own.
    /// `completed` are the same fields with implied plugins appended.
    pub(super) fn fields(
        &self,
        nodes: &[FieldNode],
        plugin: Option<&'static str>,
        completed: &[FieldNode],
        whole: &Whole,
    ) -> Vec<FormField> {
        nodes
            .iter()
            .zip(completed)
            // Rejected input stays visible even when the field becomes irrelevant.
            .filter(|(node, _)| {
                !whole.irrelevant.contains(&node.path) || self.rejected.contains(&node.path)
            })
            .map(|(node, completed)| {
                let field = &node.field;
                FormField {
                    input: self.input(field.kind(), node, completed, whole),
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
        whole: &Whole,
    ) -> Input {
        let path = &node.path;
        let answer = node.answer;
        let entered = || {
            self.rejected
                .get(path)
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
            FieldKind::Plugin(axis) => Input::Plugin(self.plugin(*axis, node, false, whole)),
            FieldKind::List { axis, ordered } => {
                Input::List(self.list(*axis, *ordered, node, completed, whole))
            }
            FieldKind::Optional(inner) => match &**inner {
                FieldKind::Plugin(axis) => Input::Plugin(self.plugin(*axis, node, true, whole)),
                FieldKind::List { .. } => self.input(inner, node, completed, whole),
                _ => Input::Optional {
                    answered: answer.is_some_and(|answer| !answer.is_null()),
                    inner: Box::new(self.input(inner, node, completed, whole)),
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
        whole: &Whole,
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
                    fields: self.fields(&fields, Some(chosen.entry.id()), &fields, whole),
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
        whole: &Whole,
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
                        fields: self.fields(&fields, Some(entry.id()), &fields, whole),
                        path: chosen.path.clone(),
                        issues: Vec::new(),
                    }
                })
                .collect(),
            shown: Vec::new(),
            implied: implied
                .iter()
                .map(|plugin| Implied {
                    plugin: PluginOption::of(
                        plugin.entry,
                        recommended.contains(&plugin.entry.id()),
                    ),
                    needed_by: needed_by(plugin.entry, axis, whole.completed),
                })
                .collect(),
            options,
        }
    }

    /// The ids of the plugins the field at `path` offers.
    fn recommended(&self, axis: AnyAxis, path: &Path) -> Vec<&'static str> {
        let offered = (self.offered)(self.draft, path).ok().flatten();
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
