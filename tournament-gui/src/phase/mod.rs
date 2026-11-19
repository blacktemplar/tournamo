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
    form::{FormView, Path, PhaseForm},
    i18n::{Arg, Message as _, Text},
    view::{Count, MatchesOf},
};
use tournament_engine::{PhaseId, PhasePlugins};

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

/// The form of a new phase, `form` or else one of its own, and the button adding it; once added,
/// `on_added` runs. With `on_discarded`, a button beside it discards the form, which then runs.
#[component]
pub fn NewPhase(
    form: Option<Signal<PhaseForm>>,
    on_added: Option<EventHandler<()>>,
    on_discarded: Option<EventHandler<()>>,
) -> Element {
    let ctx = use_context::<Ctx>();
    let created = use_hook(|| match form {
        Some(form) => Ok(form),
        None => ctx
            .session
            .peek()
            .phase_form()
            .map(Signal::new)
            .map_err(|error| error.text()),
    });
    let actions = ctx.actions();
    let tr = ctx.translator.read();
    let form = match created {
        Ok(form) => form,
        Err(error) => {
            return rsx! {
                p { class: "error", {tr.sentence(&error)} }
            };
        }
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
    // while the dialog is open: the form and the form as it opened, or why there is none
    let mut adding = use_signal(|| None::<Result<(Signal<PhaseForm>, PhaseForm), Text>>);
    let discarding = use_discarding("#field-name");
    let mut close = move || {
        adding.set(None);
        discarding.reset();
    };
    let tr = ctx.translator.read();
    let form = adding
        .read()
        .as_ref()
        .map(|adding| adding.as_ref().map(|(form, _)| *form).map_err(Clone::clone));
    rsx! {
        button {
            class: "quiet add-phase",
            onclick: move |_| {
                let created = ctx.session.peek().phase_form().map_err(|error| error.text());
                adding.set(Some(created.map(|form| (Signal::new(form.clone()), form))));
                focus_later(&["#field-name"]);
            },
            icons::Plus {}
            {tr.get("add-phase")}
        }
        Dialog {
            role: if discarding.asked() { "alertdialog" } else { "dialog" },
            open: Some(form.is_some()),
            on_open_change: move |open: bool| {
                if !open {
                    let changed = adding.peek().as_ref().is_some_and(|adding| {
                        adding
                            .as_ref()
                            .is_ok_and(|(form, opened)| form.peek().changed_since(opened))
                    });
                    discarding.close_asked(changed, close);
                }
            },
            if discarding.asked() {
                DiscardChanges { discarding, on_discard: move |()| close() }
            } else {
                DialogTitle { {tr.get("add-phase")} }
                DialogDescription { {tr.get("add-phase-description")} }
            }
            match form {
                Some(Ok(form)) => rsx! {
                    div { class: "dialog-form", hidden: discarding.asked(),
                        NewPhase { form, on_added: move |()| close() }
                        div { class: "dialog-buttons",
                            button {
                                r#type: "button",
                                onclick: move |_| close(),
                                {tr.get("cancel")}
                            }
                        }
                    }
                },
                Some(Err(error)) => rsx! {
                    p { class: "error", {tr.sentence(&error)} }
                },
                None => rsx! {},
            }
        }
    }
}

/// Whether a dialog holding the form of a phase asks if the changes made in it are to be
/// discarded, see [`use_discarding`].
#[derive(Clone, Copy, PartialEq)]
struct Discarding {
    asked: Signal<bool>,
    /// What gets the focus back when the operator keeps editing.
    field: &'static str,
}

/// Whether a dialog holding the form of a phase asks if the changes made in it are to be
/// discarded: it does when asked to close (Escape, or a click beside it) with changes made; asked
/// again, the operator keeps editing, at `field`.
fn use_discarding(field: &'static str) -> Discarding {
    Discarding {
        asked: use_signal(|| false),
        field,
    }
}

impl Discarding {
    fn asked(self) -> bool {
        (self.asked)()
    }

    /// What the dialog does when asked to close: closes by `close` if nothing is `changed`, else
    /// asks whether to discard the changes, and keeps editing if it asked already.
    fn close_asked(mut self, changed: bool, close: impl FnOnce()) {
        if *self.asked.peek() {
            self.keep_editing();
        } else if changed {
            self.asked.set(true);
            // what is safe is what a second Enter does
            focus_later(&["#keep-editing"]);
        } else {
            close();
        }
    }

    /// Asks no more, and the form shows again.
    fn keep_editing(mut self) {
        self.asked.set(false);
        focus_later(&[self.field]);
    }

    /// Asks no more, once the dialog closes.
    fn reset(mut self) {
        if *self.asked.peek() {
            self.asked.set(false);
        }
    }
}

/// What a dialog holding the form of a phase shows while it asks whether to discard the changes
/// made in it, as `discarding` says: the question, and the buttons answering it, discarding by
/// `on_discard`, or else keeping on editing.
#[component]
fn DiscardChanges(discarding: Discarding, on_discard: EventHandler<()>) -> Element {
    let ctx = use_context::<Ctx>();
    let tr = ctx.translator.read();
    rsx! {
        DialogTitle { {tr.get("discard-changes-title")} }
        DialogDescription { {tr.get("discard-changes-description")} }
        div { class: "dialog-buttons",
            button {
                class: "primary danger",
                r#type: "button",
                onclick: move |_| on_discard.call(()),
                {tr.get("discard-changes")}
            }
            button {
                id: "keep-editing",
                r#type: "button",
                onclick: move |_| discarding.keep_editing(),
                {tr.get("keep-editing")}
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
    let count = |count: Count, [exact, range, at_least]: [&'static str; 3]| {
        let text = match count.most {
            Some(most) if most == count.least => Text::new(exact).with("count", most),
            Some(most) => Text::new(range)
                .with("least", count.least)
                .with("most", most),
            None => Text::new(at_least).with("count", count.least),
        };
        tr.text(&text)
    };
    let warning = scale().and_then(|scale| scale.ok()?.warning());
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
                        count(
                            scale.rounds,
                            [
                                "scale-rounds",
                                "scale-rounds-range",
                                "scale-rounds-at-least",
                            ],
                        ),
                    )
                    .with(
                        "matches",
                        count(
                            scale.matches,
                            [
                                "scale-matches",
                                "scale-matches-range",
                                "scale-matches-at-least",
                            ],
                        ),
                    ),
            )
        }
        Err(unfit) => tr.sentence(&unfit),
    });
    rsx! {
        p { class: "hint phase-scale", role: "status",
            {text}
            if let Some(warning) = warning {
                " "
                span { class: "warnings", {tr.sentence(&warning)} }
            }
        }
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
    // the form while the dialog is open, and the form as it opened
    let mut editing = use_signal(|| None::<(Signal<PhaseForm>, PhaseForm)>);
    let mut error = use_signal(|| None::<Text>);
    let discarding = use_discarding("#field-format");
    let mut close = move || {
        editing.set(None);
        discarding.reset();
    };
    let tr = ctx.translator.read();
    let edited = editing.read().as_ref().map(|(form, _)| *form);
    let name = ctx.session.read().name_of_phase(phase);
    let several = ctx.session.read().phases().len() > 1;
    rsx! {
        button {
            aria_label: several
                .then(|| {
                    name.as_ref()
                        .map(|name| t!(tr, "edit-phase-named", phase = tr.phase_name(name)))
                })
                .flatten(),
            onclick: move |_| {
                if let Ok(form) = ctx.session.peek().phase_form_to_edit(phase) {
                    error.set(None);
                    editing.set(Some((Signal::new(form.clone()), form)));
                    // the first field, rather than the first button, what is said about it,
                    // whose text would cover the field
                    focus_later(&["#field-format"]);
                }
            },
            icons::Pencil {}
            {tr.get("edit-phase")}
        }
        Dialog {
            role: if discarding.asked() { "alertdialog" } else { "dialog" },
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
            if discarding.asked() {
                DiscardChanges { discarding, on_discard: move |()| close() }
            } else {
                DialogTitle { {tr.get("edit-phase-title")} }
                DialogDescription { {tr.get("edit-phase-description")} }
            }
            if let Some(form) = edited {
                form {
                    id: "edit-phase",
                    class: "dialog-form",
                    hidden: discarding.asked(),
                    onsubmit: move |event| {
                        event.prevent_default();
                        if !form.read().view().complete {
                            error.set(None);
                            focus_first_issue("edit-phase");
                            return;
                        }
                        match ctx.try_act(|session| session.edit_phase(phase, &form.read())) {
                            Ok(edited) => {
                                close();
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
                        button {
                            r#type: "button",
                            onclick: move |_| close(),
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
    let mut update_error = use_signal(|| None::<Text>);
    let FormView {
        fields,
        issues,
        following_suggestions,
        entry,
        outdated,
        ..
    } = form.read().view();
    let outdated = (!outdated.is_empty()).then(|| {
        t!(
            tr,
            "outdated-plugins",
            plugins = Arg::Plugins(
                outdated
                    .iter()
                    .map(|plugin| (plugin.id, plugin.label))
                    .collect()
            )
        )
    });
    rsx! {
        div { class: "phase-form",
            if let Some(outdated) = outdated {
                div { class: "notice",
                    p { id: "outdated-plugins", "{outdated}" }
                    button {
                        r#type: "button",
                        disabled,
                        aria_describedby: "outdated-plugins",
                        onclick: move |_| update_error.set(update(form)),
                        {tr.get("update-plugins")}
                    }
                    if let Some(error) = update_error() {
                        Issues { id: "update-issues", issues: vec![error] }
                    }
                }
            }
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
                        error
                            .set(
                                form
                                    .write()
                                    .apply_suggestions()
                                    .err()
                                    .map(|error| { PhaseDraftError::Configuration(error).text() }),
                            );
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

/// Updates the plugins of `form` to their latest versions and moves the focus from the button,
/// which goes with the notice, to the format; or else why not.
fn update(mut form: Signal<PhaseForm>) -> Option<Text> {
    let Err(error) = form.write().update_plugins() else {
        let format = fields::id(&Path::ROOT.field(PhasePlugins::FORMAT));
        let selectors = [format!("#edit-phase #{format}"), format!("#{format}")];
        focus_later(&selectors.each_ref().map(String::as_str));
        return None;
    };
    Some(PhaseDraftError::Configuration(error).text())
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
