//! The basic view of the phases before the start: for the whole competition its match format and
//! whether it is played by drawn pairs, then each phase with only what anyone setting up a
//! competition decides, each change made at once.

use dioxus::prelude::*;
use tournament_app::{
    form::{BasicFields, End, PhaseForm},
    session::{Dyp, MatchFormats},
    view::PhaseName,
};
use tournament_engine::{PhaseId, PhasePlugins, plugin_factory::schema::Level};

use super::fields::{EditedForm, Fields, Issues};
use crate::{competition::Ctx, i18n::t, phases::phase_id_text};

/// The settings of the whole competition, then the phases, each as the basic view shows it.
#[component]
pub fn BasicView() -> Element {
    let ctx = use_context::<Ctx>();
    let phases = ctx.session.read().phases();
    rsx! {
        CompetitionSettings {}
        ol { id: "phases", class: "phases numbered basic", tabindex: "-1",
            for phase in phases.iter() {
                li { key: "{phase.id:?}",
                    BasicPhase { phase: phase.id }
                }
            }
        }
    }
}

/// The match format of every phase not started, and whether the phases taking the registration
/// are played by drawn pairs, where that is offered.
#[component]
fn CompetitionSettings() -> Element {
    let ctx = use_context::<Ctx>();
    let actions = ctx.actions();
    let match_formats = ctx.session.read().match_formats();
    let dyp = ctx.session.read().dyp();
    let tr = ctx.translator.read();
    rsx! {
        div { class: "competition-settings",
            match match_formats {
                Some(MatchFormats::Same(form)) => rsx! {
                    MatchFormat { initial: Initial(*form) }
                },
                Some(MatchFormats::Mixed) => rsx! {
                    p { id: "mixed-match-formats",
                        span { class: "label", {tr.get("match-format")} }
                        " "
                        {tr.get("mixed-match-formats")}
                    }
                },
                None => rsx! {},
            }
            match dyp {
                Some(Dyp::Other) => rsx! {
                    p { id: "other-teams",
                        span { class: "label", {tr.get("dyp")} }
                        " "
                        {tr.get("other-teams")}
                    }
                },
                Some(dyp) => rsx! {
                    div { class: "field",
                        label {
                            input {
                                r#type: "checkbox",
                                id: "dyp",
                                checked: dyp == Dyp::On,
                                disabled: !actions.edit_phase,
                                onchange: move |event| {
                                    let on = event.checked();
                                    ctx.act(|session| session.set_dyp(on));
                                },
                            }
                            " "
                            {tr.get("dyp")}
                        }
                    }
                },
                None => rsx! {},
            }
        }
    }
}

/// A form as a property of a component, which only its first value starts, so that any two
/// compare equal.
#[derive(Clone)]
struct Initial(PhaseForm);

impl PartialEq for Initial {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

/// The match format of every phase, with the basic fields of its plugin, set on every phase not
/// started as soon as it changes and fits.
#[component]
fn MatchFormat(initial: Initial) -> Element {
    let ctx = use_context::<Ctx>();
    let actions = ctx.actions();
    let mut form = use_signal(|| initial.0.clone());
    let answer = |form: &PhaseForm| {
        form.draft()
            .answers()
            .get(PhasePlugins::MATCH_FORMAT)
            .cloned()
    };
    // set on every phase, once the operator changed it and every number typed is taken
    use_effect(move || {
        let edited = form.read().clone();
        let field = edited.match_format_field(Level::Basic);
        let taken = field.is_some_and(|field| field.issues.is_empty()) && edited.view().complete;
        let changed = match ctx.session.peek().match_formats() {
            Some(MatchFormats::Same(shown)) => answer(&shown) != answer(&edited),
            _ => false,
        };
        if taken && changed {
            ctx.act(|session| session.set_match_format(&edited));
        }
    });
    // what an undo, or another tab, made of it
    use_effect(move || {
        if let Some(MatchFormats::Same(shown)) = ctx.session.read().match_formats()
            && answer(&shown) != answer(&form.peek())
        {
            form.set(*shown);
        }
    });
    let field = form.read().match_format_field(Level::Basic);
    rsx! {
        if let Some(field) = field {
            Fields {
                form: EditedForm::new(form),
                fields: vec![field],
                disabled: !actions.edit_phase,
            }
        }
    }
}

/// The phase `phase` in the basic view: its name, where its entrants come from, its format and
/// the basic fields of its plugins, each change made at once. What it shows is decided as it
/// opens, see [`PhaseForm::basic_fields`].
#[component]
fn BasicPhase(phase: PhaseId) -> Element {
    let ctx = use_context::<Ctx>();
    let actions = ctx.actions();
    // mounted only for a phase of the competition, by its key
    let mut form = use_signal(|| {
        ctx.session
            .peek()
            .phase_form_to_edit(phase)
            .expect("the basic view shows the phases of the competition")
    });
    let basic: BasicFields = use_hook(|| form.peek().basic_fields());
    // made once complete and changed
    use_effect(move || {
        let edited = form.read().clone();
        if !edited.view().complete {
            return;
        }
        let changed = ctx
            .session
            .peek()
            .phase_form_to_edit(phase)
            .is_ok_and(|current| edited.changed_since(&current));
        if changed {
            ctx.act(|session| session.edit_phase(phase, &edited));
        }
    });
    // what an undo, or another tab, made of it
    use_effect(move || {
        if let Ok(current) = ctx.session.read().phase_form_to_edit(phase)
            && current.changed_since(&form.peek())
        {
            form.set(current);
        }
    });
    let tr = ctx.translator.read();
    let name = ctx.session.read().name_of_phase(phase);
    let view = form.read().view_basic(basic);
    let entry = view.entry.clone();
    let name_of = |id: Option<PhaseId>| -> Option<PhaseName> {
        entry
            .choices
            .iter()
            .find(|choice| choice.phase == id)
            .and_then(|choice| choice.name.clone())
    };
    let disabled = !actions.edit_phase;
    rsx! {
        if let Some(name) = &name {
            h3 { {tr.phase_name(name)} }
        }
        if basic.sources {
            for (index, row) in entry.rows.iter().enumerate() {
                p { key: "{index}", class: "basic-source",
                    label {
                        if row.from == "1" {
                            {tr.get("basic-best")}
                        } else {
                            {t!(tr, "basic-places", from = row.from.as_str())}
                        }
                        " "
                        input {
                            r#type: "text",
                            class: "number",
                            inputmode: "numeric",
                            autocomplete: "off",
                            disabled,
                            value: "{row.to}",
                            onchange: move |event| {
                                form.write().enter_place(index, End::To, &event.value());
                            },
                        }
                        " "
                        {
                            match name_of(row.of) {
                                Some(of) => t!(tr, "basic-of-phase", phase = tr.phase_name(& of)),
                                None => tr.get("basic-of-registration"),
                            }
                        }
                    }
                    if let Some(rejected) = &row.to_rejected {
                        " "
                        span { class: "error", {tr.sentence(rejected)} }
                    }
                    if let Some(issue) = &row.issue {
                        " "
                        span { class: "error", {tr.sentence(issue)} }
                    }
                }
            }
        }
        Fields { form: EditedForm::of_phase(form, phase), fields: view.fields, disabled }
        Issues {
            id: format!("basic-issues-{}", phase_id_text(phase)),
            issues: view.issues,
        }
    }
}
