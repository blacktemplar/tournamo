//! Which fields shown differ from what is expected of them, see [`FormField::mark`]: a field
//! answered otherwise, and in a list a plugin not expected in it, or expected elsewhere in its
//! order, compared as a diff compares lines, so that a plugin inserted marks only itself.

use plugin_factory::{
    config::{Config, PluginConfig, Value},
    draft::Draft,
};
use tournament_engine::PhasePlugins;

use super::{FormField, Input, ListInput, PluginOption};

/// How a field shown, or a plugin of a list, differs from what is expected of it, see
/// [`FormField::mark`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    /// Answered otherwise; for a field taking a plugin, another plugin chosen. A field whose
    /// plugin is the one expected is not marked, the fields of the plugin that differ are.
    Differs,
    /// A plugin of a list that is not expected in it.
    Added,
    /// A plugin of a list whose order matters, expected in it, but elsewhere in its order.
    Moved,
}

/// Marks `fields`, the fields of `draft` or of a plugin of it, answered by `current`, where they
/// differ from `expected`; a field `expected` does not answer is not marked.
pub(super) fn mark(
    draft: &Draft<PhasePlugins>,
    fields: &mut [FormField],
    current: &Config,
    expected: &Config,
) {
    for field in fields {
        if let Some(expected) = expected.get(field.name) {
            mark_field(draft, field, current.get(field.name), expected);
        }
    }
}

/// Whether anything of `fields` is marked, however deep, see [`mark`].
pub(super) fn any(fields: &[FormField]) -> bool {
    fields.iter().any(|field| {
        field.mark.is_some()
            || match &field.input {
                Input::Plugin(plugin) => plugin
                    .chosen
                    .as_ref()
                    .is_some_and(|chosen| any(&chosen.fields)),
                Input::List(list) => {
                    !list.removed.is_empty()
                        || list
                            .items
                            .iter()
                            .any(|item| item.mark.is_some() || any(&item.fields))
                }
                _ => false,
            }
    })
}

fn mark_field(
    draft: &Draft<PhasePlugins>,
    field: &mut FormField,
    current: Option<&Value>,
    expected: &Value,
) {
    match &mut field.input {
        Input::Plugin(plugin) => {
            let current = current.and_then(PluginConfig::from_answer);
            let expected = PluginConfig::from_answer(expected);
            let id = |config: &Option<PluginConfig>| {
                config.as_ref().map(|config| config.plugin().to_owned())
            };
            if id(&current) != id(&expected) {
                field.mark = Some(Mark::Differs);
            } else if let (Some(chosen), Some(current), Some(expected)) =
                (&mut plugin.chosen, current, expected)
            {
                mark(
                    draft,
                    &mut chosen.fields,
                    current.config(),
                    expected.config(),
                );
            }
        }
        Input::List(list) => mark_list(draft, &field.path, list, current, expected),
        _ => {
            if current.unwrap_or(&Value::Null) != expected {
                field.mark = Some(Mark::Differs);
            }
        }
    }
}

/// Marks the plugins of `list`, the list at `path`, answered by `current`, where they differ from
/// `expected`: matched as a diff matches lines where the order matters, else by plugin; a plugin
/// matched is not marked itself, its fields that differ are. Lists the plugins expected and
/// removed in [`ListInput::removed`].
fn mark_list(
    draft: &Draft<PhasePlugins>,
    path: &plugin_factory::config::Path,
    list: &mut ListInput,
    current: Option<&Value>,
    expected: &Value,
) {
    let configs = |value: Option<&Value>| -> Vec<PluginConfig> {
        value
            .and_then(Value::as_array)
            .map(|items| items.iter().filter_map(PluginConfig::from_answer).collect())
            .unwrap_or_default()
    };
    let (current, expected) = (configs(current), configs(Some(expected)));
    let ids = |configs: &[PluginConfig]| -> Vec<String> {
        configs
            .iter()
            .map(|config| config.plugin().to_owned())
            .collect()
    };
    let (current_ids, expected_ids) = (ids(&current), ids(&expected));
    let matched = if list.ordered {
        in_order(&current_ids, &expected_ids)
    } else {
        by_plugin(&current_ids, &expected_ids)
    };
    for (index, item) in list.items.iter_mut().enumerate() {
        match matched.get(index).copied().flatten() {
            Some(at) => mark(
                draft,
                &mut item.fields,
                current[index].config(),
                expected[at].config(),
            ),
            None if expected_ids.contains(&current_ids[index]) => item.mark = Some(Mark::Moved),
            None => item.mark = Some(Mark::Added),
        }
    }
    let axis = draft
        .field_at(path)
        .and_then(|node| node.field.kind().axis());
    list.removed = expected_ids
        .iter()
        .filter(|id| !current_ids.contains(id))
        .filter_map(|id| axis?.entry(id))
        .map(|entry| PluginOption::of(entry, false))
        .collect();
}

/// For each of `current`, the position of `expected` it is matched with, if any: the longest
/// sequence of both in the same order, as a diff matches lines.
fn in_order(current: &[String], expected: &[String]) -> Vec<Option<usize>> {
    let (n, m) = (current.len(), expected.len());
    // the length of the longest common sequence of the ends from each pair of positions
    let mut longest = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            longest[i][j] = if current[i] == expected[j] {
                longest[i + 1][j + 1] + 1
            } else {
                longest[i + 1][j].max(longest[i][j + 1])
            };
        }
    }
    let mut matched = vec![None; n];
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if current[i] == expected[j] {
            matched[i] = Some(j);
            i += 1;
            j += 1;
        } else if longest[i + 1][j] >= longest[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    matched
}

/// For each of `current`, the position of `expected` of the same plugin it is matched with, if
/// any, each matched once.
fn by_plugin(current: &[String], expected: &[String]) -> Vec<Option<usize>> {
    let mut taken = vec![false; expected.len()];
    current
        .iter()
        .map(|id| {
            let at = (0..expected.len()).find(|&at| !taken[at] && expected[at] == *id)?;
            taken[at] = true;
            Some(at)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::*;

    fn ids(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|id| (*id).to_owned()).collect()
    }

    #[test]
    fn a_plugin_inserted_leaves_the_others_matched() {
        assert_eq!(
            in_order(&ids(&["c", "a", "b"]), &ids(&["a", "b"])),
            [None, Some(0), Some(1)]
        );
    }

    #[test]
    fn of_two_plugins_swapped_one_is_matched() {
        assert_eq!(
            in_order(&ids(&["b", "a"]), &ids(&["a", "b"])),
            [None, Some(0)]
        );
    }
}
