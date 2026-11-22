//! How a competition without phases is played: by a template, whose phases are refined once
//! added, or by phases of its own, configured one by one.

use dioxus::prelude::*;
use tournament_app::{
    i18n::{Message as _, PluginText},
    session::Session,
};
use tournament_engine::plugin_factory::schema::Level;

use super::{NewPhase, summary::Summary};
use crate::{
    Page, announcement::Announcement, capitalized, competition::Ctx, focus::focus_later,
    phases::sources_named, sentence,
};

/// The choice of how the competition is played, templates first and own phases last, chosen at
/// first, and below it the phases of the template chosen, or the form of a phase.
#[component]
pub fn HowPlayed() -> Element {
    let ctx = use_context::<Ctx>();
    // the template chosen, by its id; `None` for own phases
    let mut chosen = use_signal(|| None::<&'static str>);
    let tr = ctx.translator.read();
    rsx! {
        fieldset { class: "how-played",
            legend { {tr.get("how-played")} }
            for template in Session::templates() {
                div { key: "{template.id}", class: "choice",
                    label {
                        input {
                            r#type: "radio",
                            name: "how-played",
                            id: "template-{template.id.replace(\"::\", \"-\")}",
                            checked: chosen() == Some(template.id),
                            aria_describedby: template
                                .description
                                .is_some()
                                .then(|| format!("about-{}", template.id.replace("::", "-"))),
                            onchange: move |_| chosen.set(Some(template.id)),
                        }
                        " "
                        {capitalized(&tr.plugin(template.id, PluginText::Label, template.label))}
                    }
                    if let Some(description) = template.description {
                        p {
                            class: "hint",
                            id: "about-{template.id.replace(\"::\", \"-\")}",
                            {
                                sentence(
                                    &tr.plugin(template.id, PluginText::Description, description),
                                )
                            }
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
                TemplateSetup { key: "{id}", id }
            },
        }
    }
}

/// The phases the template `id` adds, and the button adding them.
#[component]
fn TemplateSetup(id: &'static str) -> Element {
    let ctx = use_context::<Ctx>();
    let page = use_context::<Page>();
    let actions = ctx.actions();
    let tr = ctx.translator.read();
    let preview = Session::template_preview(id, &tr).map_err(|error| error.text());
    rsx! {
        form {
            id: "new-template",
            onsubmit: move |event| {
                event.prevent_default();
                let tr = ctx.translator.peek();
                if let Some(ids) = ctx.act(|session| session.add_template(id, &tr)) {
                    ctx.announce(Announcement::AddedPhases(ids.len()));
                    // a template is refined in the basic view, whatever the level before
                    page.choose_level(Level::Basic);
                    // the choice is gone, and with it what had the focus
                    focus_later(&["#start:enabled", "#phase"]);
                }
            },
            h3 { id: "template-preview", {tr.get("template-preview")} }
            match preview {
                Ok(phases) => rsx! {
                    ol { class: "phases numbered", aria_labelledby: "template-preview",
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
                    p { class: "error", {tr.sentence(&why)} }
                },
            }
            p { class: "hint", {tr.get("template-refined")} }
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
