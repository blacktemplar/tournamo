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
use fields::{Node, Position, Slot, locate, preorder};

use crate::{
    Plugin,
    catalog::{AnyAxis, PluginEntry},
    compatibility::{self, Declaration, Declared, Incompatibility, OpenFacts, RequiredFact},
    config::{self, Config, ConfigError, Path, PluginAnswer, PluginConfig, Segment, Value},
    schema::{Configurable, Schema},
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
/// fn fields(nodes: Vec<FieldNode>, lines: &mut Vec<String>) {
///     for node in nodes {
///         lines.push(format!("{}: {}", node.path, node.field.label()));
///         for plugin in node.plugins() {
///             fields(plugin.fields(), lines);
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
/// fields(draft.fields(), &mut lines);
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
        // answering a field adds only the fields of the plugins in its answer, which follow it
        // depth first, so the first one left is the next in that order
        while let Some((path, default)) = self.first_default() {
            self.set(&path, default)?;
        }
        Ok(self)
    }

    /// The first unanswered field offering an answer, depth first, and the answer it offers.
    fn first_default(&self) -> Option<(Path, Value)> {
        preorder(self.fields())
            .into_iter()
            .find_map(|node| match node {
                Node::Field(field) if field.answer.is_none() => {
                    Some((field.path, field.field.default_value()?.clone()))
                }
                _ => None,
            })
    }

    /// The answers given so far, as the config of the composite.
    pub fn answers(&self) -> &Config {
        &self.answers
    }

    /// The fields in schema order, each with its answer and the fields of its chosen plugins.
    /// Unanswered fields are included; defaults and implied plugins are not added.
    pub fn fields(&self) -> Vec<FieldNode<'_>> {
        fields::fields(&T::schema(), &self.answers)
    }

    /// The field at `path`, or [`None`] if `path` addresses no field of the answers, e.g. a
    /// position in a list.
    pub fn field_at(&self, path: &Path) -> Option<FieldNode<'_>> {
        match locate(&T::schema(), &self.answers, path).ok()? {
            Position::Field(node) => Some(node),
            Position::Item { .. } => None,
        }
    }

    /// The plugin answering the field or the position in a list at `path`, or [`None`] if there
    /// is none.
    pub fn plugin_at(&self, path: &Path) -> Option<PluginNode<'_>> {
        locate(&T::schema(), &self.answers, path).ok()?.plugin()
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
        if let Position::Item { list, index, .. } = locate(&T::schema(), &self.answers, path)?
            && list.answer.and_then(Value::as_array).map(Vec::len) == Some(index)
        {
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
        Ok(match locate(&T::schema(), &self.answers, path)? {
            Position::Field(node) => node.field.default_value().cloned(),
            Position::Item { .. } => None,
        })
    }

    /// Whether the field at `path` matters with the answers given: not if the composite provides
    /// the fact that makes it irrelevant, see [`Field::irrelevant_with`], its context or a plugin
    /// answered so far; a position in a list always does. A UI may leave out a field that does
    /// not; it is answered all the same.
    ///
    /// [`Field::irrelevant_with`]: crate::schema::Field::irrelevant_with
    ///
    /// # Errors
    /// Returns [`ConfigError::UnknownField`] if `path` addresses nothing in the answers.
    pub fn is_relevant(&self, path: &Path) -> Result<bool, ConfigError> {
        let schema = T::schema();
        let Position::Field(node) = locate(&schema, &self.answers, path)? else {
            return Ok(true);
        };
        let Some(fact) = node.field.irrelevant_with() else {
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
        let position = locate(&T::schema(), &self.answers, path)?;
        let Some(slot) = Slot::of(&position.kind()) else {
            return Ok(Vec::new());
        };
        let taken: Vec<_> = match &position {
            Position::Item { list, index, .. } => list
                .plugins()
                .into_iter()
                .enumerate()
                .filter(|(other, _)| other != index)
                .map(|(_, plugin)| plugin.entry.id())
                .collect(),
            Position::Field(_) => Vec::new(),
        };
        Ok(slot
            .axis()
            .entries()
            .filter(|entry| !taken.contains(&entry.id()))
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
        let issues = issues_in(&self.context, &Tree::of(&schema, &self.answers), true);
        if !issues.is_empty() {
            return Err(issues);
        }
        let walked = schema
            .walk(&self.answers, &Path::ROOT, true)
            .map_err(invalid)?;
        config::deserialize(&config::complete::<T>(walked), &Path::ROOT).map_err(invalid)
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
        if let Err(error) = &placed.instance
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
        .filter_map(|field| match Slot::of(field.kind()) {
            Some(Slot::List(axis)) => Some((field.name(), axis)),
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

/// [`Draft::set`] on `answers`, the answers to `schema`.
fn set_in(
    schema: &Schema,
    answers: &mut Config,
    path: &Path,
    answer: &Value,
) -> Result<(), ConfigError> {
    let answer = locate(schema, answers, path)?
        .kind()
        .walk(answer, path, false)?;
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
            // checking an answer writes the config of a plugin as an object
            let config = if parent.is_empty() {
                &mut answers.0
            } else {
                answers
                    .get_at_mut(parent)
                    .and_then(PluginAnswer::config_mut)
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

/// The answers as a tree: every field and every plugin answering one.
struct Tree<'a> {
    /// Every field, depth first in schema order.
    questions: Vec<FieldNode<'a>>,
    /// Every plugin answering a field or a position in a list, depth first in schema order.
    plugins: Vec<Placed>,
}

/// A plugin in the answers, built once with the tree.
struct Placed {
    path: Path,
    /// The plugin built from its config, or why it cannot be, e.g. because a field is still
    /// unanswered, see [`PluginEntry::instance`].
    instance: Result<Box<dyn Plugin>, ConfigError>,
    /// What the plugin declares itself, without the plugins answering its fields, which are
    /// placed in the tree themselves: what its configuration declares if it is built, and its
    /// static declaration otherwise. Named by its label and path.
    declared: Declared,
}

impl<'a> Tree<'a> {
    /// The tree of `answers`, the answers to `schema`.
    fn of(schema: &Schema, answers: &'a Config) -> Self {
        let mut tree = Self {
            questions: Vec::new(),
            plugins: Vec::new(),
        };
        for node in preorder(fields::fields(schema, answers)) {
            match node {
                Node::Field(field) => tree.questions.push(field),
                Node::Plugin(plugin) => tree.plugins.push(Placed::new(&plugin)),
            }
        }
        tree
    }

    /// The fields taking plugins among those `included` includes, each with its axis, in the order
    /// of [`Self::questions`].
    fn slots(&self, included: impl Fn(&FieldNode) -> bool) -> Vec<(Path, AnyAxis)> {
        self.questions
            .iter()
            .filter(|question| included(question))
            .filter_map(|question| {
                Some((
                    question.path.clone(),
                    Slot::of(question.field.kind())?.axis(),
                ))
            })
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
    /// The plugin of `plugin` as its answer configures it.
    fn new(plugin: &PluginNode) -> Self {
        let PluginNode { path, entry, .. } = plugin;
        let instance = entry.instance(&plugin.config(), path);
        let mut declaration = entry.declaration();
        if let Ok(instance) = &instance {
            instance.declare(&mut declaration);
        }
        let declared = Declared::new(format!("{} ({path})", entry.label()), declaration);
        Self {
            path: path.clone(),
            instance,
            declared,
        }
    }
}

#[cfg(test)]
mod tests;
