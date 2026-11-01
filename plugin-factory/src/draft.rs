//! A composite of plugins being configured, one answer at a time.
//!
//! A [`Draft`] holds the answers given so far to the schema of a composite and to the schemas of
//! the plugins answering its fields, as one tree in which a [`Path`] addresses a field or a
//! position in a list. A UI renders the fields from the schemas along the
//! [answers](Draft::answers), see [`Draft`], and changes the tree through [`Draft::set`] and
//! [`Draft::unset`] alone, so that every answer in it fits the schema.
//!
//! A draft also answers what a UI asks about it: the [default](Draft::default_value) of a field,
//! the [candidates](Draft::candidates) for a position taking plugins and the
//! [issues](Draft::issues) of the answers so far. [`Draft::add_implied`] adds the plugins the
//! answers imply, and [`Draft::build`] builds the composite from the answers as they are.
//! All of
//! them take the composite's context into account, i.e. what is true of it before any plugin is
//! chosen, which the draft is created with as a closure declaring it as a plugin declares, see
//! [`compatibility::check`].
//!
//! Choices are not filtered by compatibility. Issues describe the current answers; a missing
//! fact is postponed while an unanswered field's axis could provide it. [`Draft::build`] checks
//! the complete configuration with nothing open.

mod fields;

use std::{
    any::TypeId,
    fmt::{self, Display, Formatter},
    marker::PhantomData,
    sync::Arc,
};

pub use fields::{FieldNode, PluginNode};

use crate::{
    Plugin,
    catalog::{AnyAxis, PluginEntry},
    compatibility::{self, Declaration, Declared, Incompatibility, OpenFacts, RequiredFact},
    config::{self, Config, ConfigError, Path, PluginConfig, Segment, Value},
    schema::{Configurable, Field, FieldKind, Schema},
};

/// The answers to the schema of the composite `T` given so far, see the [module](self).
///
/// # Examples
/// A UI walks the fields and their chosen plugins. Each field carries its schema, answer, and
/// path, so the UI need not interpret plugin configs or resolve paths to render it:
///
/// ```
/// use plugin_factory::{
///     Plugin, axis,
///     config::PluginConfig,
///     draft::{Draft, FieldNode},
///     register,
///     schema::Configurable,
/// };
/// use serde::{Deserialize, Serialize};
///
/// #[axis]
/// pub trait Seeding: Plugin {}
///
/// #[derive(Debug, Serialize, Deserialize, Plugin)]
/// #[plugin(
///     id = "my_crate::random",
///     label = "random",
///     description = "entrants are shuffled"
/// )]
/// struct Random {
///     #[plugin(label = "Keep the top seeds", default = 0)]
///     keep_top: u32,
/// }
///
/// #[register]
/// impl Seeding for Random {}
///
/// #[derive(Deserialize, Configurable)]
/// struct Phase {
///     #[plugin(label = "Seedings")]
///     seedings: Vec<Box<dyn Seeding>>,
/// }
///
/// fn fields(nodes: &[FieldNode], lines: &mut Vec<String>) {
///     for node in nodes {
///         lines.push(format!("{}: {}", node.path, node.field.label()));
///         for plugin in &node.plugins {
///             fields(&plugin.fields, lines);
///         }
///     }
/// }
///
/// # fn main() {
/// let mut draft = Draft::<Phase>::new(|_| {});
/// draft
///     .set(
///         &"seedings".parse().unwrap(),
///         vec![PluginConfig::of::<Random>()],
///     )
///     .unwrap();
///
/// let mut lines = Vec::new();
/// fields(&draft.fields(), &mut lines);
/// assert_eq!(
///     lines,
///     [
///         "seedings: Seedings",
///         "seedings[0].keep_top: Keep the top seeds"
///     ]
/// );
/// # }
/// ```
pub struct Draft<T> {
    answers: Config,
    /// What is true of the composite before any plugin is chosen.
    context: Arc<Declaration>,
    composite: PhantomData<fn() -> T>,
}

impl<T: Configurable> Draft<T> {
    /// A draft with nothing answered, of a composite whose `context` declares what is true of it
    /// before any plugin is chosen, as a plugin declares; `|_| {}` declares nothing.
    pub fn new(context: impl FnOnce(&mut Declaration)) -> Self {
        Self {
            answers: Config::new(),
            context: Arc::new(Declaration::of(context)),
            composite: PhantomData,
        }
    }

    /// Returns this draft with the field `field` of the composite answered by `answer`, see
    /// [`Self::set`]; the fields of a plugin are answered in its [`PluginConfig`].
    ///
    /// # Examples
    /// ```
    /// use plugin_factory::{
    ///     Plugin, axis, config::PluginConfig, draft::Draft, register, schema::Configurable,
    /// };
    /// use serde::{Deserialize, Serialize};
    ///
    /// #[axis]
    /// pub trait Seeding: Plugin {}
    ///
    /// #[derive(Debug, Serialize, Deserialize, Plugin)]
    /// #[plugin(id = "my_crate::random", label = "random")]
    /// struct Random {
    ///     #[plugin(label = "Keep the top seeds")]
    ///     keep_top: u32,
    /// }
    ///
    /// #[register]
    /// impl Seeding for Random {}
    ///
    /// #[derive(Debug, Deserialize, Configurable)]
    /// struct Phase {
    ///     #[plugin(label = "Seedings")]
    ///     seedings: Vec<Box<dyn Seeding>>,
    /// }
    ///
    /// let phase = Draft::<Phase>::new(|_| {})
    ///     .with(
    ///         "seedings",
    ///         vec![PluginConfig::of::<Random>().with("keep_top", 2)],
    ///     )?
    ///     .build()
    ///     .expect("every field is answered");
    ///
    /// assert_eq!(
    ///     format!("{phase:?}"),
    ///     "Phase { seedings: [Random { keep_top: 2 }] }"
    /// );
    /// # Ok::<(), plugin_factory::config::ConfigError>(())
    /// ```
    ///
    /// # Errors
    /// Returns what [`Self::set`] returns.
    pub fn with(mut self, field: &str, answer: impl Into<Value>) -> Result<Self, ConfigError> {
        self.set(&Path::ROOT.field(field), answer)?;
        Ok(self)
    }

    /// Returns this draft with `answers` in place of every answer it holds, e.g. the answers a
    /// composite was built from, so that they can be changed. They are checked as [`Self::set`]
    /// checks an answer, and an answer to a field the schema leaves out is dropped.
    ///
    /// # Errors
    /// Returns what [`Self::set`] returns, with paths seen from the root.
    pub fn with_answers(mut self, answers: &Config) -> Result<Self, ConfigError> {
        self.answers = T::schema().walk(answers, &Path::ROOT, false)?;
        Ok(self)
    }

    /// Returns this draft with every unanswered field answered by what it offers, see
    /// [`Self::default_value`]. The fields are answered in schema order, each followed by the
    /// fields of the plugins answering it. A field offering nothing stays unanswered.
    ///
    /// # Errors
    /// Returns what [`Self::set`] returns for a schema default.
    pub fn with_defaults(mut self) -> Result<Self, ConfigError> {
        self.answer_defaults(&T::schema(), &Path::ROOT)?;
        Ok(self)
    }

    /// Answers every unanswered field of `schema`, the schema of the config at `at`, by what it
    /// offers, see [`Self::with_defaults`].
    fn answer_defaults(&mut self, schema: &Schema, at: &Path) -> Result<(), ConfigError> {
        for field in schema.fields() {
            let path = at.clone().field(field.name());
            if self.answers.get_at(&path).is_none()
                && let Some(default) = self.default_value(&path)?
            {
                self.set(&path, default)?;
            }
            self.answer_plugin_defaults(field.kind(), &path)?;
        }
        Ok(())
    }

    /// Answers the unanswered fields of the plugins answering the position of `kind` at `at`, see
    /// [`Self::with_defaults`].
    fn answer_plugin_defaults(&mut self, kind: &FieldKind, at: &Path) -> Result<(), ConfigError> {
        let Some(answer) = self.answers.get_at(at).cloned() else {
            return Ok(());
        };
        match (unwrapped(kind), answer) {
            (FieldKind::List { axis, .. }, Value::Array(answers)) => {
                for position in 0..answers.len() {
                    let at = at.clone().index(position);
                    self.answer_plugin_defaults(&FieldKind::Plugin(*axis), &at)?;
                }
            }
            (FieldKind::Plugin(axis), answer) => {
                // an answer is checked when it is set, so it names a plugin of the axis unless it
                // is nothing
                if let Some(entry) = PluginConfig::from_answer(&answer)
                    .and_then(|plugin| axis.entry(plugin.plugin()))
                {
                    self.answer_defaults(&entry.schema(), at)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// The answers given so far, as the config of the composite.
    pub fn answers(&self) -> &Config {
        &self.answers
    }

    /// The fields in schema order, each with its answer and the fields of its chosen plugins.
    /// Unanswered fields are included; defaults and implied plugins are not added.
    pub fn fields(&self) -> Vec<FieldNode> {
        fields::fields(&T::schema(), &self.answers, &Path::ROOT)
    }

    /// Answers the position at `path` with `answer`: a field, a position in a list, or the
    /// position just past its end, which appends `answer` to the list.
    ///
    /// The answer may leave fields of the plugins in it unanswered. What a plugin rejects is not
    /// checked here but reported as an [issue](Self::issues).
    ///
    /// # Errors
    /// Returns [`ConfigError::UnknownField`] if `path` addresses nothing in the answers or the
    /// answer answers a field a schema does not have, [`ConfigError::WrongKind`] if an answer is
    /// not one its field takes, [`ConfigError::UnknownPlugin`] if a plugin in it is not
    /// registered for the axis of its field, and [`ConfigError::Duplicate`] if a plugin it puts
    /// into a list is in that list already, with paths seen from the root. Nothing is changed
    /// then.
    pub fn set(&mut self, path: &Path, answer: impl Into<Value>) -> Result<(), ConfigError> {
        set_in(&T::schema(), &mut self.answers, path, &answer.into())
    }

    /// Takes back the answer at `path`: a field is unanswered again, and a position in a list is
    /// removed from it, so that the positions after it move up by one.
    ///
    /// # Errors
    /// Returns [`ConfigError::UnknownField`] if `path` addresses nothing in the answers. Nothing
    /// is changed then.
    pub fn unset(&mut self, path: &Path) -> Result<(), ConfigError> {
        let position = locate(&T::schema(), &self.answers, path)?;
        if position.field.is_none() && position.answer.is_none() {
            return Err(ConfigError::UnknownField { path: path.clone() });
        }
        edit(&mut self.answers, path, None);
        Ok(())
    }

    /// The answer the field at `path` offers, or [`None`] if it offers none or `path` addresses a
    /// position in a list rather than a field. It is never filled in: a UI offering it sets it as
    /// the answer, see [`Self::set`].
    ///
    /// Only the field's schema determines its default, independently of the other answers.
    ///
    /// # Errors
    /// Returns [`ConfigError::UnknownField`] if `path` addresses nothing in the answers.
    pub fn default_value(&self, path: &Path) -> Result<Option<Value>, ConfigError> {
        let schema = T::schema();
        let Some(field) = locate(&schema, &self.answers, path)?.field else {
            return Ok(None);
        };
        Ok(field.default_value().cloned())
    }

    /// Whether the field at `path` matters with the answers given: not if the composite provides
    /// the fact that makes it irrelevant (see [`Field::irrelevant_with`]), its context or a plugin
    /// answered so far; a position in a list always does. A UI may leave out a field that does
    /// not; it is answered all the same.
    ///
    /// # Errors
    /// Returns [`ConfigError::UnknownField`] if `path` addresses nothing in the answers.
    pub fn is_relevant(&self, path: &Path) -> Result<bool, ConfigError> {
        let schema = T::schema();
        let position = locate(&schema, &self.answers, path)?;
        let Some(fact) = position.field.and_then(|field| field.irrelevant_with()) else {
            return Ok(true);
        };
        let provided = self.context.provides_type(fact)
            || Tree::of(&schema, &self.answers)
                .declarations()
                .iter()
                .any(|declared| declared.declaration.provides_type(fact));
        Ok(!provided)
    }

    /// The registered plugins of the axis at `path`, in id order; empty for scalar fields.
    /// Compatibility does not restrict the choices: [`Self::issues`] and [`Self::build`] report
    /// problems after selection. At a list position, plugins used at other positions of that
    /// list are excluded. At the list field itself, all plugins of its axis are returned.
    ///
    /// # Errors
    /// Returns [`ConfigError::UnknownField`] if `path` addresses nothing in the answers. A path may
    /// address the position just past the end of a list.
    pub fn candidates(&self, path: &Path) -> Result<Vec<&'static PluginEntry>, ConfigError> {
        let schema = T::schema();
        let position = locate(&schema, &self.answers, path)?;
        let Some((axis, _)) = slot(&position.kind) else {
            return Ok(Vec::new());
        };
        let taken: Vec<_> = match path.segments().last() {
            Some(Segment::Index(index)) => self
                .answers
                .get_at(&path.parent().expect("an index belongs to a list"))
                .and_then(Value::as_array)
                .iter()
                .flat_map(|items| items.iter().enumerate())
                .filter(|(other, _)| other != index)
                .filter_map(|(_, answer)| PluginConfig::from_answer(answer))
                .map(|plugin| plugin.plugin().to_owned())
                .collect(),
            _ => Vec::new(),
        };
        Ok(axis
            .entries()
            .filter(|entry| !taken.iter().any(|id| id == entry.id()))
            .collect())
    }

    /// Everything currently wrong with the answers, each at the path where it is fixed, in no
    /// particular order: an answer a plugin rejects, a plugin that does not fit next to the
    /// others, and a fact a plugin requires that nothing provides and no unanswered field's axis
    /// could provide. No feasible future completion is predicted.
    ///
    /// An unanswered field is no issue, but still to be answered. A plugin with an unanswered field
    /// counts with its static declaration only.
    pub fn issues(&self) -> Vec<Issue> {
        issues_in(&self.context, &Tree::of(&T::schema(), &self.answers), false)
    }

    /// Appends to the answers every plugin they imply: for a fact a plugin requires and neither
    /// the plugins nor the context provide, the one plugin without fields that provides it, so
    /// that there is neither a plugin to choose nor anything to configure.
    ///
    /// A plugin is implied by a fact if it is the only plugin without fields providing the fact
    /// among the axes of the fields of the composite taking a list of plugins, and its axis is
    /// taken by one such field only: the plugin is appended to that field, answering it if it is
    /// unanswered. What it requires that nothing provides implies plugins in turn, and the plugins
    /// so found are appended only all together and only if none of them raises an issue: a
    /// conflict, a fact missing, or a condition an existing plugin requires of what they provide.
    /// A field taking one plugin, and a field of a plugin, is never added to. This repeats until
    /// nothing changes, so adding the implied plugins once more adds nothing.
    ///
    /// A plugin so appended stays an answer like any other, even if the plugin requiring its fact
    /// is taken back later; for a draft that may still change, add them to a clone of it.
    pub fn add_implied(&mut self) {
        add_implied(&self.context, &T::schema(), &mut self.answers);
    }

    /// The composite built from the answers as they are.
    ///
    /// Every field has to be answered; defaults are not filled in, and no plugin is added: a fact
    /// an implied plugin would provide is missing unless [`Self::add_implied`] added it before.
    ///
    /// # Errors
    /// Returns the [issues](Self::issues) of the answers, with nothing open any more: an
    /// unanswered field is a [`Problem::Invalid`] issue with [`ConfigError::Unanswered`], and a
    /// fact nothing provides is [`Problem::Unprovided`].
    pub fn build(&self) -> Result<T, Vec<Issue>> {
        let schema = T::schema();
        let invalid = |error: ConfigError| {
            vec![Issue {
                path: error.path().clone(),
                problem: Problem::Invalid(error),
            }]
        };
        let built = schema
            .walk(&self.answers, &Path::ROOT, true)
            .and_then(|walked| {
                config::deserialize::<T>(&config::complete::<T>(walked), &Path::ROOT)
            });
        let tree = match &built {
            Ok(built) => Tree::with_plugins(&schema, &self.answers, built.plugins()),
            Err(_) => {
                // Serde does not return the partially built object. Inspect complete sub-plugins
                // independently so one rejected field does not hide the other diagnostics.
                Tree::of(&schema, &self.answers)
            }
        };
        let issues = issues_in(&self.context, &tree, true);
        if !issues.is_empty() {
            return Err(issues);
        }
        built.map_err(invalid)
    }
}

// written out rather than derived, which would demand `T: Clone` of the composite
impl<T> Clone for Draft<T> {
    fn clone(&self) -> Self {
        Self {
            answers: self.answers.clone(),
            context: Arc::clone(&self.context),
            composite: PhantomData,
        }
    }
}

/// [`Draft::issues`] of the answers `tree` holds in a composite with `context`. If `complete`,
/// nothing is open any more: an unanswered field is an issue and provides nothing.
fn issues_in(context: &Declaration, tree: &Tree, complete: bool) -> Vec<Issue> {
    let mut issues = Vec::new();
    if complete {
        for question in tree
            .questions
            .iter()
            .filter(|question| question.answer.is_none())
        {
            issues.push(Issue {
                path: question.path.clone(),
                problem: Problem::Invalid(ConfigError::Unanswered {
                    path: question.path.clone(),
                }),
            });
        }
    }
    for placed in &tree.plugins {
        if let Some(error) = &placed.error
            && !matches!(error, ConfigError::Unanswered { .. })
        {
            let issue = Issue {
                path: error.path().clone(),
                problem: Problem::Invalid(error.clone()),
            };
            // a plugin is built with the plugins answering its fields, so an error of one of
            // those is found again building the plugin holding it
            if !issues.contains(&issue) {
                issues.push(issue);
            }
        }
    }

    // An unanswered field postpones missing facts of its axis; no completion is predicted.
    let unanswered = if complete {
        Vec::new()
    } else {
        tree.slots(|question| question.answer.is_none())
    };
    let declarations = tree.declarations();
    let mut unprovided = Vec::new();
    for (index, placed) in tree.plugins.iter().enumerate() {
        let others = all_but(&declarations, index).copied();
        let mut incompatible = None;
        for conflict in
            compatibility::conflicts(context, others, &placed.declared, &OpenFacts::NOTHING)
        {
            match conflict.incompatibility {
                Incompatibility::FactNotProvided { .. }
                    if able(&unanswered, conflict.fact).next().is_some() => {}
                // reported below, at the fields that could provide it
                Incompatibility::FactNotProvided { fact, .. } => {
                    unprovided.push((conflict.fact, fact, placed.path.clone()));
                }
                incompatibility => {
                    incompatible.get_or_insert(incompatibility);
                }
            }
        }
        if let Some(incompatibility) = incompatible {
            issues.push(Issue {
                path: placed.path.clone(),
                problem: Problem::Incompatible(incompatibility),
            });
        }
    }

    let slots = tree.slots(|_| true);
    for (fact, name, required_by) in unprovided {
        let fields: Vec<_> = able(&slots, fact).collect();
        if fields.is_empty() {
            issues.push(Issue {
                path: required_by.clone(),
                problem: Problem::Unprovided {
                    fact: name,
                    required_by,
                    providers: Vec::new(),
                },
            });
            continue;
        }
        for (path, axis) in fields {
            let providers: Vec<_> = axis
                .entries()
                .filter(|entry| entry.declaration().provides_type(fact))
                .map(PluginEntry::id)
                .collect();
            issues.push(Issue {
                path: path.clone(),
                problem: Problem::Unprovided {
                    fact: name,
                    required_by: required_by.clone(),
                    providers,
                },
            });
        }
    }
    issues
}

/// Appends to `answers`, the answers to `schema` of a composite with `context`, every plugin they
/// imply, see [`Draft::add_implied`].
fn add_implied(context: &Declaration, schema: &Schema, answers: &mut Config) {
    // an addition may provide what the chain of a fact tried before it lacked, so every fact is
    // tried again after each addition, which makes adding the implied plugins once more add
    // nothing. This ends, since every plugin added provides a fact nothing provided, so it is not
    // in its list yet, and the catalog is finite.
    while let Some(chain) = first_chain(context, schema, answers) {
        for (name, entry) in chain {
            let mut list = match answers.get(name) {
                Some(Value::Array(list)) => list.clone(),
                _ => Vec::new(),
            };
            list.push(PluginConfig::new(entry.id()).into());
            answers.set(name, list);
        }
    }
}

/// The [`chain`] of the first fact the plugins in `answers`, the answers to `schema`, require that
/// neither they nor `context` provide and that implies plugins; [`None`] if there is none.
fn first_chain(
    context: &Declaration,
    schema: &Schema,
    answers: &Config,
) -> Option<Vec<(&'static str, &'static PluginEntry)>> {
    let lists = top_lists(schema);
    let tree = Tree::of(schema, answers);
    let declarations = tree.declarations();
    compatibility::unprovided(&declarations)
        .filter(|&fact| !context.provides_type(fact))
        .find_map(|fact| chain(context, &declarations, &lists, fact))
}

/// The plugins [`Draft::add_implied`] adds to `lists` so that `fact` is provided in a composite
/// with `context` and plugins with `declarations`, each with the name of the list it is appended
/// to, in the order they are added in; [`None`] if it adds nothing.
///
/// The fact is provided by its [`implied`] plugin, and so is every fact that plugin requires that
/// nothing provides, in turn. The plugins are added only all together, and only if none of them
/// conflicts with the others or the context, none of them misses a fact, and none of them breaks
/// a condition an existing plugin requires of what it provides.
fn chain(
    context: &Declaration,
    declarations: &[&Declared],
    lists: &[(&'static str, AnyAxis)],
    fact: TypeId,
) -> Option<Vec<(&'static str, &'static PluginEntry)>> {
    let mut added: Vec<(&'static str, &'static PluginEntry, Declared)> = Vec::new();
    let mut pending = vec![fact];
    // every plugin added provides the fact it is added for, so each fact is looked up at most once
    while let Some(fact) = pending.pop() {
        let provided = context.provides_type(fact)
            || declarations
                .iter()
                .copied()
                .chain(added.iter().map(|(_, _, declared)| declared))
                .any(|declared| declared.declaration.provides_type(fact));
        if provided {
            continue;
        }
        let (name, entry) = implied(lists, fact)?;
        let declared = Declared::new(entry.label(), entry.declaration_or_static(&Config::new()));
        pending.extend(
            declared
                .declaration
                .required()
                .iter()
                .map(RequiredFact::type_id),
        );
        added.push((name, entry, declared));
    }

    let all: Vec<_> = declarations
        .iter()
        .copied()
        .chain(added.iter().map(|(_, _, declared)| declared))
        .collect();
    let is_added_fact = |fact| {
        added
            .iter()
            .any(|(_, _, declared)| declared.declaration.provides_type(fact))
    };
    for (index, declared) in all.iter().enumerate() {
        let is_added = index >= declarations.len();
        let others = all_but(&all, index).copied();
        // an existing plugin's own problems are not the chain's, unless they are about what the
        // chain provides
        if compatibility::conflicts(context, others, declared, &OpenFacts::NOTHING)
            .iter()
            .any(|conflict| is_added || is_added_fact(conflict.fact))
        {
            return None;
        }
    }
    Some(
        added
            .into_iter()
            .map(|(name, entry, _)| (name, entry))
            .collect(),
    )
}

/// The fields of `schema` taking a list of plugins, by name, each with its axis.
fn top_lists(schema: &Schema) -> Vec<(&'static str, AnyAxis)> {
    schema
        .fields()
        .iter()
        .filter_map(|field| match slot(field.kind()) {
            Some((axis, true)) => Some((field.name(), axis)),
            _ => None,
        })
        .collect()
}

/// The plugin `fact` implies, and the name of the list among `lists` it is added to: the only
/// plugin without fields providing it among the axes of `lists`, or [`None`] if there is none or
/// more than one.
fn implied(
    lists: &[(&'static str, AnyAxis)],
    fact: TypeId,
) -> Option<(&'static str, &'static PluginEntry)> {
    // an axis taken by two fields offers each provider twice, so it never implies a plugin
    let mut providers = lists.iter().flat_map(|(name, axis)| {
        // only a plugin without fields is complete without being asked anything
        axis.entries()
            .filter(move |entry| {
                entry.declaration().provides_type(fact)
                    && entry.instance(&Config::new(), &Path::ROOT).is_ok()
            })
            .map(move |entry| (*name, entry))
    });
    let only = providers.next()?;
    providers.next().is_none().then_some(only)
}

/// Something currently wrong with the answers of a [`Draft`], at the path where it is fixed, see
/// [`Draft::issues`].
#[derive(Debug, PartialEq, Eq)]
pub struct Issue {
    path: Path,
    problem: Problem,
}

impl Issue {
    /// The field or position in a list where the issue is fixed.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// What is wrong.
    pub fn problem(&self) -> &Problem {
        &self.problem
    }
}

/// Shows the path followed by the problem, as `path: problem`.
impl Display for Issue {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path, self.problem)
    }
}

/// What is wrong, see [`Issue`].
#[derive(Debug, PartialEq, Eq)]
pub enum Problem {
    /// The plugin an answer configures rejects it. The issue is at the path of the error.
    Invalid(ConfigError),
    /// The plugin at the issue's path does not fit next to the others.
    Incompatible(Incompatibility),
    /// The plugin at `required_by` requires `fact`, which nothing provides and no unanswered field
    /// could provide. The issue is at each field whose axis could provide it,
    /// with the ids of the plugins of that axis that do, or at `required_by` if there is none.
    Unprovided {
        fact: &'static str,
        required_by: Path,
        providers: Vec<&'static str>,
    },
}

/// Shows an [`Unprovided`](Self::Unprovided) like [`Incompatibility::FactNotProvided`], followed
/// by the providers in parentheses if there are any.
impl Display for Problem {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(error) => error.fmt(formatter),
            Self::Incompatible(incompatibility) => incompatibility.fmt(formatter),
            Self::Unprovided {
                fact,
                required_by,
                providers,
            } => {
                let unprovided = Incompatibility::FactNotProvided {
                    fact,
                    required_by: required_by.to_string(),
                };
                write!(formatter, "{unprovided}")?;
                if !providers.is_empty() {
                    write!(formatter, " (provided by {})", providers.join(", "))?;
                }
                Ok(())
            }
        }
    }
}

/// The axis of the plugins a position of `kind` takes and whether it takes a list of them, or
/// [`None`] if it takes no plugin.
fn slot(kind: &FieldKind) -> Option<(AnyAxis, bool)> {
    match unwrapped(kind) {
        FieldKind::Plugin(axis) => Some((*axis, false)),
        FieldKind::List { axis, .. } => Some((*axis, true)),
        _ => None,
    }
}

/// `kind` without the [`FieldKind::Optional`]s around it.
fn unwrapped(kind: &FieldKind) -> &FieldKind {
    match kind {
        FieldKind::Optional(inner) => unwrapped(inner),
        _ => kind,
    }
}

/// What is at a path of the answers, see [`locate`].
struct Position {
    /// The field, unless the path addresses a position in a list.
    field: Option<Field>,
    kind: FieldKind,
    answer: Option<Value>,
}

/// What is at `path` in `answers`, the answers to `schema`.
///
/// # Errors
/// Returns [`ConfigError::UnknownField`] if `path` addresses nothing in the answers: the root, a
/// field its config does not have, a field of a plugin that is not chosen, or a position in a
/// list further than just past its end.
fn locate(schema: &Schema, answers: &Config, path: &Path) -> Result<Position, ConfigError> {
    let unknown = || ConfigError::UnknownField { path: path.clone() };
    let mut segments = path.segments().iter();
    let Some(Segment::Field(name)) = segments.next() else {
        return Err(unknown());
    };
    let field = schema.field_named(name).ok_or_else(unknown)?;
    let mut position = Position {
        field: Some(field.clone()),
        kind: field.kind().clone(),
        answer: answers.get(name).cloned(),
    };
    for segment in segments {
        position = match segment {
            Segment::Index(index) => {
                let (FieldKind::List { axis, .. }, Some(Value::Array(answers))) =
                    (unwrapped(&position.kind), &position.answer)
                else {
                    return Err(unknown());
                };
                if *index > answers.len() {
                    return Err(unknown());
                }
                Position {
                    field: None,
                    kind: FieldKind::Plugin(*axis),
                    answer: answers.get(*index).cloned(),
                }
            }
            Segment::Field(name) => {
                let FieldKind::Plugin(axis) = unwrapped(&position.kind) else {
                    return Err(unknown());
                };
                let plugin = position
                    .answer
                    .as_ref()
                    .and_then(PluginConfig::from_answer)
                    .ok_or_else(unknown)?;
                let schema = axis.entry(plugin.plugin()).ok_or_else(unknown)?.schema();
                let field = schema.field_named(name).ok_or_else(unknown)?;
                Position {
                    field: Some(field.clone()),
                    kind: field.kind().clone(),
                    answer: plugin.config().get(name).cloned(),
                }
            }
        };
    }
    Ok(position)
}

/// [`Draft::set`] on `answers`, the answers to `schema`.
fn set_in(
    schema: &Schema,
    answers: &mut Config,
    path: &Path,
    answer: &Value,
) -> Result<(), ConfigError> {
    let position = locate(schema, answers, path)?;
    let answer = position.kind.walk(answer, path, false)?;
    let mut edited = answers.clone();
    edit(&mut edited, path, Some(answer));
    // a plugin put into a list repeats one of the list only next to the rest of it; the other
    // answers were checked when they were set, so this finds nothing else
    schema.walk(&edited, &Path::ROOT, false)?;
    *answers = edited;
    Ok(())
}

/// Answers the position at `path` in `answers` with `answer`, or takes its answer back if
/// `answer` is [`None`], see [`Draft::set`] and [`Draft::unset`]. `path` has been located.
fn edit(answers: &mut Config, path: &Path, answer: Option<Value>) {
    let (last, parent) = path
        .segments()
        .split_last()
        .expect("a located path is not the root");
    match last {
        Segment::Field(name) => {
            // the answers of a plugin are its answer's "value", which checking an answer writes
            // as an object
            let config = if parent.is_empty() {
                &mut answers.0
            } else {
                answers
                    .get_at_mut(parent)
                    .and_then(|plugin| plugin.get_mut("value"))
                    .and_then(Value::as_object_mut)
                    .expect("a located field is in the config of a plugin")
            };
            match answer {
                Some(answer) => config.insert(name.clone(), answer),
                None => config.remove(name),
            };
        }
        Segment::Index(index) => {
            let list = answers
                .get_at_mut(parent)
                .and_then(Value::as_array_mut)
                .expect("a located index is in a list");
            match answer {
                Some(answer) if *index == list.len() => list.push(answer),
                Some(answer) => list[*index] = answer,
                None => {
                    list.remove(*index);
                }
            }
        }
    }
}

/// One field of the tree of answers.
struct Question {
    path: Path,
    field: Field,
    answer: Option<Value>,
}

/// The answers as a tree: every field and every plugin answering one.
struct Tree {
    /// Every field, depth first in schema order.
    questions: Vec<Question>,
    /// Every plugin answering a field or a position in a list, depth first in schema order.
    plugins: Vec<Placed>,
}

/// A plugin's evaluated declaration and any construction error, at its answer's path.
struct Placed {
    path: Path,
    error: Option<ConfigError>,
    /// What the plugin declares itself, without the plugins answering its fields, which are
    /// placed in the tree themselves: what its configuration declares if it is built, and its
    /// static declaration otherwise. Named by its label and path.
    declared: Declared,
}

impl Tree {
    /// The tree of `answers`, the answers to `schema`.
    fn of(schema: &Schema, answers: &Config) -> Self {
        Self::with_plugins(schema, answers, Vec::new())
    }

    /// Inspects existing plugins instead of constructing them again. A dependency-ordered list
    /// can differ from answer order, so instances are matched by id within each parent's fields.
    fn with_plugins(schema: &Schema, answers: &Config, plugins: Vec<&dyn Plugin>) -> Self {
        let mut tree = Self {
            questions: Vec::new(),
            plugins: Vec::new(),
        };
        tree.add_fields(fields::fields(schema, answers, &Path::ROOT), plugins);
        tree
    }

    fn add_fields(&mut self, fields: Vec<FieldNode>, mut instances: Vec<&dyn Plugin>) {
        for node in fields {
            self.questions.push(Question {
                path: node.path,
                field: node.field,
                answer: node.answer,
            });
            for plugin in node.plugins {
                let owned;
                let instance = match instances
                    .iter()
                    .position(|instance| instance.id() == plugin.entry.id())
                {
                    Some(index) => Ok(instances.remove(index)),
                    None => {
                        owned = plugin.entry.instance(&plugin.config, &plugin.path);
                        owned.as_deref().map_err(Clone::clone)
                    }
                };
                let children = instance
                    .as_ref()
                    .map(|instance| instance.sub_plugins())
                    .unwrap_or_default();
                self.plugins
                    .push(Placed::new(plugin.entry, plugin.path.clone(), instance));
                self.add_fields(plugin.fields, children);
            }
        }
    }

    /// The fields taking plugins among those `included` includes, each with its axis, in the order
    /// of [`Self::questions`].
    fn slots(&self, included: impl Fn(&Question) -> bool) -> Vec<(Path, AnyAxis)> {
        self.questions
            .iter()
            .filter(|question| included(question))
            .filter_map(|question| Some((question.path.clone(), slot(question.field.kind())?.0)))
            .collect()
    }

    /// What the plugins declare, in the order of [`Self::plugins`].
    fn declarations(&self) -> Vec<&Declared> {
        self.plugins.iter().map(|placed| &placed.declared).collect()
    }
}

/// The fields among `fields` whose axis could provide `fact`.
fn able(fields: &[(Path, AnyAxis)], fact: TypeId) -> impl Iterator<Item = &(Path, AnyAxis)> {
    fields
        .iter()
        .filter(move |(_, axis)| axis.facts().contains(fact))
}

/// Every one of `items` but the one at `index`.
fn all_but<T>(items: &[T], index: usize) -> impl Iterator<Item = &T> {
    items
        .iter()
        .enumerate()
        .filter(move |&(other, _)| other != index)
        .map(|(_, item)| item)
}

impl Placed {
    fn new(
        entry: &'static PluginEntry,
        path: Path,
        instance: Result<&dyn Plugin, ConfigError>,
    ) -> Self {
        let mut declaration = entry.declaration();
        if let Ok(instance) = &instance {
            instance.declare(&mut declaration);
        }
        let declared = Declared::new(format!("{} ({path})", entry.label()), declaration);
        Self {
            path,
            error: instance.err(),
            declared,
        }
    }
}

/// The draft, tested on axes of its own so that it does not depend on what the plugins of a
/// program happen to register.
#[cfg(test)]
mod tests {
    use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
    use serde_json::json;
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::*;
    use crate::{Plugin, PluginType, axis, compatibility::Fact, register};

    #[derive(Debug)]
    struct Points;

    impl Fact for Points {}

    #[derive(Debug)]
    struct Matches;

    impl Fact for Matches {}

    #[derive(Debug)]
    struct Eliminating;

    impl Fact for Eliminating {}

    /// How many entrants a ranking stands for, the fact the context of the composite provides.
    #[derive(Debug)]
    struct EntrantsPerRanking(u32);

    impl Fact for EntrantsPerRanking {}

    /// How the matches of a phase are scheduled.
    #[axis]
    trait Format: Plugin {}

    /// What is calculated for an entrant.
    #[axis]
    trait Metric: Plugin {}

    /// A format with no settings or declarations.
    #[derive(Debug, Serialize, Deserialize, Plugin)]
    #[plugin(
        id = "round_robin",
        label = "round_robin",
        description = "a plugin of the draft's tests"
    )]
    struct RoundRobin;

    #[register]
    impl Format for RoundRobin {}

    /// A format with a parameter, providing a fact no metric provides.
    #[derive(Debug, Serialize, Deserialize, Plugin)]
    #[plugin(
        id = "knockout",
        label = "knockout",
        description = "a plugin of the draft's tests",
        declare_static = Self::declare_static
    )]
    struct Knockout {
        #[plugin(label = "Third place", default)]
        third_place: bool,
    }

    impl Knockout {
        fn declare_static(declaration: &mut Declaration) {
            declaration.provide(Eliminating);
        }
    }

    #[register]
    impl Format for Knockout {}

    /// A format providing a fact the context provides.
    #[derive(Debug, Serialize, Deserialize, Plugin)]
    #[plugin(
        id = "doubles",
        label = "doubles",
        description = "a plugin of the draft's tests",
        declare_static = Self::declare_static
    )]
    struct Doubles;

    impl Doubles {
        fn declare_static(declaration: &mut Declaration) {
            declaration.provide(EntrantsPerRanking(2));
        }
    }

    #[register]
    impl Format for Doubles {}

    /// A format with a condition on the fact the context provides.
    #[derive(Debug, Serialize, Deserialize, Plugin)]
    #[plugin(
        id = "monster",
        label = "monster",
        description = "a plugin of the draft's tests",
        declare_static = Self::declare_static
    )]
    struct Monster;

    impl Monster {
        fn declare_static(declaration: &mut Declaration) {
            declaration.require_that::<EntrantsPerRanking>(
                "the entrants per ranking",
                "rankings of single players",
                |entrants| entrants.0 == 1,
            );
        }
    }

    #[register]
    impl Format for Monster {}

    #[derive(Debug, Serialize, Deserialize, Plugin)]
    #[plugin(
        id = "match_count",
        label = "match_count",
        description = "a plugin of the draft's tests",
        declare_static = Self::declare_static
    )]
    struct MatchCount;

    impl MatchCount {
        fn declare_static(declaration: &mut Declaration) {
            declaration.provide(Matches);
        }
    }

    #[register]
    impl Metric for MatchCount {}

    /// A metric with a parameter it validates.
    #[derive(Debug, Serialize, Deserialize, Plugin)]
    #[plugin(
        id = "wins",
        label = "wins",
        description = "a plugin of the draft's tests",
        declare_static = Self::declare_static
    )]
    struct Wins {
        #[serde(deserialize_with = "non_negative")]
        #[plugin(label = "Points for a win", default = 1.0)]
        for_win: f64,
    }

    impl Wins {
        fn declare_static(declaration: &mut Declaration) {
            declaration.provide(Points);
        }
    }

    fn non_negative<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
        let points = f64::deserialize(deserializer)?;
        if points < 0.0 {
            return Err(D::Error::custom("the points for a win cannot be negative"));
        }
        Ok(points)
    }

    #[register]
    impl Metric for Wins {}

    /// A metric providing what [`Wins`] provides.
    #[derive(Debug, Serialize, Deserialize, Plugin)]
    #[plugin(
        id = "score",
        label = "score",
        description = "a plugin of the draft's tests",
        declare_static = Self::declare_static
    )]
    struct Score;

    impl Score {
        fn declare_static(declaration: &mut Declaration) {
            declaration.provide(Points);
        }
    }

    #[register]
    impl Metric for Score {}

    /// A metric requiring what other metrics provide.
    #[derive(Debug, Serialize, Deserialize, Plugin)]
    #[plugin(
        id = "buchholz",
        label = "buchholz",
        description = "a plugin of the draft's tests",
        declare_static = Self::declare_static
    )]
    struct Buchholz;

    impl Buchholz {
        fn declare_static(declaration: &mut Declaration) {
            declaration
                .require::<Points>("points")
                .require::<Matches>("matches");
        }
    }

    #[register]
    impl Metric for Buchholz {}

    /// A metric requiring what only a format provides.
    #[derive(Debug, Serialize, Deserialize, Plugin)]
    #[plugin(
        id = "losses",
        label = "losses",
        description = "a plugin of the draft's tests",
        declare_static = Self::declare_static
    )]
    struct Losses;

    impl Losses {
        fn declare_static(declaration: &mut Declaration) {
            declaration.require::<Eliminating>("eliminations");
        }
    }

    #[register]
    impl Metric for Losses {}

    /// A metric with a field that does not matter in an elimination.
    #[derive(Debug, Serialize, Deserialize, Plugin)]
    #[plugin(
        id = "draws",
        label = "draws",
        description = "a plugin of the draft's tests"
    )]
    struct Draws {
        #[plugin(label = "Points for a draw", default = 0.5, irrelevant_with = Eliminating)]
        for_draw: f64,
    }

    #[register]
    impl Metric for Draws {}

    /// A metric made of metrics, so that plugins nest.
    #[derive(Debug, Serialize, Deserialize, Plugin)]
    #[plugin(
        id = "combined",
        label = "combined",
        description = "a metric made of metrics"
    )]
    struct Combined {
        #[plugin(label = "Parts", default)]
        parts: Vec<Box<dyn Metric>>,
    }

    #[register]
    impl Metric for Combined {}

    /// The composite: one format, any number of metrics, and a metric or none.
    #[derive(Debug, Serialize, Deserialize, Configurable)]
    struct Phase {
        #[plugin(label = "Format")]
        format: Box<dyn Format>,
        #[plugin(label = "Metrics", default)]
        metrics: Vec<Box<dyn Metric>>,
        #[serde(deserialize_with = "Option::deserialize")]
        #[plugin(label = "Tie break", default)]
        tie_break: Option<Box<dyn Metric>>,
    }

    /// The context of a phase of single players.
    fn singles(declaration: &mut Declaration) {
        declaration.provide(EntrantsPerRanking(1));
    }

    /// A draft of a phase of single players.
    fn draft() -> Draft<Phase> {
        Draft::new(singles)
    }

    /// The path written as `text`, e.g. `metrics[1].for_win`.
    fn path(text: &str) -> Path {
        text.parse().unwrap()
    }

    /// The plugin `P` with nothing answered.
    fn plugin<P: PluginType>() -> PluginConfig {
        PluginConfig::of::<P>()
    }

    /// A draft with `answers` set, each at the path written as its first element.
    fn answered(answers: &[(&str, Value)]) -> Draft<Phase> {
        let mut draft = draft();
        for (at, answer) in answers {
            draft.set(&path(at), answer.clone()).unwrap();
        }
        draft
    }

    /// A field is irrelevant while the composite provides the fact its schema names, here the
    /// eliminations a knockout provides; it stays answered all the same.
    #[test]
    fn a_field_is_irrelevant_while_its_fact_is_provided() {
        let mut draft = answered(&[
            ("format", json!(plugin::<RoundRobin>())),
            ("metrics", json!([plugin::<Draws>().with("for_draw", 0.5)])),
        ]);
        let for_draw = path("metrics[0].for_draw");
        assert!(draft.is_relevant(&for_draw).unwrap());
        assert!(
            draft.is_relevant(&path("metrics[0]")).unwrap(),
            "a position"
        );

        draft
            .set(
                &path("format"),
                json!(plugin::<Knockout>().with("third_place", false)),
            )
            .unwrap();

        assert!(!draft.is_relevant(&for_draw).unwrap());
        assert!(draft.is_relevant(&path("format")).unwrap());
        assert_eq!(
            draft.answers().get_at(&for_draw),
            Some(&json!(0.5)),
            "still answered"
        );
        assert!(matches!(
            draft.is_relevant(&path("metrics[1].for_draw")),
            Err(ConfigError::UnknownField { .. })
        ));
    }

    /// The registered choices at `at`, by id.
    fn candidates(draft: &Draft<Phase>, at: &str) -> Vec<&'static str> {
        draft
            .candidates(&path(at))
            .unwrap()
            .into_iter()
            .map(PluginEntry::id)
            .collect()
    }

    /// The paths of the fields in the tree of the answers of `draft`, each with whether it is
    /// answered.
    fn paths<T: Configurable>(draft: &Draft<T>) -> Vec<(String, bool)> {
        Tree::of(&T::schema(), draft.answers())
            .questions
            .iter()
            .map(|question| (question.path.to_string(), question.answer.is_some()))
            .collect()
    }

    #[test]
    fn the_tree_holds_the_fields_of_the_composite_and_of_the_plugins_answering_them() {
        let mut draft = draft();

        assert_eq!(
            paths(&draft),
            [
                ("format".to_owned(), false),
                ("metrics".to_owned(), false),
                ("tie_break".to_owned(), false),
            ]
        );
        draft.set(&path("format"), plugin::<Knockout>()).unwrap();
        assert_eq!(
            paths(&draft),
            [
                ("format".to_owned(), true),
                ("format.third_place".to_owned(), false),
                ("metrics".to_owned(), false),
                ("tie_break".to_owned(), false),
            ]
        );
    }

    #[test]
    fn enumerated_fields_carry_the_answers_and_nested_plugins() {
        let draft = draft()
            .with(
                "metrics",
                vec![plugin::<Combined>().with("parts", vec![plugin::<Wins>().with("for_win", 2)])],
            )
            .unwrap()
            .with("tie_break", plugin::<Wins>())
            .unwrap();

        let fields = draft.fields();
        assert_eq!(fields[0].field.name(), "format");
        assert_eq!(fields[0].answer, None);
        assert!(fields[0].plugins.is_empty());

        let combined = &fields[1].plugins[0];
        assert_eq!(combined.entry.id(), Combined::ID);
        let wins = &combined.fields[0].plugins[0];
        assert_eq!(wins.entry.id(), Wins::ID);
        assert_eq!(wins.fields[0].path, path("metrics[0].parts[0].for_win"));
        assert_eq!(wins.fields[0].answer, Some(json!(2)));
        assert!(wins.fields[0].plugins.is_empty());

        let tie_break = &fields[2].plugins[0];
        assert_eq!(tie_break.entry.id(), Wins::ID);
        assert_eq!(tie_break.fields[0].path, path("tie_break.for_win"));
        assert_eq!(tie_break.fields[0].answer, None);
    }

    #[test]
    fn enumeration_after_edits_uses_the_current_plugin_and_list_positions() {
        let mut draft = draft()
            .with("format", plugin::<Knockout>())
            .unwrap()
            .with("metrics", vec![plugin::<Wins>(), plugin::<Combined>()])
            .unwrap();
        let before = draft.fields();

        draft.unset(&before[1].plugins[0].path).unwrap();
        draft.set(&before[0].path, plugin::<RoundRobin>()).unwrap();
        draft.set(&path("tie_break"), Value::Null).unwrap();

        let after = draft.fields();
        assert_eq!(after[0].plugins[0].entry.id(), RoundRobin::ID);
        assert!(after[0].plugins[0].fields.is_empty());
        assert_eq!(after[1].plugins.len(), 1);
        assert_eq!(after[1].plugins[0].entry.id(), Combined::ID);
        assert_eq!(after[1].plugins[0].fields[0].path, path("metrics[0].parts"));
        assert_eq!(after[2].answer, Some(Value::Null));
        assert!(after[2].plugins.is_empty());
        // the previous enumeration remains a snapshot, not another owner of the draft's answers
        assert_eq!(before[1].plugins[0].entry.id(), Wins::ID);
    }

    #[test]
    fn an_answer_is_set_at_its_path_and_unset_again() {
        let mut draft = answered(&[("metrics", json!([plugin::<Wins>()]))]);

        draft.set(&path("metrics[0].for_win"), 2).unwrap();
        draft
            .set(&path("metrics[1]"), plugin::<MatchCount>())
            .unwrap();
        assert_eq!(
            draft.answers().get("metrics"),
            Some(&json!([
                {"type": "wins", "value": {"for_win": 2}},
                {"type": "match_count", "value": {}},
            ]))
        );

        draft.unset(&path("metrics[0]")).unwrap();
        assert_eq!(
            draft.answers().get("metrics"),
            Some(&json!([{"type": "match_count", "value": {}}]))
        );
        draft.unset(&path("metrics")).unwrap();
        assert_eq!(draft.answers().get("metrics"), None);
    }

    /// The answers of a draft are taken over by another one, as they are written back after being
    /// checked.
    #[test]
    fn a_draft_takes_over_the_answers_of_another_one() {
        let filled = answered(&[
            (
                "format",
                json!(plugin::<Knockout>().with("third_place", true)),
            ),
            ("metrics", json!([plugin::<MatchCount>()])),
            ("tie_break", Value::Null),
        ]);

        let draft = draft().with_answers(filled.answers()).unwrap();

        assert_eq!(draft.answers(), filled.answers());
        assert!(draft.build().is_ok());
    }

    /// A composite serializes as the answers it is built from, so that it can be changed in a
    /// draft once it is built.
    #[test]
    fn a_built_composite_has_the_config_it_was_built_from() {
        let draft = answered(&[
            (
                "format",
                json!(plugin::<Knockout>().with("third_place", true)),
            ),
            ("metrics", json!([plugin::<Wins>().with("for_win", 2.0)])),
            ("tie_break", json!(plugin::<MatchCount>())),
        ]);

        let built = draft.build().unwrap();

        assert_eq!(Config::of(&built).as_ref(), Ok(draft.answers()));
    }

    #[test]
    fn answers_that_do_not_fit_the_schema_are_not_taken_over() {
        let answers =
            Config::new().with("metrics", json!([plugin::<Wins>().with("for_win", true)]));

        assert_eq!(
            draft().with_answers(&answers).err(),
            Some(ConfigError::WrongKind {
                path: path("metrics[0].for_win"),
                expected: "a number",
                found: "yes or no",
            })
        );
    }

    #[test]
    fn a_path_addressing_nothing_is_an_error() {
        let mut draft = answered(&[("metrics", json!([]))]);

        for at in ["format.third_place", "metrics[1]", "rounds", "tie_break[0]"] {
            assert_eq!(
                draft.set(&path(at), 1),
                Err(ConfigError::UnknownField { path: path(at) }),
                "{at}"
            );
        }
        assert_eq!(
            draft.unset(&path("metrics[0]")),
            Err(ConfigError::UnknownField {
                path: path("metrics[0]"),
            })
        );
    }

    #[test]
    fn an_answer_that_is_not_one_its_field_takes_is_not_set() {
        let mut draft = draft();

        assert_eq!(
            draft.set(&path("metrics"), 1),
            Err(ConfigError::WrongKind {
                path: path("metrics"),
                expected: "a list",
                found: "a whole number",
            })
        );
        assert_eq!(
            draft.set(
                &path("metrics"),
                json!([plugin::<Wins>().with("for_win", true)])
            ),
            Err(ConfigError::WrongKind {
                path: path("metrics[0].for_win"),
                expected: "a number",
                found: "yes or no",
            })
        );
        assert_eq!(
            draft.set(&path("format"), plugin::<Wins>()),
            Err(ConfigError::UnknownPlugin {
                path: path("format"),
            })
        );
        assert_eq!(draft.answers(), &Config::new());
    }

    #[test]
    fn a_plugin_already_in_a_list_is_not_set_into_it_again() {
        let mut draft = answered(&[("metrics", json!([plugin::<Wins>(), plugin::<Buchholz>()]))]);
        let before = draft.answers().clone();
        let duplicate = |at: &str| Err(ConfigError::Duplicate { path: path(at) });
        let wins = || plugin::<Wins>().with("for_win", 2);

        // whatever it is configured with, appended or in another position
        assert_eq!(
            draft.set(&path("metrics[2]"), wins()),
            duplicate("metrics[2]")
        );
        assert_eq!(
            draft.set(&path("metrics[1]"), wins()),
            duplicate("metrics[1]")
        );
        assert_eq!(
            draft.set(&path("metrics"), json!([wins(), wins()])),
            duplicate("metrics[1]")
        );
        assert_eq!(draft.answers(), &before);

        // a plugin replacing itself is no repeat
        draft.set(&path("metrics[0]"), wins()).unwrap();
        assert_eq!(
            draft.answers().get("metrics"),
            Some(&json!([wins(), plugin::<Buchholz>()]))
        );
    }

    #[test]
    fn choices_include_plugins_conflicting_with_the_context() {
        assert_eq!(
            candidates(&draft(), "format"),
            ["doubles", "knockout", "monster", "round_robin"]
        );
    }

    #[test]
    fn choices_include_alternative_providers_of_an_already_provided_fact() {
        let draft = answered(&[
            ("format", plugin::<RoundRobin>().into()),
            ("metrics", json!([plugin::<Wins>()])),
            ("tie_break", Value::Null),
        ]);

        for at in ["metrics[0]", "metrics", "metrics[1]"] {
            assert!(candidates(&draft, at).contains(&"score"));
        }
    }

    #[test]
    fn a_plugin_answering_another_position_of_a_list_is_no_candidate_there() {
        let draft = answered(&[
            ("metrics", json!([plugin::<Buchholz>(), plugin::<Wins>()])),
            (
                "tie_break",
                plugin::<Combined>().with("parts", json!([])).into(),
            ),
        ]);
        let ids = |at| candidates(&draft, at);
        let all = [
            "buchholz",
            "combined",
            "draws",
            "losses",
            "match_count",
            "score",
            "wins",
        ];

        // the wins are left out whatever they are configured with
        assert_eq!(
            ids("metrics[2]"),
            ["combined", "draws", "losses", "match_count", "score"]
        );
        // what answers the position itself stays a candidate, and so does what answers another
        // field
        assert_eq!(
            ids("metrics[0]"),
            [
                "buchholz",
                "combined",
                "draws",
                "losses",
                "match_count",
                "score"
            ]
        );
        assert_eq!(ids("metrics"), all);
        assert_eq!(ids("tie_break.parts[0]"), all);
    }

    #[test]
    fn choices_include_plugins_with_missing_requirements() {
        let draft = answered(&[
            ("format", plugin::<RoundRobin>().into()),
            ("tie_break", Value::Null),
        ]);

        assert!(candidates(&draft, "metrics").contains(&"losses"));
    }

    #[test]
    fn a_position_taking_no_plugins_has_no_candidates() {
        let draft = answered(&[("format", plugin::<Knockout>().into())]);

        assert!(candidates(&draft, "format.third_place").is_empty());
    }

    #[test]
    fn nested_plugins_are_addressed_through_the_fields_they_answer() {
        let mut draft = answered(&[
            ("format", plugin::<RoundRobin>().into()),
            ("metrics", json!([plugin::<Combined>()])),
        ]);

        assert!(paths(&draft).contains(&("metrics[0].parts".to_owned(), false)));
        draft
            .set(&path("metrics[0].parts"), json!([plugin::<Wins>()]))
            .unwrap();
        assert!(candidates(&draft, "metrics[0].parts[1]").contains(&"score"));
    }

    #[test]
    fn an_answer_a_plugin_rejects_is_an_issue_at_its_field() {
        let draft = answered(&[("metrics", json!([plugin::<Wins>().with("for_win", -1.0)]))]);
        let rejected = ConfigError::Rejected {
            path: path("metrics[0].for_win"),
            reason: "the points for a win cannot be negative".to_owned(),
        };

        assert_eq!(
            draft.issues(),
            [Issue {
                path: path("metrics[0].for_win"),
                problem: Problem::Invalid(rejected),
            }]
        );
    }

    #[test]
    fn plugins_providing_the_same_fact_are_issues_at_each_of_them() {
        let draft = answered(&[
            ("format", plugin::<RoundRobin>().into()),
            ("metrics", json!([plugin::<Wins>(), plugin::<Score>()])),
            ("tie_break", Value::Null),
        ]);
        let twice = |first: &str, second: &str| {
            Problem::Incompatible(Incompatibility::FactProvidedTwice {
                fact: format!("{Points:?}"),
                first: first.to_owned(),
                second: second.to_owned(),
            })
        };

        assert_eq!(
            draft.issues(),
            [
                Issue {
                    path: path("metrics[0]"),
                    problem: twice("score (metrics[1])", "wins (metrics[0])"),
                },
                Issue {
                    path: path("metrics[1]"),
                    problem: twice("wins (metrics[0])", "score (metrics[1])"),
                },
            ]
        );
    }

    #[test]
    fn a_fact_nothing_provides_is_an_issue_at_each_field_that_could_provide_it() {
        let losing = answered(&[
            ("format", plugin::<RoundRobin>().into()),
            ("metrics", json!([plugin::<Losses>()])),
            ("tie_break", Value::Null),
        ]);
        let counting = answered(&[
            ("format", plugin::<RoundRobin>().into()),
            ("metrics", json!([plugin::<Buchholz>(), plugin::<Wins>()])),
            ("tie_break", Value::Null),
        ]);
        let unprovided = |fact, providers: &[&'static str]| Problem::Unprovided {
            fact,
            required_by: path("metrics[0]"),
            providers: providers.to_vec(),
        };

        assert_eq!(
            losing.issues(),
            [Issue {
                path: path("format"),
                problem: unprovided("eliminations", &["knockout"]),
            }]
        );
        assert_eq!(
            counting.issues(),
            [
                Issue {
                    path: path("metrics"),
                    problem: unprovided("matches", &["match_count"]),
                },
                Issue {
                    path: path("tie_break"),
                    problem: unprovided("matches", &["match_count"]),
                },
            ]
        );
    }

    #[test]
    fn a_fact_an_unanswered_field_could_provide_is_no_issue() {
        let draft = answered(&[("metrics", json!([plugin::<Losses>()]))]);

        assert_eq!(draft.issues(), []);
    }

    #[test]
    fn a_field_offers_the_default_of_its_schema() {
        let draft = answered(&[("format", plugin::<Knockout>().into())]);

        assert_eq!(
            draft.default_value(&path("format.third_place")),
            Ok(Some(json!(false)))
        );
        assert_eq!(draft.default_value(&path("metrics")), Ok(Some(json!([]))));
        assert_eq!(draft.default_value(&path("format")), Ok(None));
    }

    /// Answers every unanswered field of `draft` that offers an answer with it, depth first in
    /// schema order, as a UI accepting every default does.
    fn accept_defaults(draft: &mut Draft<Phase>) {
        while let Some(at) = Tree::of(&Phase::schema(), draft.answers())
            .questions
            .into_iter()
            .find(|question| {
                question.answer.is_none() && draft.default_value(&question.path) != Ok(None)
            })
            .map(|question| question.path)
        {
            let default = draft.default_value(&at).unwrap().unwrap();
            draft.set(&at, default).unwrap();
        }
    }

    #[test]
    fn building_fills_in_no_default() {
        let mut draft = answered(&[("format", plugin::<RoundRobin>().into())]);
        let unanswered = |at: &str| Issue {
            path: path(at),
            problem: Problem::Invalid(ConfigError::Unanswered { path: path(at) }),
        };

        assert_eq!(
            draft.build().err(),
            Some(vec![unanswered("metrics"), unanswered("tie_break")])
        );
        accept_defaults(&mut draft);
        assert_eq!(
            draft.build().map(|phase| format!("{phase:?}")),
            Ok(format!(
                "{:?}",
                Phase {
                    format: Box::new(RoundRobin),
                    metrics: vec![],
                    tie_break: None,
                }
            ))
        );
    }

    #[test]
    fn building_with_an_unanswered_field_fails() {
        let unanswered = |at: &str| Issue {
            path: path(at),
            problem: Problem::Invalid(ConfigError::Unanswered { path: path(at) }),
        };

        assert_eq!(
            draft().build().err(),
            Some(vec![
                unanswered("format"),
                unanswered("metrics"),
                unanswered("tie_break"),
            ])
        );
    }

    #[test]
    fn a_composite_that_is_a_unit_struct_is_built() {
        /// A composite asking for nothing.
        #[derive(Debug, Deserialize, Configurable)]
        struct Nothing;

        assert!(Draft::<Nothing>::new(|_| {}).build().is_ok());
    }

    #[test]
    fn building_with_an_answer_a_plugin_rejects_fails() {
        let draft = answered(&[
            ("format", plugin::<RoundRobin>().into()),
            ("metrics", json!([plugin::<Wins>().with("for_win", -1.0)])),
            ("tie_break", Value::Null),
        ]);

        assert_eq!(
            draft.build().err(),
            Some(vec![Issue {
                path: path("metrics[0].for_win"),
                problem: Problem::Invalid(ConfigError::Rejected {
                    path: path("metrics[0].for_win"),
                    reason: "the points for a win cannot be negative".to_owned(),
                }),
            }])
        );
    }

    #[test]
    fn building_reports_with_nothing_open() {
        let draft = answered(&[
            ("metrics", json!([plugin::<Losses>()])),
            ("tie_break", Value::Null),
        ]);

        // the unanswered format could still provide the eliminations
        assert_eq!(draft.issues(), []);
        assert_eq!(
            draft.build().err(),
            Some(vec![
                Issue {
                    path: path("format"),
                    problem: Problem::Invalid(ConfigError::Unanswered {
                        path: path("format"),
                    }),
                },
                Issue {
                    path: path("format"),
                    problem: Problem::Unprovided {
                        fact: "eliminations",
                        required_by: path("metrics[0]"),
                        providers: vec!["knockout"],
                    },
                },
            ])
        );
    }

    #[test]
    fn the_implied_plugins_are_added_to_the_answers_and_not_by_building() {
        let mut draft = answered(&[
            (
                "format",
                plugin::<Knockout>().with("third_place", false).into(),
            ),
            ("metrics", json!([plugin::<Buchholz>()])),
            ("tie_break", Value::Null),
        ]);
        let issues = draft.build().expect_err("building adds no plugin");
        let unprovided: Vec<_> = issues
            .iter()
            .filter_map(|issue| match issue.problem() {
                Problem::Unprovided { fact, .. } => Some((issue.path().to_string(), *fact)),
                _ => None,
            })
            .collect();
        assert_eq!(
            unprovided,
            [
                ("metrics".to_owned(), "points"),
                ("tie_break".to_owned(), "points"),
                ("metrics".to_owned(), "matches"),
                ("tie_break".to_owned(), "matches"),
            ]
        );

        draft.add_implied();

        // wins has a field, so score is the only plugin providing the points that is added
        assert_eq!(
            draft.answers().get("metrics"),
            Some(&json!([
                plugin::<Buchholz>(),
                plugin::<Score>(),
                plugin::<MatchCount>()
            ]))
        );
        assert_eq!(
            draft.build().map(|phase| format!("{:?}", phase.metrics)),
            Ok("[Buchholz, Score, MatchCount]".to_owned())
        );
    }

    #[test]
    fn a_composite_answered_in_code_is_built() -> Result<(), ConfigError> {
        let draft = draft()
            .with("format", plugin::<Knockout>().with("third_place", true))?
            .with(
                "metrics",
                json!([
                    plugin::<Buchholz>(),
                    plugin::<Score>(),
                    plugin::<MatchCount>()
                ]),
            )?
            .with("tie_break", Value::Null)?;

        assert_eq!(
            draft.build().map(|phase| format!("{phase:?}")),
            Ok(format!(
                "{:?}",
                Phase {
                    format: Box::new(Knockout { third_place: true }),
                    metrics: vec![Box::new(Buchholz), Box::new(Score), Box::new(MatchCount)],
                    tie_break: None,
                }
            ))
        );
        Ok(())
    }

    #[test]
    fn answering_a_field_in_code_is_checked_as_setting_it() {
        assert_eq!(
            draft().with("metrics", 1).err(),
            Some(ConfigError::WrongKind {
                path: path("metrics"),
                expected: "a list",
                found: "a whole number",
            })
        );
        assert_eq!(
            draft().with("rounds", 1).err(),
            Some(ConfigError::UnknownField {
                path: path("rounds")
            })
        );
    }

    #[test]
    fn the_defaults_do_not_depend_on_other_answers() -> Result<(), ConfigError> {
        let draft = draft()
            .with("format", plugin::<RoundRobin>())?
            .with_defaults()?;

        assert_eq!(
            Value::from(draft.answers().clone()),
            json!({
                "format": {"type": "round_robin", "value": {}},
                "metrics": [],
                "tie_break": null,
            })
        );
        Ok(())
    }

    #[test]
    fn the_defaults_keep_the_answers_and_leave_what_offers_nothing() -> Result<(), ConfigError> {
        let draft = draft()
            .with("metrics", json!([plugin::<Wins>().with("for_win", 3.0)]))?
            .with_defaults()?;

        assert_eq!(
            Value::from(draft.answers().clone()),
            json!({
                "metrics": [{"type": "wins", "value": {"for_win": 3.0}}],
                "tie_break": null,
            })
        );
        Ok(())
    }

    /// Implied plugins, on axes of their own so that what the answers imply is not changed by the
    /// plugins of the other tests.
    /// A fact nothing provides yet only counts as still to arrive while a plugin providing it
    /// could still be added, tested on axes of their own: a criterion requiring rounds of
    /// elimination, which only a calculator requiring a format that eliminates calculates.
    mod partial_validation {
        #[cfg(target_arch = "wasm32")]
        use wasm_bindgen_test::wasm_bindgen_test as test;

        use super::*;

        #[derive(Debug)]
        struct EliminationRounds;

        impl Fact for EliminationRounds {}

        #[axis]
        trait Schedule: Plugin {}

        #[axis]
        trait Criterion: Plugin {}

        #[axis]
        trait Calculator: Plugin {}

        /// A format eliminating nobody.
        #[derive(Debug, Serialize, Deserialize, Plugin)]
        #[plugin(id = "everyone", label = "everyone")]
        struct Everyone;

        #[register]
        impl Schedule for Everyone {}

        #[derive(Debug, Serialize, Deserialize, Plugin)]
        #[plugin(
            id = "eliminating",
            label = "eliminating",
            declare_static = Self::declare_static
        )]
        struct EliminatingSchedule;

        impl EliminatingSchedule {
            fn declare_static(declaration: &mut Declaration) {
                declaration.provide(Eliminating);
            }
        }

        #[register]
        impl Schedule for EliminatingSchedule {}

        /// The only calculator of the elimination rounds, which needs a format that eliminates.
        #[derive(Debug, Serialize, Deserialize, Plugin)]
        #[plugin(
            id = "elimination_rounds",
            label = "elimination rounds",
            declare_static = Self::declare_static
        )]
        struct EliminationRoundsCalculator;

        impl EliminationRoundsCalculator {
            fn declare_static(declaration: &mut Declaration) {
                declaration
                    .provide(EliminationRounds)
                    .require::<Eliminating>("eliminations");
            }
        }

        #[register]
        impl Calculator for EliminationRoundsCalculator {}

        #[derive(Debug, Serialize, Deserialize, Plugin)]
        #[plugin(
            id = "later_elimination",
            label = "later elimination",
            declare_static = Self::declare_static
        )]
        struct LaterElimination;

        impl LaterElimination {
            fn declare_static(declaration: &mut Declaration) {
                declaration.require::<EliminationRounds>("the elimination rounds");
            }
        }

        #[register]
        impl Criterion for LaterElimination {}

        #[derive(Debug, Serialize, Deserialize, Configurable)]
        struct Ranked {
            #[plugin(label = "format")]
            format: Box<dyn Schedule>,
            #[plugin(label = "ranked by")]
            ranking: Vec<Box<dyn Criterion>>,
            #[plugin(label = "metrics")]
            metrics: Vec<Box<dyn Calculator>>,
        }

        /// A draft with an empty ranking and the format `format`, if any.
        fn ranked(format: Option<PluginConfig>) -> Draft<Ranked> {
            let mut draft = Draft::new(|_| {});
            draft.set(&path("ranking"), json!([])).unwrap();
            if let Some(format) = format {
                draft.set(&path("format"), format).unwrap();
            }
            draft
        }

        #[test]
        fn choices_do_not_depend_on_the_feasibility_of_future_dependencies() {
            for format in [
                None,
                Some(plugin::<Everyone>()),
                Some(plugin::<EliminatingSchedule>()),
            ] {
                let draft = ranked(format);
                let ids: Vec<_> = draft
                    .candidates(&path("ranking[0]"))
                    .unwrap()
                    .into_iter()
                    .map(PluginEntry::id)
                    .collect();
                assert_eq!(ids, [LaterElimination::ID]);
            }
        }

        #[test]
        fn an_unanswered_field_postpones_missing_facts_without_predicting_a_completion() {
            let unprovided = |format| {
                let mut draft = ranked(Some(format));
                draft
                    .set(&path("ranking"), json!([plugin::<LaterElimination>()]))
                    .unwrap();
                draft
                    .issues()
                    .into_iter()
                    .filter(|issue| matches!(issue.problem(), Problem::Unprovided { .. }))
                    .map(|issue| issue.path().clone())
                    .collect::<Vec<_>>()
            };

            assert_eq!(unprovided(plugin::<Everyone>()), []);
            assert_eq!(unprovided(plugin::<EliminatingSchedule>()), []);
        }

        #[test]
        fn an_incompatible_selection_can_be_corrected_but_cannot_be_built() {
            let mut draft = ranked(Some(plugin::<Everyone>()));
            draft
                .set(&path("ranking"), vec![plugin::<LaterElimination>()])
                .unwrap();
            draft
                .set(
                    &path("metrics"),
                    vec![plugin::<EliminationRoundsCalculator>()],
                )
                .unwrap();
            assert!(!draft.issues().is_empty());
            assert!(draft.build().is_err());
            draft
                .set(&path("format"), plugin::<EliminatingSchedule>())
                .unwrap();
            assert!(draft.issues().is_empty());
            assert!(draft.build().is_ok());
        }
    }

    mod resolution {
        #[cfg(target_arch = "wasm32")]
        use wasm_bindgen_test::wasm_bindgen_test as test;

        use super::*;

        /// A fact that implies [`Implied`].
        #[derive(Debug)]
        struct Implying;

        impl Fact for Implying {}

        #[derive(Debug)]
        struct Ambiguous;

        impl Fact for Ambiguous {}

        #[derive(Debug)]
        struct Downstream;

        impl Fact for Downstream {}

        #[derive(Debug)]
        struct Deep;

        impl Fact for Deep {}

        #[derive(Debug)]
        struct Crosswise;

        impl Fact for Crosswise {}

        #[derive(Debug)]
        struct Lone;

        impl Fact for Lone {}

        #[derive(Debug)]
        struct Nested;

        impl Fact for Nested {}

        #[derive(Debug)]
        struct Stranded;

        impl Fact for Stranded {}

        #[derive(Debug)]
        struct Clash;

        impl Fact for Clash {}

        /// A fact [`Clashing`] provides, so that it clashes with a context providing it too.
        #[derive(Debug)]
        struct Given;

        impl Fact for Given {}

        /// A fact required before [`Opening`], whose provider needs what only the plugin
        /// [`Opening`] implies provides.
        #[derive(Debug)]
        struct Late;

        impl Fact for Late {}

        #[derive(Debug)]
        struct Opening;

        impl Fact for Opening {}

        /// A fact two plugins provide, so that it implies none.
        #[derive(Debug)]
        struct Unlocked;

        impl Fact for Unlocked {}

        /// The axis of [`Requiring`], taken by a single slot only.
        #[axis]
        trait Need: Plugin {}

        /// An axis taken by lists of the composites.
        #[axis]
        trait Supply: Plugin {}

        /// Another axis taken by a list of a composite.
        #[axis]
        trait Extra: Plugin {}

        /// An axis taken by a list in a plugin only.
        #[axis]
        trait Inner: Plugin {}

        /// A plugin requiring the facts whose fields are set, so that a composite holding it
        /// requires them.
        #[derive(Debug, Serialize, Deserialize, Plugin)]
        #[plugin(
            id = "requiring",
            label = "requiring",
            description = "a plugin requiring what it is told to",
            declare = Self::declare
        )]
        struct Requiring {
            #[serde(default)]
            #[plugin(label = "Implying")]
            implying: bool,
            #[serde(default)]
            #[plugin(label = "Ambiguous")]
            ambiguous: bool,
            #[serde(default)]
            #[plugin(label = "Downstream")]
            downstream: bool,
            #[serde(default)]
            #[plugin(label = "Deep")]
            deep: bool,
            #[serde(default)]
            #[plugin(label = "Crosswise")]
            crosswise: bool,
            #[serde(default)]
            #[plugin(label = "Lone")]
            lone: bool,
            #[serde(default)]
            #[plugin(label = "Nested")]
            nested: bool,
            #[serde(default)]
            #[plugin(label = "Stranded")]
            stranded: bool,
            #[serde(default)]
            #[plugin(label = "Clash")]
            clash: bool,
            #[serde(default)]
            #[plugin(label = "Late")]
            late: bool,
            #[serde(default)]
            #[plugin(label = "Opening")]
            opening: bool,
        }

        impl Requiring {
            fn declare(&self, declaration: &mut Declaration) {
                if self.implying {
                    declaration.require::<Implying>("the implying fact");
                }
                if self.ambiguous {
                    declaration.require::<Ambiguous>("the ambiguous fact");
                }
                if self.downstream {
                    declaration.require::<Downstream>("the downstream fact");
                }
                if self.deep {
                    declaration.require::<Deep>("the deep fact");
                }
                if self.crosswise {
                    declaration.require::<Crosswise>("the crosswise fact");
                }
                if self.lone {
                    declaration.require::<Lone>("the lone fact");
                }
                if self.nested {
                    declaration.require::<Nested>("the nested fact");
                }
                if self.stranded {
                    declaration.require::<Stranded>("the stranded fact");
                }
                if self.clash {
                    declaration.require::<Clash>("the clash fact");
                }
                // required in this order, so that the late fact is tried first
                if self.late {
                    declaration.require::<Late>("the late fact");
                }
                if self.opening {
                    declaration.require::<Opening>("the opening fact");
                }
            }
        }

        #[register]
        impl Need for Requiring {}

        /// The only plugin providing [`Lone`], of the axis only a single slot takes.
        #[derive(Debug, Serialize, Deserialize, Plugin)]
        #[plugin(
            id = "alone",
            label = "alone",
            description = "a plugin of the resolution tests",
            declare_static = Self::declare_static
        )]
        struct Alone;

        impl Alone {
            fn declare_static(declaration: &mut Declaration) {
                declaration.provide(Lone);
            }
        }

        #[register]
        impl Need for Alone {}

        /// The only plugin without fields that provides [`Implying`].
        #[derive(Debug, Serialize, Deserialize, Plugin)]
        #[plugin(
            id = "implied",
            label = "implied",
            description = "a plugin of the resolution tests",
            declare_static = Self::declare_static
        )]
        struct Implied;

        impl Implied {
            fn declare_static(declaration: &mut Declaration) {
                declaration.provide(Implying);
            }
        }

        #[register]
        impl Supply for Implied {}

        /// A plugin providing [`Implying`] that has to be configured.
        #[derive(Debug, Serialize, Deserialize, Plugin)]
        #[plugin(
            id = "configured_provider",
            label = "configured provider",
            description = "a plugin of the resolution tests",
            declare_static = Self::declare_static
        )]
        struct ConfiguredProvider {
            #[plugin(label = "Rounds")]
            rounds: u32,
        }

        impl ConfiguredProvider {
            fn declare_static(declaration: &mut Declaration) {
                declaration.provide(Implying);
            }
        }

        #[register]
        impl Supply for ConfiguredProvider {}

        fn provides_ambiguous(declaration: &mut Declaration) {
            declaration.provide(Ambiguous);
        }

        /// One of the two plugins providing [`Ambiguous`].
        #[derive(Debug, Serialize, Deserialize, Plugin)]
        #[plugin(
            id = "one_ambiguous",
            label = "one ambiguous",
            description = "a plugin of the resolution tests",
            declare_static = provides_ambiguous
        )]
        struct OneAmbiguous;

        #[register]
        impl Supply for OneAmbiguous {}

        /// The other plugin providing [`Ambiguous`].
        #[derive(Debug, Serialize, Deserialize, Plugin)]
        #[plugin(
            id = "other_ambiguous",
            label = "other ambiguous",
            description = "a plugin of the resolution tests",
            declare_static = provides_ambiguous
        )]
        struct OtherAmbiguous;

        #[register]
        impl Supply for OtherAmbiguous {}

        /// The only plugin providing [`Downstream`], requiring what [`Implied`] provides.
        #[derive(Debug, Serialize, Deserialize, Plugin)]
        #[plugin(
            id = "chaining",
            label = "chaining",
            description = "a plugin of the resolution tests",
            declare_static = Self::declare_static
        )]
        struct Chaining;

        impl Chaining {
            fn declare_static(declaration: &mut Declaration) {
                declaration
                    .provide(Downstream)
                    .require::<Implying>("the implying fact");
            }
        }

        #[register]
        impl Supply for Chaining {}

        /// The only plugin providing [`Stranded`], requiring what is ambiguous.
        #[derive(Debug, Serialize, Deserialize, Plugin)]
        #[plugin(
            id = "stranding",
            label = "stranding",
            description = "a plugin of the resolution tests",
            declare_static = Self::declare_static
        )]
        struct Stranding;

        impl Stranding {
            fn declare_static(declaration: &mut Declaration) {
                declaration
                    .provide(Stranded)
                    .require::<Ambiguous>("the ambiguous fact");
            }
        }

        #[register]
        impl Supply for Stranding {}

        /// The only plugin providing [`Clash`], providing [`Given`] as well.
        #[derive(Debug, Serialize, Deserialize, Plugin)]
        #[plugin(
            id = "clashing",
            label = "clashing",
            description = "a plugin of the resolution tests",
            declare_static = Self::declare_static
        )]
        struct Clashing;

        impl Clashing {
            fn declare_static(declaration: &mut Declaration) {
                declaration.provide(Clash).provide(Given);
            }
        }

        #[register]
        impl Supply for Clashing {}

        /// The only plugin providing [`Late`], requiring what implies no plugin on its own.
        #[derive(Debug, Serialize, Deserialize, Plugin)]
        #[plugin(
            id = "waiting",
            label = "waiting",
            description = "a plugin of the resolution tests",
            declare_static = Self::declare_static
        )]
        struct Waiting;

        impl Waiting {
            fn declare_static(declaration: &mut Declaration) {
                declaration
                    .provide(Late)
                    .require::<Unlocked>("the unlocked fact");
            }
        }

        #[register]
        impl Supply for Waiting {}

        /// The only plugin providing [`Opening`], and one of the two providing [`Unlocked`].
        #[derive(Debug, Serialize, Deserialize, Plugin)]
        #[plugin(
            id = "unlocking",
            label = "unlocking",
            description = "a plugin of the resolution tests",
            declare_static = Self::declare_static
        )]
        struct Unlocking;

        impl Unlocking {
            fn declare_static(declaration: &mut Declaration) {
                declaration.provide(Opening).provide(Unlocked);
            }
        }

        #[register]
        impl Supply for Unlocking {}

        /// The other plugin providing [`Unlocked`].
        #[derive(Debug, Serialize, Deserialize, Plugin)]
        #[plugin(
            id = "also_unlocking",
            label = "also unlocking",
            description = "a plugin of the resolution tests",
            declare_static = Self::declare_static
        )]
        struct AlsoUnlocking;

        impl AlsoUnlocking {
            fn declare_static(declaration: &mut Declaration) {
                declaration.provide(Unlocked);
            }
        }

        #[register]
        impl Supply for AlsoUnlocking {}

        /// A plugin providing [`Crosswise`], which [`Deepening`] of another axis provides too.
        #[derive(Debug, Serialize, Deserialize, Plugin)]
        #[plugin(
            id = "crossing",
            label = "crossing",
            description = "a plugin of the resolution tests",
            declare_static = Self::declare_static
        )]
        struct Crossing;

        impl Crossing {
            fn declare_static(declaration: &mut Declaration) {
                declaration.provide(Crosswise);
            }
        }

        #[register]
        impl Supply for Crossing {}

        /// A plugin holding a list of plugins, which is never added to.
        #[derive(Debug, Serialize, Deserialize, Plugin)]
        #[plugin(
            id = "bundle",
            label = "bundle",
            description = "a plugin of the resolution tests"
        )]
        struct Bundle {
            #[serde(default)]
            #[plugin(label = "Parts")]
            parts: Vec<Box<dyn Inner>>,
        }

        #[register]
        impl Supply for Bundle {}

        /// The only plugin providing [`Deep`], and one of the two providing [`Crosswise`].
        #[derive(Debug, Serialize, Deserialize, Plugin)]
        #[plugin(
            id = "deepening",
            label = "deepening",
            description = "a plugin of the resolution tests",
            declare_static = Self::declare_static
        )]
        struct Deepening;

        impl Deepening {
            fn declare_static(declaration: &mut Declaration) {
                declaration.provide(Deep).provide(Crosswise);
            }
        }

        #[register]
        impl Extra for Deepening {}

        /// The only plugin providing [`Nested`], of the axis only [`Bundle`] takes.
        #[derive(Debug, Serialize, Deserialize, Plugin)]
        #[plugin(
            id = "nesting",
            label = "nesting",
            description = "a plugin of the resolution tests",
            declare_static = Self::declare_static
        )]
        struct Nesting;

        impl Nesting {
            fn declare_static(declaration: &mut Declaration) {
                declaration.provide(Nested);
            }
        }

        #[register]
        impl Inner for Nesting {}

        /// A composite taking lists of two axes.
        #[derive(Debug, Serialize, Deserialize, Configurable)]
        struct Resolved {
            #[plugin(label = "Requirements")]
            requirements: Box<dyn Need>,
            #[serde(default)]
            #[plugin(label = "Supplies")]
            supplies: Vec<Box<dyn Supply>>,
            #[serde(default)]
            #[plugin(label = "Extras")]
            extras: Vec<Box<dyn Extra>>,
        }

        /// A composite taking the same axis in two lists.
        #[derive(Debug, Serialize, Deserialize, Configurable)]
        struct Twice {
            #[plugin(label = "Requirements")]
            requirements: Box<dyn Need>,
            #[serde(default)]
            #[plugin(label = "First")]
            first: Vec<Box<dyn Supply>>,
            #[serde(default)]
            #[plugin(label = "Second")]
            second: Vec<Box<dyn Supply>>,
        }

        /// The ids of the plugins in the lists of the composite `C` with `context`, its
        /// requirements requiring the facts named in `requires`, its lists empty and `answers`
        /// set, built once the implied plugins are added, or the facts it reports unprovided, each
        /// with the path it reports it at.
        ///
        /// Checks on the way that adding the implied plugins once more adds nothing.
        fn built<C: Configurable + Serialize>(
            context: impl FnOnce(&mut Declaration),
            requires: &[&str],
            answers: &[(&str, Value)],
        ) -> Result<Vec<String>, Vec<(String, &'static str)>> {
            let requiring = Requiring::schema()
                .fields()
                .iter()
                .fold(plugin::<Requiring>(), |requiring, field| {
                    requiring.with(field.name(), requires.contains(&field.name()))
                });
            let mut draft = Draft::<C>::new(context);
            draft.set(&path("requirements"), requiring).unwrap();
            for field in C::schema().fields() {
                if matches!(field.kind(), FieldKind::List { .. }) {
                    draft.set(&path(field.name()), json!([])).unwrap();
                }
            }
            for (at, answer) in answers {
                draft.set(&path(at), answer.clone()).unwrap();
            }
            draft.add_implied();
            let added = draft.answers().clone();
            draft.add_implied();
            assert_eq!(
                draft.answers(),
                &added,
                "adding the implied plugins once more adds nothing"
            );
            match draft.build() {
                Ok(built) => {
                    let built = serde_json::to_value(built).unwrap();
                    Ok(built
                        .as_object()
                        .unwrap()
                        .values()
                        .filter_map(Value::as_array)
                        .flatten()
                        .map(|plugin| plugin["type"].as_str().unwrap().to_owned())
                        .collect())
                }
                Err(issues) => Err(issues
                    .into_iter()
                    .map(|issue| match issue.problem {
                        Problem::Unprovided { fact, .. } => (issue.path.to_string(), fact),
                        problem => panic!("{problem:?} is not about a fact nothing provides"),
                    })
                    .collect()),
            }
        }

        #[test]
        fn the_providers_of_what_an_added_plugin_requires_are_added_too() {
            // a plugin with fields does not make the implying fact ambiguous
            assert_eq!(
                built::<Resolved>(|_| {}, &["downstream"], &[]),
                Ok(vec!["chaining".to_owned(), "implied".to_owned()])
            );
        }

        /// The waiting plugin needs the unlocked fact, which two plugins provide, so the late fact
        /// implies nothing when it is tried; the unlocking plugin the opening fact implies provides
        /// the unlocked fact as well, so the late fact implies the waiting plugin once it is tried
        /// again.
        #[test]
        fn a_fact_tried_before_implies_a_plugin_by_what_a_later_addition_provides() {
            assert_eq!(
                built::<Resolved>(|_| {}, &["late"], &[]),
                Err(vec![("supplies".to_owned(), "the late fact")])
            );
            assert_eq!(
                built::<Resolved>(|_| {}, &["late", "opening"], &[]),
                Ok(vec!["unlocking".to_owned(), "waiting".to_owned()])
            );
        }

        #[test]
        fn a_provider_whose_requirement_cannot_be_provided_is_not_added() {
            // the stranding plugin would leave the ambiguous fact unprovided
            assert_eq!(
                built::<Resolved>(|_| {}, &["stranded"], &[]),
                Err(vec![("supplies".to_owned(), "the stranded fact")])
            );
        }

        #[test]
        fn a_provider_that_would_conflict_is_not_added() {
            let given = |declaration: &mut Declaration| {
                declaration.provide(Given);
            };

            assert_eq!(
                built::<Resolved>(|_| {}, &["clash"], &[]),
                Ok(vec!["clashing".to_owned()])
            );
            assert_eq!(
                built::<Resolved>(given, &["clash"], &[]),
                Err(vec![("supplies".to_owned(), "the clash fact")])
            );
        }

        #[test]
        fn a_provider_is_added_to_the_list_taking_its_axis() {
            assert_eq!(
                built::<Resolved>(|_| {}, &["deep"], &[]),
                Ok(vec!["deepening".to_owned()])
            );
        }

        #[test]
        fn a_fact_plugins_of_two_lists_provide_is_left_unprovided() {
            assert_eq!(
                built::<Resolved>(|_| {}, &["crosswise"], &[]),
                Err(vec![
                    ("supplies".to_owned(), "the crosswise fact"),
                    ("extras".to_owned(), "the crosswise fact"),
                ])
            );
        }

        #[test]
        fn nothing_is_added_for_a_fact_the_context_provides() {
            let context = |declaration: &mut Declaration| {
                declaration.provide(Implying);
            };

            assert_eq!(
                built::<Resolved>(context, &["implying"], &[]),
                Ok(Vec::new())
            );
        }

        #[test]
        fn an_ambiguous_fact_is_left_unprovided_and_the_others_provided() {
            assert_eq!(
                built::<Resolved>(|_| {}, &["ambiguous", "implying"], &[]),
                Err(vec![("supplies".to_owned(), "the ambiguous fact")])
            );
        }

        #[test]
        fn a_single_slot_is_never_added_to() {
            assert_eq!(
                built::<Resolved>(|_| {}, &["lone"], &[]),
                Err(vec![("requirements".to_owned(), "the lone fact")])
            );
        }

        #[test]
        fn a_list_of_a_plugin_is_never_added_to() {
            assert_eq!(
                built::<Resolved>(
                    |_| {},
                    &["nested"],
                    &[(
                        "supplies",
                        json!([plugin::<Bundle>().with("parts", json!([]))])
                    )]
                ),
                Err(vec![
                    ("supplies".to_owned(), "the nested fact"),
                    ("supplies[0].parts".to_owned(), "the nested fact"),
                ])
            );
        }

        #[test]
        fn an_axis_two_lists_take_is_never_added_to() {
            assert_eq!(
                built::<Twice>(|_| {}, &["implying"], &[]),
                Err(vec![
                    ("first".to_owned(), "the implying fact"),
                    ("second".to_owned(), "the implying fact"),
                ])
            );
        }
    }
}
