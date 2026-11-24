//! The basic view of the phases before the start: for the whole competition its match format and
//! whether it is played by drawn pairs, then each phase with only what anyone setting up a
//! competition decides, each change made at once.

use dioxus::prelude::*;
use tournament_app::{
    form::{BasicFields, End, Mark, PhaseForm, TemplateForm},
    i18n::{Message as _, Text},
    session::{Dyp, MatchFormats},
    view::PhaseName,
};
use tournament_engine::{PhaseId, PhasePlugins, plugin_factory::schema::Level};

use super::{
    HiddenSettings,
    fields::{EditedForm, Fields, Issues, MarkDot, OfTemplate},
};
use crate::{competition::Ctx, i18n::t, phases::phase_id_text};

/// The settings of the whole competition, then the phases, each as the basic view shows it.
#[component]
pub fn BasicView() -> Element {
    let ctx = use_context::<Ctx>();
    let phases = ctx.session.read().phases();
    let actions = ctx.actions();
    rsx! {
        CompetitionSettings { phases: Phases::Added, disabled: !actions.edit_phase }
        ol { id: "phases", class: "phases numbered basic", tabindex: "-1",
            for phase in phases.iter() {
                li { key: "{phase.id:?}",
                    BasicPhase { phase: phase.id }
                }
            }
        }
    }
}

/// The phases the settings of the whole competition are set on: those added and not started, or
/// the forms of a template before they are added.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Phases {
    Added,
    Template(Signal<TemplateForm>),
}

impl Phases {
    /// Their match format, subscribing to what it is read from.
    fn match_formats(self, ctx: Ctx) -> Option<MatchFormats> {
        match self {
            Self::Added => ctx.session.read().match_formats(),
            Self::Template(template) => Some(template.read().match_formats()),
        }
    }

    /// Their match format, subscribing to nothing.
    fn match_formats_now(self, ctx: Ctx) -> Option<MatchFormats> {
        match self {
            Self::Added => ctx.session.peek().match_formats(),
            Self::Template(template) => Some(template.peek().match_formats()),
        }
    }

    /// Whether they are played by drawn pairs, subscribing to what it is read from.
    fn dyp(self, ctx: Ctx) -> Option<Dyp> {
        match self {
            Self::Added => ctx.session.read().dyp(),
            Self::Template(template) => template.read().dyp(),
        }
    }

    /// Whether they are of a template, which what differs is compared with, see [`OfTemplate`].
    fn of_template(self, ctx: Ctx) -> bool {
        match self {
            Self::Added => ctx
                .session
                .peek()
                .based_on(&ctx.translator.peek())
                .is_some(),
            Self::Template(_) => true,
        }
    }

    /// Whether their teams differ from what is expected of them, see [`Self::dyp`].
    fn dyp_differs(self, ctx: Ctx) -> bool {
        match self {
            Self::Added => ctx.session.read().dyp_differs(),
            Self::Template(template) => template.read().dyp_differs(),
        }
    }

    /// Sets the match format `form` answers on them; why not, if refused, for `error` to show
    /// where the session does not.
    fn set_match_format(self, ctx: Ctx, form: &PhaseForm, mut error: Signal<Option<Text>>) {
        match self {
            Self::Added => {
                ctx.act(|session| session.set_match_format(form));
            }
            Self::Template(mut template) => {
                let set = template.write().set_match_format(form);
                error.set(set.err().map(|refused| refused.text()));
            }
        }
    }

    /// Plays them by drawn pairs if `on`; why not, if refused, for `error` to show where the
    /// session does not.
    fn set_dyp(self, ctx: Ctx, on: bool, mut error: Signal<Option<Text>>) {
        match self {
            Self::Added => {
                ctx.act(|session| session.set_dyp(on));
            }
            Self::Template(mut template) => {
                let set = template.write().set_dyp(on);
                error.set(set.err().map(|refused| refused.text()));
            }
        }
    }
}

/// The match format of every phase of `phases`, and whether those taking the registration are
/// played by drawn pairs, where that is offered.
#[component]
pub(super) fn CompetitionSettings(phases: Phases, disabled: bool) -> Element {
    let ctx = use_context::<Ctx>();
    let error = use_signal(|| None::<Text>);
    use_context_provider(|| OfTemplate(phases.of_template(ctx)));
    let match_formats = phases.match_formats(ctx);
    let dyp_mark = phases.dyp_differs(ctx).then_some(Mark::Differs);
    let dyp = phases.dyp(ctx);
    let tr = ctx.translator.read();
    rsx! {
        div { class: "competition-settings",
            match match_formats {
                Some(MatchFormats::Same(form)) => rsx! {
                    MatchFormat {
                        initial: Initial(*form),
                        phases,
                        disabled,
                        error,
                    }
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
                                disabled,
                                onchange: move |event| phases.set_dyp(ctx, event.checked(), error),
                            }
                            " "
                            {tr.get("dyp")}
                        }
                        MarkDot { id: "dyp-mark", subject: tr.get("dyp"), mark: dyp_mark }
                    }
                },
                None => rsx! {},
            }
            if let Some(error) = error() {
                p { class: "error", role: "alert", {tr.sentence(&error)} }
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

/// The match format of every phase of `phases`, with the basic fields of its plugin, set on each
/// as soon as it changes and fits.
#[component]
fn MatchFormat(
    initial: Initial,
    phases: Phases,
    disabled: bool,
    error: Signal<Option<Text>>,
) -> Element {
    let ctx = use_context::<Ctx>();
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
        let changed = match phases.match_formats_now(ctx) {
            Some(MatchFormats::Same(shown)) => answer(&shown) != answer(&edited),
            _ => false,
        };
        if taken && changed {
            phases.set_match_format(ctx, &edited, error);
        }
    });
    // what an undo, another tab or a refusal made of it
    use_effect(move || {
        if let Some(MatchFormats::Same(shown)) = phases.match_formats(ctx)
            && answer(&shown) != answer(&form.peek())
        {
            form.set(*shown);
        }
    });
    let field = form.read().match_format_field(Level::Basic);
    rsx! {
        if let Some(field) = field {
            Fields { form: EditedForm::new(form), fields: vec![field], disabled }
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
    rsx! {
        if let Some(name) = &name {
            h3 { {tr.phase_name(name)} }
        }
        BasicFieldsOf {
            form,
            basic,
            of: phase,
            disabled: !actions.edit_phase,
        }
    }
}

/// The phase `form` configures as the basic view shows it, laid out by `basic`: where its
/// entrants come from, its fields, what is wrong with it, and the settings it hides that differ.
/// The ids of its elements name the phase `of`.
#[component]
pub(super) fn BasicFieldsOf(
    form: Signal<PhaseForm>,
    basic: BasicFields,
    of: PhaseId,
    disabled: bool,
) -> Element {
    let ctx = use_context::<Ctx>();
    use_context_provider(|| OfTemplate(form.peek().of_template()));
    let tr = ctx.translator.read();
    let phase = of;
    let view = form.read().view_basic(basic);
    let entry = view.entry.clone();
    let name_of = |id: Option<PhaseId>| -> Option<PhaseName> {
        entry
            .choices
            .iter()
            .find(|choice| choice.phase == id)
            .and_then(|choice| choice.name.clone())
    };
    rsx! {
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
                    MarkDot {
                        id: format!("basic-source-{}-{index}-mark", phase_id_text(phase)),
                        subject: t!(tr, "source-of", number = index + 1),
                        mark: row.mark,
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
        Fields {
            form: EditedForm::of_phase(form, phase),
            fields: view.fields,
            disabled,
            followed: view.followed,
        }
        Issues {
            id: format!("basic-issues-{}", phase_id_text(phase)),
            issues: view.issues,
        }
        HiddenSettings {
            id: format!("hidden-settings-{}", phase_id_text(phase)),
            hidden: view.hidden,
        }
    }
}
