//! How a competition without phases is played: by a template, whose phases are changed before
//! they are added, as a phase of its own is, or by phases of its own, configured one by one at the
//! advanced level.

use dioxus::prelude::*;
use tournament_app::{
    form::{PhaseForm, TemplateForm},
    i18n::{Message as _, PluginText},
    session::Session,
    view::{PhaseName, SourceRow},
};
use tournament_engine::plugin_factory::schema::Level;

use super::{
    DiscardChanges, Form, NewPhase,
    basic::{BasicFieldsOf, CompetitionSettings, Phases},
    fields::Issues,
    focus_first_issue,
    summary::{self, Summary},
    use_discarding,
};
use crate::{
    announcement::Announcement,
    capitalized,
    competition::Ctx,
    components::dialog::{Dialog, DialogDescription, DialogTitle},
    focus::focus_later,
    i18n::t,
    icons,
    phases::sources_named,
    sentence,
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

/// The phases of the template `id`, each changed as an added phase is, at the level shown, and
/// the button adding them as they stand.
#[component]
fn TemplateSetup(id: &'static str) -> Element {
    let ctx = use_context::<Ctx>();
    let created = use_hook(|| {
        ctx.session
            .peek()
            .template_form(id, &ctx.translator.peek())
            .map(Signal::new)
            .map_err(|error| error.text())
    });
    let actions = ctx.actions();
    let tr = ctx.translator.read();
    let template = match created {
        Ok(template) => template,
        Err(why) => {
            return rsx! {
                p { class: "error", {tr.sentence(&why)} }
            };
        }
    };
    let basic = *ctx.level.read() == Level::Basic;
    let ids = template.read().ids().to_vec();
    let disabled = !actions.add_phase;
    rsx! {
        div { id: "new-template",
            h3 { id: "template-phases", {tr.get("template-phases")} }
            p { class: "hint", {tr.get("template-refined")} }
            if basic {
                CompetitionSettings { phases: Phases::Template(template), disabled }
            }
            ol {
                class: if basic { "phases numbered basic" } else { "phases numbered" },
                aria_labelledby: "template-phases",
                for (index , id) in ids.iter().enumerate() {
                    li { key: "{id:?}",
                        TemplatePhase {
                            template,
                            index,
                            basic,
                            disabled,
                        }
                    }
                }
            }
            div { class: "form-actions",
                button {
                    class: "primary",
                    r#type: "button",
                    disabled,
                    onclick: move |_| {
                        if !template.peek().complete() {
                            focus_first_issue("new-template");
                            return;
                        }
                        if let Some(ids) = ctx
                            .act(|session| session.add_template_form(&template.peek()))
                        {
                            ctx.announce(Announcement::AddedPhases(ids.len()));
                            // the choice is gone, and with it what had the focus
                            focus_later(&["#start:enabled", "#phase"]);
                        }
                    },
                    {tr.get("add-phases")}
                }
            }
        }
    }
}

/// The phase at `index` of `template`: in the basic view's layout if `basic`, else as an added
/// phase is summed up, with the button editing it in a dialog. It edits a copy of its form, which
/// it hands back to the template after every change, and takes anew once the other forms change
/// it (see [`TemplateForm::revision`]).
#[component]
fn TemplatePhase(
    template: Signal<TemplateForm>,
    index: usize,
    basic: bool,
    disabled: bool,
) -> Element {
    let ctx = use_context::<Ctx>();
    let mut form = use_signal(|| template.peek().forms()[index].clone());
    let mut seen = use_signal(|| template.peek().revision(index));
    // decided as it opens, as in the basic view
    let fields = use_hook(|| form.peek().basic_fields());
    use_effect(move || {
        let edited = form.read().clone();
        template.write().replace(index, edited);
    });
    use_effect(move || {
        let template = template.read();
        let revision = template.revision(index);
        if revision != *seen.peek() {
            seen.set(revision);
            form.set(template.forms()[index].clone());
        }
    });
    let tr = ctx.translator.read();
    let name = template.read().name(index);
    let id = template.read().ids()[index];
    rsx! {
        h4 { {tr.phase_name(&name)} }
        if basic {
            BasicFieldsOf {
                form,
                basic: fields,
                of: id,
                disabled,
            }
        } else {
            TemplatePhaseSummary {
                template,
                form,
                index,
                disabled,
            }
        }
    }
}

/// The phase at `index` of `template` as an added phase is summed up: where its entrants come
/// from, how it is played, and the button editing `form`, its copy, in a dialog.
#[component]
fn TemplatePhaseSummary(
    template: Signal<TemplateForm>,
    form: Signal<PhaseForm>,
    index: usize,
    disabled: bool,
) -> Element {
    let ctx = use_context::<Ctx>();
    let tr = ctx.translator.read();
    let view = form.read().view();
    let columns = summary::columns(&tr, &view);
    let sources: Vec<_> = form.read().sources().iter().map(SourceRow::of).collect();
    let names: Vec<_> = (0..template.read().forms().len())
        .map(|earlier| {
            (
                template.read().ids()[earlier],
                template.read().name(earlier),
            )
        })
        .collect();
    let name = template.read().name(index);
    rsx! {
        p { class: "sources",
            {
                sources_named(
                    &tr,
                    &sources,
                    |id| names.iter().find(|(of, _)| *of == id).map(|(_, name)| name),
                )
            }
        }
        Summary { fields: view.summary(), columns }
        Issues {
            id: format!("template-issues-{index}"),
            issues: view.issues.clone(),
        }
        if !disabled {
            div { class: "phase-actions",
                EditFormDialog { form, name }
            }
        }
    }
}

/// The button editing `form`, the form of the phase `name`, in a dialog, and the dialog: saving
/// puts the form edited in its place.
#[component]
fn EditFormDialog(form: Signal<PhaseForm>, name: PhaseName) -> Element {
    let ctx = use_context::<Ctx>();
    // the copy edited while the dialog is open, and the form as it opened
    let mut editing = use_signal(|| None::<(Signal<PhaseForm>, PhaseForm)>);
    let discarding = use_discarding();
    let mut close = move || {
        editing.set(None);
        discarding.keep_editing();
    };
    let tr = ctx.translator.read();
    let edited = editing.read().as_ref().map(|(form, _)| *form);
    rsx! {
        button {
            r#type: "button",
            aria_label: t!(tr, "edit-phase-named", phase = tr.phase_name(&name)),
            onclick: move |_| {
                let opened = form.peek().clone();
                editing.set(Some((Signal::new(opened.clone()), opened)));
                focus_later(&["#field-format"]);
            },
            icons::Pencil {}
            {tr.get("edit-phase")}
        }
        Dialog {
            open: Some(edited.is_some()),
            on_open_change: move |open: bool| {
                if !open {
                    let changed = editing
                        .peek()
                        .as_ref()
                        .is_some_and(|(form, opened)| form.peek().changed_since(opened));
                    discarding.close_asked(changed, close);
                }
            },
            DialogTitle { {tr.get("edit-phase-title")} }
            DialogDescription { {tr.get("edit-template-phase-description")} }
            if let Some(edited) = edited {
                form {
                    id: "edit-phase",
                    class: "dialog-form",
                    onsubmit: move |event| {
                        event.prevent_default();
                        if !edited.read().view().complete {
                            focus_first_issue("edit-phase");
                            return;
                        }
                        let mut form = form;
                        form.set(edited.peek().clone());
                        close();
                    },
                    Form { form: edited, disabled: false, level_choice: true }
                    div { class: "dialog-buttons",
                        button { class: "primary", r#type: "submit", {tr.get("save-changes")} }
                        button {
                            r#type: "button",
                            onclick: move |_| close(),
                            {tr.get("cancel")}
                        }
                    }
                }
            }
        }
        DiscardChanges { discarding, on_discard: move |()| close() }
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
