//! The phase: the form configuring it, generated from the [`FormView`] of a [`PhaseForm`], a
//! summary of how it is played, and the dialog changing it once added.

mod entry;
mod fields;
mod lists;
mod preset;
mod summary;

use dioxus::prelude::*;
use tournament_app::{
    error::PhaseDraftError,
    form::{FormView, PhaseForm},
    i18n::{Message as _, Text},
    view::{Count, MatchesOf},
};
use tournament_engine::PhaseId;

use self::{
    entry::Entry,
    fields::{EditedForm, Fields, Issues},
    summary::Summary,
};
pub use self::{preset::HowPlayed, summary::answer};
use crate::{
    announcement::Announcement,
    competition::Ctx,
    components::dialog::{Dialog, DialogDescription, DialogTitle},
    focus::focus_later,
    i18n::t,
    icons,
    phases::{StartPhaseButton, phase_id_text, sources_text},
};

/// The form of a new phase, and the button adding it; once added, `on_added` runs. With
/// `on_discarded`, a button beside it discards the form, which then runs.
#[component]
pub fn NewPhase(
    on_added: Option<EventHandler<()>>,
    on_discarded: Option<EventHandler<()>>,
) -> Element {
    let ctx = use_context::<Ctx>();
    let created = use_hook(|| {
        ctx.session
            .peek()
            .phase_form()
            .map(Signal::new)
            .map_err(|error| error.text())
    });
    let actions = ctx.actions();
    let tr = ctx.translator.read();
    let form = match created {
        Ok(form) => form,
        Err(error) => return rsx! { p { class: "error", {tr.sentence(&error)} } },
    };
    rsx! {
        form {
            id: "new-phase",
            onsubmit: move |event| {
                event.prevent_default();
                if !form.read().view().complete {
                    focus_first_issue("new-phase");
                    return;
                }
                if ctx.act(|session| session.add_phase(&form.read())).is_some() {
                    ctx.announce(Announcement::AddedPhase);
                    match on_added {
                        Some(on_added) => on_added.call(()),
                        // the form is gone, and with it what had the focus
                        None => focus_later(&["#start:enabled", "#phase"]),
                    }
                }
            },
            Form { form, disabled: !actions.add_phase }
            PhaseScale { form }
            div { class: "form-actions",
                if let Some(on_discarded) = on_discarded {
                    button {
                        id: "discard-phase",
                        r#type: "button",
                        onclick: move |_| on_discarded.call(()),
                        {tr.get("discard-phase")}
                    }
                }
                button {
                    class: "primary",
                    r#type: "submit",
                    disabled: !actions.add_phase,
                    {tr.get("add-phase")}
                }
            }
        }
    }
}

/// A button opening a dialog with the form of a new phase, for a competition that has started.
#[component]
pub fn AddPhaseDialog() -> Element {
    let ctx = use_context::<Ctx>();
    let mut open = use_signal(|| false);
    let tr = ctx.translator.read();
    rsx! {
        button {
            class: "quiet add-phase",
            onclick: move |_| {
                open.set(true);
                focus_later(&["#field-name"]);
            },
            icons::Plus {}
            {tr.get("add-phase")}
        }
        Dialog {
            open: Some(open()),
            on_open_change: move |is_open: bool| open.set(is_open),
            DialogTitle { {tr.get("add-phase")} }
            DialogDescription { {tr.get("add-phase-description")} }
            if open() {
                div { class: "dialog-form",
                    NewPhase { on_added: move |()| open.set(false) }
                    div { class: "dialog-buttons",
                        button { r#type: "button", onclick: move |_| open.set(false),
                            {tr.get("cancel")}
                        }
                    }
                }
            }
        }
    }
}

/// How many rounds and matches the phase takes with the entrants its sources take as many as
/// there are now, or why it cannot form its teams of them, until it starts: the phase of `form`,
/// or else the phase `phase`. Said as it
/// changes, since it is what a format is chosen by.
#[component]
fn PhaseScale(form: Option<Signal<PhaseForm>>, phase: Option<PhaseId>) -> Element {
    let ctx = use_context::<Ctx>();
    // played out anew only when the phase or the entrants change, see `Session::scale`
    let scale = use_memo(move || {
        let session = ctx.session.read();
        let form = match (form, phase) {
            (Some(form), _) => form.read().clone(),
            (None, Some(phase)) => {
                let started = session
                    .competition()
                    .phase(&phase)
                    .is_some_and(|phase| phase.is_started());
                if started {
                    return None;
                }
                session.phase_form_to_edit(phase).ok()?
            }
            (None, None) => return None,
        };
        match session.unfit(&form) {
            Some(unfit) => Some(Err(unfit)),
            None => session.scale(&form).map(Ok),
        }
    });
    let tr = ctx.translator.read();
    let count = |count: Count, exact: &'static str, range: &'static str| {
        let text = if count.least == count.most {
            Text::new(exact).with("count", count.least)
        } else {
            Text::new(range)
                .with("least", count.least)
                .with("most", count.most)
        };
        tr.text(&text)
    };
    let text = scale().map(|scale| match scale {
        Ok(scale) => {
            let key = if scale.teams == scale.entrants {
                "phase-scale"
            } else {
                "phase-scale-teams"
            };
            tr.text(
                &Text::new(key)
                    .with("entrants", scale.entrants)
                    .with("teams", scale.teams)
                    .with(
                        "rounds",
                        count(scale.rounds, "scale-rounds", "scale-rounds-range"),
                    )
                    .with(
                        "matches",
                        count(scale.matches, "scale-matches", "scale-matches-range"),
                    ),
            )
        }
        Err(unfit) => tr.sentence(&unfit),
    });
    rsx! {
        p { class: "hint phase-scale", role: "status", {text} }
    }
}

/// How the phase `phase` is played, where it takes its entrants from where the competition has
/// several phases, and the buttons changing and removing it.
#[component]
pub fn PhaseSummary(phase: PhaseId) -> Element {
    let ctx = use_context::<Ctx>();
    let Ok(form) = ctx.session.read().phase_form_to_edit(phase) else {
        return rsx! {};
    };
    let actions = ctx.actions();
    let view = form.view();
    let columns = summary::columns(&ctx.translator.read(), &view);
    rsx! {
        Summary { fields: view.summary(), columns }
        PhaseScale { phase }
        if actions.edit_phase {
            div { class: "phase-actions",
                EditPhase { phase }
                RemovePhase { phase }
            }
        }
    }
}

/// A phase that has not started after the start of the competition: how it is played, where its
/// entrants come from, and the button starting it.
#[component]
pub fn PendingPhase(phase: PhaseId) -> Element {
    let ctx = use_context::<Ctx>();
    let session = ctx.session.read();
    let phases = session.phases();
    let Some(overview) = phases.iter().find(|overview| overview.id == phase).cloned() else {
        return rsx! {};
    };
    drop(session);
    let tr = ctx.translator.read();
    rsx! {
        div { class: "columns play",
            div { class: "column wide",
                section { class: "card", aria_labelledby: "phase",
                    h2 { id: "phase", tabindex: "-1", {tr.phase_name(&overview.name)} }
                    p { class: "sources", {sources_text(&tr, &phases, &overview.sources)} }
                    PhaseSummary { phase }
                    div { class: "start",
                        StartPhaseButton { phase, primary: true }
                    }
                }
            }
        }
    }
}

/// The button editing the phase `phase`, and the dialog it opens: the form of the phase as it is.
#[component]
fn EditPhase(phase: PhaseId) -> Element {
    let ctx = use_context::<Ctx>();
    // the form while the dialog is open
    let mut editing = use_signal(|| None::<Signal<PhaseForm>>);
    let mut error = use_signal(|| None::<Text>);
    let tr = ctx.translator.read();
    let open = editing.read().is_some();
    let name = ctx.session.read().name_of_phase(phase);
    let several = ctx.session.read().phases().len() > 1;
    rsx! {
        button {
            aria_label: several.then(|| {
                name.as_ref().map(|name| t!(tr, "edit-phase-named", phase = tr.phase_name(name)))
            }).flatten(),
            onclick: move |_| {
                if let Ok(form) = ctx.session.peek().phase_form_to_edit(phase) {
                    error.set(None);
                    editing.set(Some(Signal::new(form)));
                    // the first field, rather than the first button, what is said about it,
                    // whose text would cover the field
                    focus_later(&["#field-format"]);
                }
            },
            icons::Pencil {}
            {tr.get("edit-phase")}
        }
        Dialog {
            open: Some(open),
            on_open_change: move |open: bool| {
                if !open {
                    editing.set(None);
                }
            },
            DialogTitle { {tr.get("edit-phase-title")} }
            DialogDescription { {tr.get("edit-phase-description")} }
            if let Some(form) = editing() {
                form {
                    id: "edit-phase",
                    class: "dialog-form",
                    onsubmit: move |event| {
                        event.prevent_default();
                        if !form.read().view().complete {
                            error.set(None);
                            focus_first_issue("edit-phase");
                            return;
                        }
                        match ctx.try_act(|session| session.edit_phase(phase, &form.read())) {
                            Ok(edited) => {
                                editing.set(None);
                                if edited {
                                    let results = ctx
                                        .session
                                        .read()
                                        .rounds(MatchesOf::phase(phase))
                                        .iter()
                                        .map(|summary| summary.matches - summary.open)
                                        .sum();
                                    ctx.announce(Announcement::EditedPhase(results));
                                }
                            }
                            Err(refused) => error.set(Some(refused)),
                        }
                    },
                    Form { form, disabled: false }
                    // next to the buttons, which stay in sight below a long form
                    div { class: "dialog-buttons",
                        p { class: "error", role: "alert",
                            if let Some(error) = error() {
                                {tr.sentence(&error)}
                            }
                        }
                        button { class: "primary", r#type: "submit", {tr.get("save-changes")} }
                        button { r#type: "button", onclick: move |_| editing.set(None),
                            {tr.get("cancel")}
                        }
                    }
                }
            }
        }
    }
}

/// The button removing the phase `phase`, without a question, since undo brings it back:
/// `aria-disabled` with the reason while it cannot be removed.
#[component]
fn RemovePhase(phase: PhaseId) -> Element {
    let ctx = use_context::<Ctx>();
    let refusal = ctx
        .session
        .read()
        .remove_phase_refusal(phase)
        .map(|refusal| refusal.text());
    let name = ctx.session.read().name_of_phase(phase);
    let several = ctx.session.read().phases().len() > 1;
    let tr = ctx.translator.read();
    let described = format!("remove-refusal-{}", phase_id_text(phase));
    let disabled = refusal.is_some();
    rsx! {
        button {
            aria_disabled: disabled.then_some("true"),
            aria_describedby: disabled.then(|| described.clone()),
            aria_label: several
                .then(|| {
                    name.as_ref()
                        .map(|name| t!(tr, "remove-phase-named", phase = tr.phase_name(name)))
                })
                .flatten(),
            onclick: move |_| {
                if disabled {
                    return;
                }
                if ctx.act(|session| session.remove_phase(phase)).is_some() {
                    ctx.announce(Announcement::RemovedPhase);
                    focus_later(&["#phases", "#phase"]);
                }
            },
            icons::Trash {}
            {tr.get("remove-phase")}
        }
        if let Some(refusal) = &refusal {
            " "
            span { id: "{described}", class: "hint", {tr.sentence(refusal)} }
        }
    }
}

/// The fields of `form`, which the operator changes unless `disabled`, with what is wrong with
/// the answers as a whole.
#[component]
fn Form(form: Signal<PhaseForm>, disabled: bool) -> Element {
    let ctx = use_context::<Ctx>();
    let tr = ctx.translator.read();
    let mut error = use_signal(|| None::<Text>);
    let FormView {
        fields,
        issues,
        following_suggestions,
        entry,
        ..
    } = form.read().view();
    rsx! {
        div { class: "phase-form",
            Entry { form, entry, disabled }
            Fields { form: EditedForm::Phase(form), fields, disabled }
            p { role: "status",
                if following_suggestions {
                    {tr.get("following-suggestions")}
                } else {
                    {tr.get("customized-settings")}
                }
            }
            button {
                r#type: "button",
                disabled,
                aria_disabled: following_suggestions,
                onclick: move |_| {
                    if !disabled && !form.read().view().following_suggestions {
                        error.set(form.write().apply_suggestions().err().map(|error| {
                            PhaseDraftError::Configuration(error).text()
                        }));
                    }
                },
                {tr.get("apply-suggested-ranking")}
            }
            if let Some(error) = error() {
                Issues { id: "suggestion-issues", issues: vec![error] }
            }
            Issues { id: "phase-issues", issues }
        }
    }
}

/// Moves the focus to the first field marked as invalid or list of issues in the form whose element
/// has the id `form`, whichever comes first, scrolled into the middle of the screen, so that the
/// operator sees and hears what keeps the phase from being played.
fn focus_first_issue(form: &str) {
    _ = document::eval(&format!(
        r##"
        const issue = document.querySelector("#{form} [aria-invalid=true], #{form} .issues");
        issue?.scrollIntoView({{ block: "center" }});
        issue?.focus({{ preventScroll: true }});
        "##
    ));
}
