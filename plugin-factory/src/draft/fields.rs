use crate::{
    catalog::PluginEntry,
    config::{Config, Path, PluginConfig, Value},
    schema::{Field, Schema},
};

/// A field and its current answer, followed by the plugins answering it.
///
/// An enumeration is a snapshot: enumerate again after changing the draft. The path can be
/// retained for edits and for associating diagnostics with a field.
pub struct FieldNode {
    pub path: Path,
    pub field: Field,
    /// `None` for an unanswered field; `Some(Value::Null)` is an answer.
    pub answer: Option<Value>,
    /// Empty for a scalar, an unanswered field, or an optional plugin answered with nothing.
    pub plugins: Vec<PluginNode>,
}

/// A chosen plugin and its fields. A list has one node per item, in list order.
pub struct PluginNode {
    pub path: Path,
    pub entry: &'static PluginEntry,
    pub(super) config: Config,
    pub fields: Vec<FieldNode>,
}

pub(super) fn fields(schema: &Schema, config: &Config, at: &Path) -> Vec<FieldNode> {
    schema
        .fields()
        .iter()
        .map(|field| {
            let path = at.clone().field(field.name());
            let answer = config.get(field.name()).cloned();
            let mut plugins = Vec::new();
            if let Some(axis) = field.kind().axis() {
                let answers = match &answer {
                    Some(Value::Array(answers)) => answers
                        .iter()
                        .enumerate()
                        .map(|(index, answer)| (path.clone().index(index), answer))
                        .collect(),
                    Some(answer) => vec![(path.clone(), answer)],
                    None => Vec::new(),
                };
                for (path, answer) in answers {
                    let Some(plugin) = PluginConfig::from_answer(answer) else {
                        continue;
                    };
                    let Some(entry) = axis.entry(plugin.plugin()) else {
                        continue;
                    };
                    plugins.push(PluginNode {
                        fields: fields(&entry.schema(), plugin.config(), &path),
                        path,
                        entry,
                        config: plugin.config().clone(),
                    });
                }
            }
            FieldNode {
                path,
                field: field.clone(),
                answer,
                plugins,
            }
        })
        .collect()
}
