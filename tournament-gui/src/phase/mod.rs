//! The phase: the form configuring it, generated from the [`FormView`] of a [`PhaseForm`], a
//! summary of how it is played, and the dialog changing it once added.

mod fields;
mod lists;
mod summary;

use dioxus::prelude::*;
use tournament_app::{
    error::PhaseDraftError,
    form::{FormView, PhaseForm},
    i18n::{Message as _, Text},
    view::Count,
};

pub use self::summary::answer;
use self::{
    fields::{Fields, Issues},
    summary::Summary,
};
use crate::{
    announcement::Announcement,
    competition::Ctx,
    components::dialog::{Dialog, DialogDescription, DialogTitle},
    focus::focus_later,
    i18n::t,
};

/// The form of a new phase, and the button adding it.
#[component]
pub fn NewPhase() -> Element {
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
                if ctx.act(|session| session.add_phase(form.read().draft())).is_some() {
                    ctx.announce(Announcement::AddedPhase);
                    // the form is gone, and with it what had the focus
                    focus_later(&["#start:enabled", "#phase"]);
                }
            },
            Form { form, disabled: !actions.add_phase }
            PhaseScale { form }
            div { class: "form-actions",
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

/// How many rounds and matches the phase takes with the entrants entered, until the start, which
/// shows them: the phase of `form`, or else the phase added. Said as it changes, since it is what
/// a format is chosen by.
#[component]
fn PhaseScale(form: Option<Signal<PhaseForm>>) -> Element {
    let ctx = use_context::<Ctx>();
    // played out anew only when the phase or the entrants change, see `Session::scale`
    let scale = use_memo(move || {
        let session = ctx.session.read();
        if session.competition().is_started() {
            return None;
        }
        let scale = match form {
            Some(form) => session.scale(form.read().draft()),
            None => session.scale(&session.phase_draft_to_edit().ok()?),
        };
        scale.map(|scale| (session.competition().entrants().len(), scale))
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
    let text = scale().map(|(entrants, scale)| {
        t!(
            tr,
            "phase-scale",
            entrants = entrants,
            rounds = count(scale.rounds, "scale-rounds", "scale-rounds-range"),
            matches = count(scale.matches, "scale-matches", "scale-matches-range"),
        )
    });
    rsx! {
        p { id: "phase-scale", class: "hint", role: "status", {text} }
    }
}

/// How the phase is played, and a button opening the dialog that changes it.
#[component]
pub fn PhaseSummary() -> Element {
    let ctx = use_context::<Ctx>();
    let Ok(form) = ctx.session.read().phase_form_to_edit() else {
        return rsx! {};
    };
    let actions = ctx.actions();
    rsx! {
        Summary { fields: form.view().fields }
        PhaseScale {}
        if actions.edit_phase {
            EditPhase {}
        }
    }
}

/// The button editing the phase, and the dialog it opens: the form of the phase as it is played.
#[component]
fn EditPhase() -> Element {
    let ctx = use_context::<Ctx>();
    // the form while the dialog is open
    let mut editing = use_signal(|| None::<Signal<PhaseForm>>);
    let mut error = use_signal(|| None::<Text>);
    let tr = ctx.translator.read();
    let open = editing.read().is_some();
    rsx! {
        button {
            onclick: move |_| {
                if let Ok(form) = ctx.session.peek().phase_form_to_edit() {
                    error.set(None);
                    editing.set(Some(Signal::new(form)));
                    // the first field, rather than the first button, what is said about it,
                    // whose text would cover the field
                    focus_later(&["#field-format"]);
                }
            },
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
                        match ctx.try_act(|session| session.edit_phase(form.read().draft())) {
                            Ok(edited) => {
                                editing.set(None);
                                if edited {
                                    let results = ctx
                                        .session
                                        .read()
                                        .rounds()
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
                    p { class: "error", role: "alert",
                        if let Some(error) = error() {
                            {tr.sentence(&error)}
                        }
                    }
                    div { class: "dialog-buttons",
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
        ..
    } = form.read().view();
    rsx! {
        div { class: "phase-form",
            Fields { form, fields, disabled }
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
