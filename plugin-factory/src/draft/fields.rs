//! The answers of a draft as a tree of fields and the plugins answering them, walked along the
//! schemas: the one place that knows which answers hold plugins and where their fields are.
//!
//! The answers walked fit their schemas, as a draft's do; what does not is left out.

use crate::{
    catalog::{AnyAxis, PluginEntry},
    config::{Config, ConfigError, Path, PluginAnswer, Segment, Value},
    schema::{Field, FieldKind, Schema},
};

/// A field and its current answer, see [`Draft::fields`](super::Draft::fields).
///
/// A node borrows the answers of the draft: enumerate again after changing it. The path can be
/// retained for edits and for associating diagnostics with a field.
#[derive(Debug, Clone)]
pub struct FieldNode<'a> {
    pub path: Path,
    pub field: Field,
    /// `None` for an unanswered field; `Some(Value::Null)` is an answer.
    pub answer: Option<&'a Value>,
}

impl<'a> FieldNode<'a> {
    fn new(at: &Path, field: &Field, answer: Option<&'a Value>) -> Self {
        Self {
            path: at.clone().field(field.name()),
            field: field.clone(),
            answer,
        }
    }

    /// The plugins answering the field, one per item of a list, in list order. Empty for a
    /// scalar, an unanswered field, or an optional plugin answered with nothing.
    pub fn plugins(&self) -> Vec<PluginNode<'a>> {
        let (Some(slot), Some(answer)) = (Slot::of(self.field.kind()), self.answer) else {
            return Vec::new();
        };
        match (slot, answer) {
            (Slot::List(axis), Value::Array(items)) => items
                .iter()
                .enumerate()
                .filter_map(|(index, item)| {
                    PluginNode::new(axis, self.path.clone().index(index), item)
                })
                .collect(),
            (Slot::One(axis), answer) => PluginNode::new(axis, self.path.clone(), answer)
                .into_iter()
                .collect(),
            (Slot::List(_), _) => Vec::new(),
        }
    }
}

/// A chosen plugin, at the path of the field or the position in a list it answers.
#[derive(Debug, Clone)]
pub struct PluginNode<'a> {
    pub path: Path,
    pub entry: &'static PluginEntry,
    answer: PluginAnswer<'a>,
}

impl<'a> PluginNode<'a> {
    /// The plugin `answer` names among those of `axis`, or [`None`] if it names none.
    fn new(axis: AnyAxis, path: Path, answer: &'a Value) -> Option<Self> {
        let answer = PluginAnswer::read(answer)?;
        Some(Self {
            path,
            entry: axis.entry(answer.id())?,
            answer,
        })
    }

    /// The fields of the plugin in schema order.
    pub fn fields(&self) -> Vec<FieldNode<'a>> {
        self.entry
            .schema()
            .fields()
            .iter()
            .map(|field| FieldNode::new(&self.path, field, self.answer.answer(field.name())))
            .collect()
    }

    /// The field of the plugin named `name`, or [`None`] if it has none.
    fn field(&self, name: &str) -> Option<FieldNode<'a>> {
        let schema = self.entry.schema();
        let field = schema.field_named(name)?;
        Some(FieldNode::new(&self.path, field, self.answer.answer(name)))
    }

    /// The answers the plugin is configured with.
    pub(super) fn config(&self) -> Config {
        self.answer.config()
    }
}

/// The fields of `schema` answered by `answers`, in schema order.
pub(super) fn fields<'a>(schema: &Schema, answers: &'a Config) -> Vec<FieldNode<'a>> {
    schema
        .fields()
        .iter()
        .map(|field| FieldNode::new(&Path::ROOT, field, answers.get(field.name())))
        .collect()
}

/// A field or a plugin of the tree, see [`preorder`].
pub(super) enum Node<'a> {
    Field(FieldNode<'a>),
    Plugin(PluginNode<'a>),
}

/// `fields` and everything below them, depth first: each field followed by its plugins, each
/// plugin followed by its fields.
pub(super) fn preorder(fields: Vec<FieldNode<'_>>) -> Vec<Node<'_>> {
    let mut nodes = Vec::new();
    add(fields, &mut nodes);
    nodes
}

fn add<'a>(fields: Vec<FieldNode<'a>>, nodes: &mut Vec<Node<'a>>) {
    for field in fields {
        let plugins = field.plugins();
        nodes.push(Node::Field(field));
        for plugin in plugins {
            let fields = plugin.fields();
            nodes.push(Node::Plugin(plugin));
            add(fields, nodes);
        }
    }
}

/// What is at a path of the answers, see [`locate`].
pub(super) enum Position<'a> {
    Field(FieldNode<'a>),
    /// The position `index` in the list `list` of plugins of `axis`, which may be just past its
    /// end.
    Item {
        list: FieldNode<'a>,
        axis: AnyAxis,
        index: usize,
    },
}

impl<'a> Position<'a> {
    /// The kind of answer the position takes.
    pub(super) fn kind(&self) -> FieldKind {
        match self {
            Self::Field(node) => node.field.kind().clone(),
            Self::Item { axis, .. } => FieldKind::Plugin(*axis),
        }
    }

    /// The plugin answering the position, or [`None`] if it takes no plugin or has none.
    pub(super) fn plugin(&self) -> Option<PluginNode<'a>> {
        match self {
            Self::Field(node) => match Slot::of(node.field.kind()) {
                Some(Slot::One(_)) => node.plugins().into_iter().next(),
                _ => None,
            },
            // a list of a draft holds plugins only, so its plugins are its items
            Self::Item { list, index, .. } => list.plugins().into_iter().nth(*index),
        }
    }
}

/// What is at `path` in `answers`, the answers to `schema`.
///
/// # Errors
/// Returns [`ConfigError::UnknownField`] if `path` addresses nothing in the answers: the root, a
/// field its config does not have, a field of a plugin that is not chosen, or a position in a
/// list further than just past its end.
pub(super) fn locate<'a>(
    schema: &Schema,
    answers: &'a Config,
    path: &Path,
) -> Result<Position<'a>, ConfigError> {
    let unknown = || ConfigError::UnknownField { path: path.clone() };
    let mut segments = path.segments().iter();
    let Some(Segment::Field(name)) = segments.next() else {
        return Err(unknown());
    };
    let field = schema.field_named(name).ok_or_else(unknown)?;
    let mut position = Position::Field(FieldNode::new(&Path::ROOT, field, answers.get(name)));
    for segment in segments {
        position = match (position, segment) {
            (Position::Field(list), Segment::Index(index)) => {
                let Some(Slot::List(axis)) = Slot::of(list.field.kind()) else {
                    return Err(unknown());
                };
                let length = list.answer.and_then(Value::as_array).map(Vec::len);
                if !length.is_some_and(|length| *index <= length) {
                    return Err(unknown());
                }
                Position::Item {
                    list,
                    axis,
                    index: *index,
                }
            }
            (position, Segment::Field(name)) => {
                let plugin = position.plugin().ok_or_else(unknown)?;
                Position::Field(plugin.field(name).ok_or_else(unknown)?)
            }
            (Position::Item { .. }, Segment::Index(_)) => return Err(unknown()),
        };
    }
    Ok(position)
}

/// What a position taking plugins takes.
#[derive(Clone, Copy)]
pub(super) enum Slot {
    /// One plugin of the axis.
    One(AnyAxis),
    /// A list of plugins of the axis.
    List(AnyAxis),
}

impl Slot {
    /// What a position of `kind` takes, looking through [`FieldKind::Optional`], or [`None`] if it
    /// takes no plugin.
    pub(super) fn of(kind: &FieldKind) -> Option<Self> {
        match kind {
            FieldKind::Plugin(axis) => Some(Self::One(*axis)),
            FieldKind::List { axis, .. } => Some(Self::List(*axis)),
            FieldKind::Optional(inner) => Self::of(inner),
            FieldKind::Bool | FieldKind::Integer(_) | FieldKind::Decimal(_) => None,
        }
    }

    /// The axis of the plugins.
    pub(super) fn axis(self) -> AnyAxis {
        match self {
            Self::One(axis) | Self::List(axis) => axis,
        }
    }
}
