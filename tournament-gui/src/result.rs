//! The dialog entering the result of a match whose match format enters more than who won, e.g. a
//! score: the [`ResultForm`] of its result, every part with the values of each side and its result,
//! the one the values suggest unless another is chosen.

use dioxus::prelude::*;
use tournament_app::{
    i18n::{PluginText, Text},
    result_form::{FieldView, PartView, ResultForm},
    view::MatchRow,
};
use tournament_engine::{EnteredValue, MatchResult, Score, Team, plugins::match_format::ValueKind};

use crate::{
    announcement::Announcement,
    capitalized,
    competition::Ctx,
    components::dialog::{Dialog, DialogDescription, DialogTitle},
    i18n::t,
    phase::focus_later,
    play::{Focus, move_focus},
    team_name,
};

/// The button entering the result of the match `row`, and the dialog it opens; once reported, the
/// focus moves to `after_report`.
#[component]
pub fn EnterResult(row: MatchRow, focus: Signal<Option<Focus>>, after_report: Focus) -> Element {
    let ctx = use_context::<Ctx>();
    // the form while the dialog is open
    let mut entering = use_signal(|| None::<Signal<ResultForm>>);
    let mut error = use_signal(|| None::<Text>);
    let tr = ctx.translator.read();
    let id = row.id;
    let (a, b) = (team_name(&tr, &row.a), team_name(&tr, &row.b));
    // only reported by the tab that changes the competition
    let disabled = !ctx.changes();
    let on_mounted = crate::play::use_focus(focus, Focus::Report(id));
    rsx! {
        button {
            class: "enter-result",
            disabled,
            aria_label: t!(tr, "open-result-of", number = row.number),
            onmounted: on_mounted,
            onclick: move |_| {
                if let Ok(form) = ctx.session.peek().result_form(id) {
                    error.set(None);
                    entering.set(Some(Signal::new(form)));
                    focus_later(&["#result-form input", "#result-form select"]);
                }
            },
            {tr.get("open-result")}
        }
        Dialog {
            open: Some(entering.read().is_some()),
            on_open_change: move |open: bool| {
                if !open {
                    entering.set(None);
                }
            },
            DialogTitle { {t!(tr, "result-title", number = row.number)} }
            DialogDescription { {t!(tr, "result-description", a = a.as_str(), b = b.as_str())} }
            if let Some(form) = entering() {
                form {
                    id: "result-form",
                    class: "dialog-form",
                    onsubmit: move |event| {
                        event.prevent_default();
                        match ctx.try_act(|session| session.report_form(&form.read())) {
                            Ok(reported) => {
                                entering.set(None);
                                ctx.announce(Announcement::Reported(reported));
                                move_focus(focus, after_report);
                            }
                            Err(refused) => error.set(Some(refused)),
                        }
                    },
                    ResultPart {
                        form,
                        part: form.read().view().root,
                        match_format: form.read().view().match_format,
                        a: a.clone(),
                        b: b.clone(),
                    }
                    p { class: "error", role: "alert",
                        if let Some(error) = error() {
                            {tr.sentence(&error)}
                        }
                    }
                    div { class: "dialog-buttons",
                        button { class: "primary", r#type: "submit", {tr.get("report")} }
                        button { r#type: "button", onclick: move |_| entering.set(None),
                            {tr.get("cancel")}
                        }
                    }
                }
            }
        }
    }
}

/// The part `part` of the result of `form` entered by the match format `match_format`, between `a`
/// and `b`: its values, its parts, then its result.
#[component]
fn ResultPart(
    form: Signal<ResultForm>,
    part: PartView,
    match_format: &'static str,
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
                    a: a.clone(),
                    b: b.clone(),
                }
            }
            ResultChoice {
                form,
                part: part.clone(),
                name: legend.clone(),
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
    form: Signal<ResultForm>,
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
    let kind = field.kind;
    let shown = |value: Option<EnteredValue>| match value {
        Some(EnteredValue::Count(count)) => count.to_string(),
        Some(EnteredValue::Decimal(decimal)) => decimal.get().to_string(),
        None => String::new(),
    };
    let input = |team: Team, name: &str, value: Option<EnteredValue>| {
        let (path, key) = (path.clone(), field.key.clone());
        rsx! {
            input {
                r#type: "number",
                min: "0",
                inputmode: if kind == ValueKind::Count { "numeric" } else { "decimal" },
                step: if kind == ValueKind::Count { "1" } else { "any" },
                aria_label: match &within {
                    Some(part) => {
                        t!(
                            tr,
                            "value-of-in",
                            value = label.as_str(),
                            team = name,
                            part = part.as_str()
                        )
                    }
                    None => t!(tr, "value-of", value = label.as_str(), team = name),
                },
                value: shown(value),
                oninput: move |event: FormEvent| {
                    let value = entered(&event.value(), kind);
                    form.write().set_value(&path, &key, team, value);
                },
            }
        }
    };
    rsx! {
        div { class: "result-field",
            span { class: "result-label", aria_hidden: "true", {label.clone()} }
            // the names beside their values, which their inputs name for screen readers
            span { class: "result-team a", aria_hidden: "true", {a.clone()} }
            {input(Team::A, &a, field.values.a)}
            span { aria_hidden: "true", "–" }
            {input(Team::B, &b, field.values.b)}
            span { class: "result-team b", aria_hidden: "true", {b.clone()} }
            for warning in &field.warnings {
                p { class: "warnings", role: "status", {tr.sentence(warning)} }
            }
        }
    }
}

/// The value `text` entered into a field of `kind` stands for, `None` if none.
fn entered(text: &str, kind: ValueKind) -> Option<EnteredValue> {
    let text = text.trim();
    match kind {
        ValueKind::Count => text.parse().ok().map(EnteredValue::Count),
        ValueKind::Decimal => text
            .replace(',', ".")
            .parse::<f64>()
            .ok()
            .filter(|number| number.is_finite())
            .map(|number| EnteredValue::Decimal(Score::new(number))),
    }
}

/// The result of the part `part`, named `name`: the one its values suggest, or another chosen.
#[component]
fn ResultChoice(
    form: Signal<ResultForm>,
    part: PartView,
    name: String,
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
                    form.write().choose(&path, result);
                },
                option { value: "", selected: chosen.is_none(), {suggested} }
                for result in [MatchResult::AWins, MatchResult::Draw, MatchResult::BWins] {
                    option { value: value(Some(result)), selected: chosen == Some(result),
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
