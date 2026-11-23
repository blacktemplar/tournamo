//! How a competition without phases is played: by a template, whose phases are refined once
//! added, or by phases of its own, configured one by one at the advanced level.

use dioxus::prelude::*;
use tournament_app::{
    i18n::{Message as _, PluginText},
    session::Session,
};
use tournament_engine::plugin_factory::schema::Level;

use super::{NewPhase, summary::Summary};
use crate::{
    announcement::Announcement, capitalized, competition::Ctx, focus::focus_later, i18n::t,
    phases::sources_named, sentence,
};

/// How the competition is played: by a template or own phases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Played {
    /// By the template of this id.
    Template(&'static str),
    OwnPhases,
}

/// The choice of how the competition is played, templates first and own phases last, none
/// chosen at first unless a phase added by hand was undone, and below it the phases of the
/// template chosen, or the form of a phase. Choosing own phases shows the advanced settings, which
/// setting up phases by hand needs.
#[component]
pub fn HowPlayed() -> Element {
    let ctx = use_context::<Ctx>();
    // the phase whose adding was undone comes back in its form
    let mut chosen = use_signal(|| {
        ctx.session
            .peek()
            .has_undone_phase()
            .then_some(Played::OwnPhases)
    });
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
                            checked: chosen() == Some(Played::Template(template.id)),
                            aria_describedby: template
                                .description
                                .is_some()
                                .then(|| format!("about-{}", template.id.replace("::", "-"))),
                            onchange: move |_| chosen.set(Some(Played::Template(template.id))),
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
                        checked: chosen() == Some(Played::OwnPhases),
                        onchange: move |_| {
                            chosen.set(Some(Played::OwnPhases));
                            if *ctx.level.peek() < Level::Advanced {
                                ctx.choose_level(Level::Advanced);
                            }
                        },
                    }
                    " "
                    {tr.get("own-phases")}
                }
            }
        }
        match chosen() {
            None => rsx! {},
            Some(Played::OwnPhases) => rsx! {
                NewPhase {}
            },
            Some(Played::Template(id)) => rsx! {
                TemplateSetup { key: "{id}", id }
            },
        }
    }
}

/// The phases the template `id` adds, and the button adding them.
#[component]
fn TemplateSetup(id: &'static str) -> Element {
    let ctx = use_context::<Ctx>();
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

/// The template the phases were added by, with the phases added besides and those of it removed,
/// if they were added by one.
#[component]
pub fn BasedOn() -> Element {
    let ctx = use_context::<Ctx>();
    let tr = ctx.translator.read();
    let Some(based_on) = ctx.session.read().based_on(&tr) else {
        return rsx! {};
    };
    let template = capitalized(&tr.plugin(
        based_on.template.id,
        PluginText::Label,
        based_on.template.label,
    ));
    let added = based_on
        .added
        .iter()
        .map(|name| tr.phase_name(name))
        .collect::<Vec<_>>()
        .join(", ");
    let removed = based_on
        .removed
        .iter()
        .map(|phase| match &phase.name {
            Some(name) => name.clone(),
            None => t!(tr, "template-phase-number", number = phase.number),
        })
        .collect::<Vec<_>>()
        .join(", ");
    rsx! {
        p { id: "based-on", {t!(tr, "based-on", template = template)} }
        if !added.is_empty() {
            p { class: "hint", id: "added-to-template",
                {t!(tr, "added-to-template", phases = added)}
            }
        }
        if !removed.is_empty() {
            p { class: "hint", id: "removed-from-template",
                {t!(tr, "removed-from-template", phases = removed)}
            }
        }
    }
}
