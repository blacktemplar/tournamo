//! How a competition without phases is played: by a preset, set up by its fields, or by phases
//! of its own, configured one by one.

use dioxus::prelude::*;
use tournament_app::{
    form::PresetForm,
    i18n::{Message as _, PluginText},
    session::Session,
};

use super::{
    NewPhase,
    fields::{EditedForm, Fields, Issues},
    focus_first_issue,
    summary::Summary,
};
use crate::{
    announcement::Announcement, capitalized, competition::Ctx, focus::focus_later,
    phases::sources_named, sentence,
};

/// The choice of how the competition is played, presets first and own phases last, chosen at
/// first, and below it the form of what is chosen.
#[component]
pub fn HowPlayed() -> Element {
    let ctx = use_context::<Ctx>();
    // the preset chosen, by its id; `None` for own phases
    let mut chosen = use_signal(|| None::<&'static str>);
    let tr = ctx.translator.read();
    rsx! {
        fieldset { class: "how-played",
            legend { {tr.get("how-played")} }
            for preset in Session::presets() {
                div { key: "{preset.id}", class: "choice",
                    label {
                        input {
                            r#type: "radio",
                            name: "how-played",
                            id: "preset-{preset.id.replace(\"::\", \"-\")}",
                            checked: chosen() == Some(preset.id),
                            aria_describedby: preset
                                .description
                                .is_some()
                                .then(|| format!("about-{}", preset.id.replace("::", "-"))),
                            onchange: move |_| chosen.set(Some(preset.id)),
                        }
                        " "
                        {capitalized(&tr.plugin(preset.id, PluginText::Label, preset.label))}
                    }
                    if let Some(description) = preset.description {
                        p {
                            class: "hint",
                            id: "about-{preset.id.replace(\"::\", \"-\")}",
                            {sentence(&tr.plugin(preset.id, PluginText::Description, description))}
                        }
                    }
                }
            }
            div { class: "choice",
                label {
                    input {
                        r#type: "radio",
                        name: "how-played",
                        id: "own-phases",
                        checked: chosen().is_none(),
                        onchange: move |_| chosen.set(None),
                    }
                    " "
                    {tr.get("own-phases")}
                }
            }
        }
        match chosen() {
            None => rsx! {
                NewPhase {}
            },
            Some(id) => rsx! {
                PresetSetup { key: "{id}", id }
            },
        }
    }
}

/// The fields of the preset `id`, the phases it adds, and the button adding them.
#[component]
fn PresetSetup(id: &'static str) -> Element {
    let ctx = use_context::<Ctx>();
    let created = use_hook(|| {
        Session::preset_form(id)
            .map(Signal::new)
            .map_err(|error| error.text())
    });
    let actions = ctx.actions();
    let tr = ctx.translator.read();
    let form: Signal<PresetForm> = match created {
        Ok(form) => form,
        Err(error) => return rsx! { p { class: "error", {tr.sentence(&error)} } },
    };
    let view = form.read().view();
    let preview = form.read().preview(&tr);
    rsx! {
        form {
            id: "new-preset",
            onsubmit: move |event| {
                event.prevent_default();
                if !form.read().view().complete {
                    focus_first_issue("new-preset");
                    return;
                }
                let tr = ctx.translator.peek();
                if let Some(ids) = ctx.act(|session| session.add_preset(&form.read(), &tr)) {
                    ctx.announce(Announcement::AddedPhases(ids.len()));
                    // the form is gone, and with it what had the focus
                    focus_later(&["#start:enabled", "#phase"]);
                }
            },
            div { class: "phase-form",
                Fields {
                    form: EditedForm::Preset(form),
                    fields: view.fields,
                    disabled: !actions.add_phase,
                }
                Issues { id: "preset-issues", issues: view.issues }
            }
            h3 { id: "preset-preview", {tr.get("preset-preview")} }
            match preview {
                Ok(phases) => rsx! {
                    ol { class: "phases numbered", aria_labelledby: "preset-preview",
                        for phase in phases.iter() {
                            li { key: "{phase.name.number}",
                                h4 { {tr.phase_name(&phase.name)} }
                                p { class: "sources",
                                    {
                                        sources_named(
                                            &tr,
                                            &phase.sources,
                                            |id| {
                                                phases
                                                    .iter()
                                                    .find(|phase| phase.id == id)
                                                    .map(|phase| &phase.name)
                                            },
                                        )
                                    }
                                }
                                Summary { fields: phase.summary.clone() }
                            }
                        }
                    }
                },
                Err(why) => rsx! {
                    p { class: "hint", {tr.sentence(&why)} }
                },
            }
            div { class: "form-actions",
                button {
                    class: "primary",
                    r#type: "submit",
                    disabled: !actions.add_phase,
                    {tr.get("add-phases")}
                }
            }
        }
    }
}
