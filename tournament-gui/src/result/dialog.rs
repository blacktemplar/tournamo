//! The dialog entering a result: every part with the values of each side and its result, the one
//! the values suggest. Overriding it is rare, so a result is chosen only after asking to override,
//! except for a part that only a choice decides, and the dialog opens overriding where a result is
//! chosen already, e.g. that of a match reopened.

use dioxus::prelude::*;
use tournament_app::{
    i18n::{PluginText, Text},
    result_form::{FieldView, PartView, ResultForm},
    view::MatchRow,
};
use tournament_engine::{FieldKey, MatchResult, Team};

use super::{value::ValueInput, with_parts_won};
use crate::{
    announcement::Announcement,
    capitalized,
    competition::Ctx,
    components::dialog::{Dialog, DialogDescription, DialogTitle},
    focus::{focus_later, move_focus, use_focus},
    i18n::t,
    play::Focus,
    team_name,
};

/// The button entering the result of the match `row` in a dialog, and the dialog; once reported,
/// the focus moves to `after_report`. Beside the values of a [`QuickResult`], it opens with them,
/// `from`, and only offers more, not taking the focus once a result is reported.
#[component]
pub fn EnterResult(
    row: MatchRow,
    focus: Signal<Option<Focus>>,
    after_report: Focus,
    from: Option<Signal<Option<ResultForm>>>,
) -> Element {
    let ctx = use_context::<Ctx>();
    // the form while the dialog is open
    let mut entering = use_signal(|| None::<ResultForm>);
    let mut error = use_signal(|| None::<Text>);
    // whether results may be chosen against those the values suggest
    let mut overriding = use_signal(|| false);
    let tr = ctx.translator.read();
    let id = row.id;
    let (a, b) = (team_name(&tr, &row.a), team_name(&tr, &row.b));
    // only reported by the tab that changes the competition
    let disabled = !ctx.changes();
    let mut on_mounted = use_focus(focus, Focus::Report(id));
    let more = from.is_some();
    let (label, aria_label) = if more {
        (
            "result-details",
            t!(tr, "result-details-of", number = row.number),
        )
    } else {
        ("open-result", t!(tr, "open-result-of", number = row.number))
    };
    let view = entering.read().as_ref().map(ResultForm::view);
    let needs_choice = view.as_ref().is_some_and(|view| view.root.needs_choice);
    rsx! {
        button {
            class: if more { "details quiet small" } else { "enter-result" },
            r#type: "button",
            disabled,
            aria_label,
            onmounted: move |event| {
                if !more {
                    on_mounted(event);
                }
            },
            onclick: move |_| {
                let form = match from {
                    Some(from) => from.peek().clone().ok_or(()),
                    None => ctx.session.peek().result_form(id).map_err(drop),
                };
                if let Ok(form) = form {
                    error.set(None);
                    // a result chosen already, e.g. that of a match reopened, shows as chosen
                    overriding.set(form.is_chosen());
                    entering.set(Some(form));
                    focus_later(&["#result-form input", "#result-form select"]);
                }
            },
            {tr.get(label)}
        }
        Dialog {
            open: Some(view.is_some()),
            on_open_change: move |open: bool| {
                if !open {
                    entering.set(None);
                }
            },
            DialogTitle { {t!(tr, "result-title", number = row.number)} }
            DialogDescription { {t!(tr, "result-description", a = a.as_str(), b = b.as_str())} }
            if let Some(view) = view {
                form {
                    id: "result-form",
                    class: "dialog-form",
                    onsubmit: move |event| {
                        event.prevent_default();
                        let Some(form) = entering.read().clone() else {
                            return;
                        };
                        match ctx.try_act(|session| session.report_form(&form)) {
                            Ok(reported) => {
                                entering.set(None);
                                ctx.announce(Announcement::Reported(reported));
                                move_focus(focus, after_report);
                            }
                            Err(refused) => error.set(Some(refused)),
                        }
                    },
                    ResultPart {
                        form: entering,
                        part: view.root,
                        match_format: view.match_format,
                        overriding: overriding(),
                        a: a.clone(),
                        b: b.clone(),
                    }
                    p { class: "error", role: "alert",
                        if let Some(error) = error() {
                            {tr.sentence(&error)}
                        }
                    }
                    div { class: "dialog-buttons",
                        if !needs_choice {
                            button {
                                class: "quiet small muted override",
                                r#type: "button",
                                onclick: move |_| {
                                    if overriding() {
                                        if let Some(form) = entering.write().as_mut() {
                                            form.follow_suggestions();
                                        }
                                        overriding.set(false);
                                    } else {
                                        overriding.set(true);
                                        focus_later(&["#result-form .result-choice select"]);
                                    }
                                },
                                if overriding() {
                                    {tr.get("result-follow-values")}
                                } else {
                                    {tr.get("result-override")}
                                }
                            }
                        }
                        button { class: "primary", r#type: "submit", {tr.get("report")} }
                        button {
                            r#type: "button",
                            onclick: move |_| entering.set(None),
                            {tr.get("cancel")}
                        }
                    }
                }
            }
        }
    }
}

/// The part `part` of the result of `form` entered by the match format `match_format`, between `a`
/// and `b`: its values, its parts, then its result, to be chosen if `overriding`.
#[component]
fn ResultPart(
    form: Signal<Option<ResultForm>>,
    part: PartView,
    match_format: &'static str,
    overriding: bool,
    a: String,
    b: String,
) -> Element {
    let ctx = use_context::<Ctx>();
    let tr = ctx.translator.read();
    let name = capitalized(&tr.plugin_phrase(
        match_format,
        PluginText::Level(part.name.key()),
        &part.name,
    ));
    let legend = match part.path.last() {
        Some(index) => t!(
            tr,
            "part-numbered",
            name = name.as_str(),
            number = index + 1
        ),
        None => name,
    };
    // the values of a part of the match say which, as every part has the same fields
    let within = (!part.path.is_empty()).then(|| legend.clone());
    let id = part
        .path
        .iter()
        .fold("part".to_owned(), |id, index| format!("{id}-{index}"));
    rsx! {
        fieldset { class: "result-part", id: "{id}",
            legend { {legend.clone()} }
            for field in part.fields.clone() {
                ResultField {
                    form,
                    path: part.path.clone(),
                    field,
                    within: within.clone(),
                    match_format,
                    a: a.clone(),
                    b: b.clone(),
                }
            }
            for child in part.parts.clone() {
                ResultPart {
                    form,
                    part: child,
                    match_format,
                    overriding,
                    a: a.clone(),
                    b: b.clone(),
                }
            }
            ResultChoice {
                form,
                part: part.clone(),
                name: legend.clone(),
                overriding,
                a: a.clone(),
                b: b.clone(),
                id: "{id}-result",
            }
        }
    }
}

/// The values of `field` of the part at `path`, one for each side, said to be `within` the part so
/// named, if it is not the match.
#[component]
fn ResultField(
    form: Signal<Option<ResultForm>>,
    path: Vec<usize>,
    field: FieldView,
    within: Option<String>,
    match_format: &'static str,
    a: String,
    b: String,
) -> Element {
    let ctx = use_context::<Ctx>();
    let tr = ctx.translator.read();
    let label = capitalized(&tr.plugin_phrase(
        match_format,
        PluginText::EnteredValue(field.label.key()),
        &field.label,
    ));
    let issues_id = issues_id(&path, &field.key);
    let input = |team: Team, name: &str| {
        rsx! {
            ValueInput {
                form,
                path: path.clone(),
                field: field.clone(),
                team,
                prefix: "result-value",
                aria_label: match &within {
                    Some(part) => {
                        t!(
                            tr, "value-of-in", value = label.as_str(), team = name, part = part
                            .as_str()
                        )
                    }
                    None => t!(tr, "value-of", value = label.as_str(), team = name),
                },
                issues: issues_id.clone(),
            }
        }
    };
    rsx! {
        div { class: "result-field",
            span { class: "result-label", aria_hidden: "true", {label.clone()} }
            // the names beside their values, which their inputs name for screen readers
            span { class: "result-team a", aria_hidden: "true", {a.clone()} }
            {input(Team::A, &a)}
            span { aria_hidden: "true", "–" }
            {input(Team::B, &b)}
            span { class: "result-team b", aria_hidden: "true", {b.clone()} }
            if !field.issues.is_empty() {
                div { class: "issues", id: "{issues_id}", role: "status",
                    for issue in &field.issues {
                        p { {tr.sentence(issue)} }
                    }
                }
            }
            for warning in &field.warnings {
                p { class: "warnings", role: "status", {tr.sentence(warning)} }
            }
        }
    }
}

/// The id of why the values of the field `key` of the part at `path` in the dialog are not taken.
fn issues_id(path: &[usize], key: &FieldKey) -> String {
    let path: String = path.iter().map(|index| format!("-{index}")).collect();
    format!("result-issues{path}-{}", key.key())
}

/// The result of the part `part`, named `name`: the one its values suggest, or else another chosen,
/// which is offered if `overriding` or if only a choice decides the part.
#[component]
fn ResultChoice(
    form: Signal<Option<ResultForm>>,
    part: PartView,
    name: String,
    overriding: bool,
    a: String,
    b: String,
    id: String,
) -> Element {
    let ctx = use_context::<Ctx>();
    let tr = ctx.translator.read();
    let said = |result: MatchResult| match result {
        MatchResult::AWins => t!(tr, "wins", team = a.as_str()),
        MatchResult::BWins => t!(tr, "wins", team = b.as_str()),
        MatchResult::Draw => tr.get("draw"),
    };
    let suggested = match part.suggested {
        Some(result) => t!(tr, "result-as-entered", result = said(result).as_str()),
        None => tr.get("result-open"),
    };
    let chosen = if part.chosen { part.result } else { None };
    let value = |result: Option<MatchResult>| match result {
        None => "",
        Some(MatchResult::AWins) => "a",
        Some(MatchResult::Draw) => "draw",
        Some(MatchResult::BWins) => "b",
    };
    let path = part.path.clone();
    if !overriding && !part.needs_choice {
        let result = match part.result {
            Some(result) => said(result),
            None => tr.get("result-open"),
        };
        let result = with_parts_won(&tr, result, part.parts_won);
        let outcome = t!(
            tr,
            "result-is",
            part = name.as_str(),
            result = result.as_str()
        );
        return rsx! {
            div { class: "result-outcome",
                // read out as it changes with the values
                output { id: "{id}", {outcome} }
                for warning in &part.warnings {
                    p { class: "warnings", role: "status", {tr.sentence(warning)} }
                }
            }
        };
    }
    rsx! {
        div { class: "result-choice",
            label { r#for: "{id}", {t!(tr, "result-of", part = name.as_str())} }
            select {
                id: "{id}",
                value: value(chosen),
                onchange: move |event: FormEvent| {
                    let result = match event.value().as_str() {
                        "a" => Some(MatchResult::AWins),
                        "draw" => Some(MatchResult::Draw),
                        "b" => Some(MatchResult::BWins),
                        _ => None,
                    };
                    if let Some(form) = form.write().as_mut() {
                        form.choose(&path, result);
                    }
                },
                option { value: "", selected: chosen.is_none(), {suggested} }
                for result in [MatchResult::AWins, MatchResult::Draw, MatchResult::BWins] {
                    option {
                        value: value(Some(result)),
                        selected: chosen == Some(result),
                        {said(result)}
                    }
                }
            }
            for warning in &part.warnings {
                p { class: "warnings", role: "status", {tr.sentence(warning)} }
            }
        }
    }
}
