//! The part of the form of a phase before its plugins: its name, and where it takes its entrants
//! from.

use dioxus::prelude::*;
use tournament_app::form::{End, EntryView, PhaseForm};
use tournament_engine::PhaseId;

use super::fields::{Issues, Warnings};
use crate::{competition::Ctx, i18n::t, icons, phases::phase_id_text};

/// The name of the phase of `form`, and where it takes its entrants from: everyone registered for
/// the first phase unless the operator asks to take only some places, else a row per source, each
/// of a phase or the registration and a range of its places, with a button adding one more, and
/// where the lot decides who of the registration is taken.
#[component]
pub(super) fn Entry(form: Signal<PhaseForm>, entry: EntryView, disabled: bool) -> Element {
    let ctx = use_context::<Ctx>();
    let tr = ctx.translator.read();
    // what the phase is called without a name
    let format = form
        .read()
        .view()
        .fields
        .iter()
        .find_map(|field| match &field.input {
            tournament_app::form::Input::Plugin(plugin) if field.name == "format" => {
                plugin.chosen.as_ref().map(|chosen| chosen.plugin)
            }
            _ => None,
        });
    let placeholder = format.map(|format| {
        tr.phase_name(&tournament_app::view::PhaseName {
            number: entry.number,
            name: None,
            format: tournament_app::view::Plugin {
                id: format.id,
                label: format.label,
                description: format.description,
            },
        })
    });
    // where the lot decides who of the registration is taken, as the ratings stand
    let sources = form.read().sources().to_vec();
    let lot_cuts = ctx.session.read().lot_cuts(&sources);
    let choice_name = |phase: Option<PhaseId>| {
        entry
            .choices
            .iter()
            .find(|choice| choice.phase == phase)
            .and_then(|choice| choice.name.as_ref())
            .map_or_else(|| tr.get("registration"), |name| tr.phase_name(name))
    };
    rsx! {
        // on the grid of the numbers of the plugins' fields, a row each
        div { class: "field number-field phase-name",
            span { class: "number-label",
                label { r#for: "field-name", {tr.get("phase-name")} }
            }
            input {
                r#type: "text",
                id: "field-name",
                autocomplete: "off",
                value: "{entry.name}",
                placeholder,
                disabled,
                oninput: move |event| form.write().set_name(&event.value()),
            }
        }
        if entry.everyone_registered {
            div { class: "field number-field",
                span { class: "number-label", {tr.get("entrants-from")} }
                span { class: "answer",
                    {crate::capitalized(&tr.get("everyone-registered"))}
                    " "
                    button {
                        id: "some-places",
                        r#type: "button",
                        class: "quiet small",
                        disabled,
                        onclick: move |_| {
                            form.write().take_some_places();
                            crate::focus::focus_later(&["#source-0-from"]);
                        },
                        {tr.get("only-some-places")}
                    }
                }
            }
        } else {
            fieldset { class: "field sources",
                legend { {tr.get("entrants-from")} }
                ul { class: "source-rows",
                    for (index, row) in entry.rows.iter().cloned().enumerate() {
                        li { key: "{index}", class: "source-row",
                            label { class: "visually-hidden", r#for: "source-{index}",
                                {t!(tr, "source-of", number = index + 1)}
                            }
                            select {
                                id: "source-{index}",
                                disabled,
                                aria_invalid: row.issue.is_some().then_some("true"),
                                aria_describedby: "source-{index}-issues",
                                onchange: move |event| {
                                    let chosen = event.value();
                                    let phase = form
                                        .peek()
                                        .view()
                                        .entry
                                        .choices
                                        .iter()
                                        .find(|choice| choice_value(choice.phase) == chosen)
                                        .and_then(|choice| choice.phase);
                                    form.write().choose_source(index, phase);
                                },
                                for choice in entry.choices.iter() {
                                    option {
                                        value: choice_value(choice.phase),
                                        selected: choice.phase == row.of,
                                        {choice_name(choice.phase)}
                                    }
                                }
                            }
                            PlaceField {
                                form,
                                index,
                                end: End::From,
                                value: row.from.clone(),
                                disabled,
                                rejected: row.from_rejected.is_some(),
                            }
                            PlaceField {
                                form,
                                index,
                                end: End::To,
                                value: row.to.clone(),
                                disabled,
                                rejected: row.to_rejected.is_some(),
                            }
                            button {
                                r#type: "button",
                                class: "quiet small",
                                disabled,
                                aria_label: t!(tr, "remove-source", number = index + 1),
                                onclick: move |_| form.write().remove_source(index),
                                icons::Trash {}
                            }
                            Issues {
                                id: "source-{index}-issues",
                                issues: row
                                    .from_rejected
                                    .iter()
                                    .chain(&row.to_rejected)
                                    .chain(&row.issue)
                                    .cloned()
                                    .collect::<Vec<_>>(),
                            }
                        }
                    }
                }
                button {
                    r#type: "button",
                    disabled,
                    onclick: move |_| form.write().add_source(),
                    icons::Plus {}
                    {tr.get("add-source")}
                }
                Issues { id: "source-issues", issues: entry.issues.clone() }
                Warnings { id: "source-warnings", warnings: lot_cuts }
            }
        }
    }
}

/// The value of the option choosing `phase`, or the registration for `None`.
fn choice_value(phase: Option<PhaseId>) -> String {
    phase.map_or_else(|| "registration".to_owned(), phase_id_text)
}

/// The first or the last place of the source at `index`, as typed.
#[component]
fn PlaceField(
    form: Signal<PhaseForm>,
    index: usize,
    end: End,
    value: String,
    disabled: bool,
    rejected: bool,
) -> Element {
    let ctx = use_context::<Ctx>();
    let tr = ctx.translator.read();
    let (id, label) = match end {
        End::From => (format!("source-{index}-from"), tr.get("from-place")),
        End::To => (format!("source-{index}-to"), tr.get("to-place")),
    };
    rsx! {
        label { class: "place-field",
            span { {label} }
            input {
                r#type: "text",
                class: "number",
                id: "{id}",
                inputmode: "numeric",
                autocomplete: "off",
                value: "{value}",
                placeholder: if end == End::To { tr.get("to-last-place") },
                disabled,
                aria_invalid: rejected.then_some("true"),
                aria_describedby: "source-{index}-issues",
                onchange: move |event| form.write().enter_place(index, end, &event.value()),
            }
        }
    }
}
