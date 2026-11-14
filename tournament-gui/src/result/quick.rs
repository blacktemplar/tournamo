//! The values of a result entered in the row below its pairing.

use dioxus::prelude::*;
use tournament_app::{
    i18n::{PluginText, Text},
    result_form::{QuickEntry, QuickField, ResultForm},
    view::MatchRow,
};
use tournament_engine::{MatchResult, Team};

use super::{EnterResult, value::ValueInput, with_parts_won};
use crate::{
    announcement::Announcement,
    capitalized,
    competition::Ctx,
    focus::{move_focus, use_focus},
    i18n::t,
    play::Focus,
    team_name,
};

/// The values of the result of the match `row` in one row, `quick` of `form`, reported by Enter or
/// its button as they suggest, with the button opening the dialog with them for anything more.
#[component]
pub(super) fn QuickResult(
    row: MatchRow,
    form: Signal<Option<ResultForm>>,
    quick: QuickEntry,
    focus: Signal<Option<Focus>>,
    after_report: Focus,
) -> Element {
    let ctx = use_context::<Ctx>();
    let mut error = use_signal(|| None::<Text>);
    let id = row.id;
    // the first value, where a result is entered first
    let on_mounted = EventHandler::new(use_focus(focus, Focus::Report(id)));
    // only reported by the tab that changes the competition
    let disabled = !ctx.changes();
    let tr = ctx.translator.read();
    let (a, b) = (team_name(&tr, &row.a), team_name(&tr, &row.b));
    let entered = quick
        .fields
        .iter()
        .any(|quick| quick.field.values.a.is_some() || quick.field.values.b.is_some());
    let outcome = match quick.result {
        _ if !entered => None,
        Some(MatchResult::AWins) => Some(t!(tr, "wins", team = a.as_str())),
        Some(MatchResult::BWins) => Some(t!(tr, "wins", team = b.as_str())),
        Some(MatchResult::Draw) => Some(tr.get("draw")),
        None => Some(tr.get("result-open")),
    }
    .map(|outcome| with_parts_won(&tr, outcome, quick.parts_won));
    let match_format = quick.match_format;
    let form_id = format!("quick-result-{}", row.number);
    rsx! {
        // the dialog is a form of its own, so beside this form rather than in it
        div { class: "quick-result",
            form {
                id: "{form_id}",
                class: "quick-values",
                onsubmit: move |event| {
                    event.prevent_default();
                    let Some(entered) = form.read().clone() else {
                        return;
                    };
                    match ctx.try_act(|session| session.report_form(&entered)) {
                        Ok(reported) => {
                            ctx.announce(Announcement::Reported(reported));
                            move_focus(focus, after_report);
                        }
                        Err(refused) => error.set(Some(refused)),
                    }
                },
                for (index, quick) in quick.fields.into_iter().enumerate() {
                    QuickValues {
                        key: "{index}",
                        form,
                        quick,
                        match_format,
                        number: row.number,
                        disabled,
                        a: a.clone(),
                        b: b.clone(),
                        on_mounted: (index == 0).then_some(on_mounted),
                        on_input: move |()| error.set(None),
                    }
                }
            }
            div { class: "quick-buttons",
                button {
                    class: "primary small",
                    r#type: "submit",
                    form: "{form_id}",
                    disabled,
                    aria_label: t!(tr, "report-match", number = row.number),
                    {tr.get("report")}
                }
                EnterResult {
                    row: row.clone(),
                    focus,
                    after_report,
                    from: form,
                }
            }
            // read out as it changes with the values
            output { class: "quick-outcome", {outcome} }
            // which the values not taken name as their description
            if !quick.issues.is_empty() {
                div {
                    class: "issues",
                    id: "quick-issues-{row.number}",
                    role: "status",
                    for issue in &quick.issues {
                        p { {tr.sentence(issue)} }
                    }
                }
            }
            for warning in &quick.warnings {
                p { class: "warnings", role: "status", {tr.sentence(warning)} }
            }
            p { class: "error", role: "alert",
                if let Some(error) = error() {
                    {tr.sentence(&error)}
                }
            }
        }
    }
}

/// The values of `quick`, a field of `form`, one for each side, of the match `number` between `a`
/// and `b`; the first value of a match is mounted with `on_mounted`.
#[component]
fn QuickValues(
    form: Signal<Option<ResultForm>>,
    quick: QuickField,
    match_format: &'static str,
    number: usize,
    disabled: bool,
    a: String,
    b: String,
    on_mounted: Option<EventHandler<MountedEvent>>,
    on_input: EventHandler<()>,
) -> Element {
    let ctx = use_context::<Ctx>();
    let tr = ctx.translator.read();
    let field = quick.field;
    let label = capitalized(&tr.plugin_phrase(
        match_format,
        PluginText::EnteredValue(field.label.key()),
        &field.label,
    ));
    let part = quick.part.as_ref().map(|(name, number)| {
        let name =
            capitalized(&tr.plugin_phrase(match_format, PluginText::Level(name.key()), name));
        t!(tr, "part-numbered", name = name.as_str(), number = *number)
    });
    let input = |team: Team, name: &str| {
        let value_of = label.as_str();
        let aria_label = match &part {
            Some(part) => t!(
                tr,
                "quick-value-of-in",
                value = value_of,
                team = name,
                part = part.as_str(),
                number = number
            ),
            None => t!(
                tr,
                "quick-value-of",
                value = value_of,
                team = name,
                number = number
            ),
        };
        rsx! {
            ValueInput {
                form,
                path: quick.path.clone(),
                field: field.clone(),
                team,
                prefix: format!("quick-value-{number}"),
                aria_label,
                issues: format!("quick-issues-{number}"),
                disabled,
                on_mounted: on_mounted.filter(|_| team == Team::A),
                on_input,
            }
        }
    };
    rsx! {
        span { class: "quick-value",
            // the inputs say which part, for screen readers
            if let Some((_, number)) = quick.part {
                span { class: "quick-part", aria_hidden: "true", "{number}" }
            }
            {input(Team::A, &a)}
            span { aria_hidden: "true", "–" }
            {input(Team::B, &b)}
        }
    }
}
