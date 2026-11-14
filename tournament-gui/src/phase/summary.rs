//! How the phase is played, read from the fields of its form.

use dioxus::prelude::*;
use tournament_app::{
    form::{FormField, Input, ListInput, PluginInput},
    i18n::Translator,
};

use super::fields::plugin_label;
use crate::{capitalized, competition::Ctx};

/// How the phase is played, read from the fields of its form.
#[component]
pub(super) fn Summary(fields: Vec<FormField>) -> Element {
    let ctx = use_context::<Ctx>();
    let tr = ctx.translator.read();
    rsx! {
        dl { class: "summary",
            for field in fields {
                dt { key: "{field.path}", {capitalized(&tr.field_label(&field))} }
                dd {
                    match &field.input {
                        // each item of a list on a line of its own, with its fields below it
                        Input::List(list) if has_fields(list) => rsx! {
                            ul { class: "summary-items",
                                for item in list.items.clone() {
                                    li {
                                        span { class: "answer",
                                            {capitalized(&plugin_label(&tr, &item.plugin))}
                                        }
                                        if !item.fields.is_empty() {
                                            Summary { fields: item.fields }
                                        }
                                    }
                                }
                                for implied in list.implied.clone() {
                                    li {
                                        span { class: "answer",
                                            {capitalized(&plugin_label(&tr, &implied.plugin))}
                                        }
                                    }
                                }
                            }
                        },
                        input => rsx! {
                            span { class: "answer", {capitalized(&answer(&tr, input))} }
                            if let Input::Plugin(PluginInput { chosen: Some(chosen), .. }) = input
                                && !chosen.fields.is_empty()
                            {
                                Summary { fields: chosen.fields.clone() }
                            }
                        },
                    }
                }
            }
        }
    }
}

/// Whether any plugin of `list` has fields.
fn has_fields(list: &ListInput) -> bool {
    list.items.iter().any(|item| !item.fields.is_empty())
}

/// `input`'s answer as the summary reads it, without the fields of the plugins in it.
pub fn answer(tr: &Translator, input: &Input) -> String {
    match input {
        Input::Bool(Some(true)) => tr.get("yes"),
        Input::Bool(Some(false)) => tr.get("no"),
        Input::Integer {
            value: Some(value), ..
        } => value.to_string(),
        Input::Decimal {
            value: Some(value), ..
        } => tr.number(*value),
        Input::Optional {
            answered: true,
            inner,
        } => answer(tr, inner),
        Input::Plugin(PluginInput {
            chosen: Some(chosen),
            ..
        }) => plugin_label(tr, &chosen.plugin),
        Input::List(list) => {
            let labels: Vec<_> = list
                .items
                .iter()
                .map(|item| plugin_label(tr, &item.plugin))
                .chain(
                    list.implied
                        .iter()
                        .map(|implied| plugin_label(tr, &implied.plugin)),
                )
                .collect();
            if labels.is_empty() {
                tr.get("no-plugin")
            } else {
                labels.join(", ")
            }
        }
        _ => tr.get("no-plugin"),
    }
}
