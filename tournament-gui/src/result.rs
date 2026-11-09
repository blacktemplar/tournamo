//! The dialog entering the result of a match whose match format enters more than who won, e.g. a
//! score: the [`ResultForm`] of its result, every part with the values of each side and its result,
//! the one the values suggest. Overriding it is rare, so a result is chosen only after asking to
//! override, except for a part that only a choice decides, and the dialog opens overriding where a
//! result is chosen already, e.g. that of a match reopened.
//!
//! A match whose result takes no more than a row ([`ResultForm::quick_entry`]), e.g. a score or the
//! score of each game, is entered in the row below its pairing instead, the dialog a button away
//! for anything more, e.g. overriding the result.
//!
//! A value left completes the score of the other side where its winning score is the only one
//! possible ([`ResultForm::complete_score`]), selected if the focus moves there.
//!
//! What a field cannot take, e.g. a count above 65535, stays as typed, marked as invalid, with why
//! below the row or the field ([`ResultForm::enter`]).

use dioxus::prelude::*;
use tournament_app::{
    i18n::{PluginText, Text, Translator},
    result_form::{FieldView, PartView, QuickEntry, QuickField, ResultForm, ResultView},
    view::MatchRow,
};
use tournament_engine::{
    EnteredValue, FieldKey, MatchResult, Sides, Team,
    plugins::{Phrase, match_format::ValueKind},
};

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

/// What enters the result of the match `row`, whose match format enters more than who won: the row
/// of its values if it takes no more, else the button opening the dialog; once reported, the focus
/// moves to `after_report`.
#[component]
pub fn ResultOf(row: MatchRow, focus: Signal<Option<Focus>>, after_report: Focus) -> Element {
    let ctx = use_context::<Ctx>();
    let id = row.id;
    let mut form = use_signal(|| ctx.session.peek().result_form(id).ok());
    // what the result looks like, changing only with the match format, e.g. when the phase is
    // edited; what was entered for another one is dropped then
    let template = use_memo(move || ctx.session.read().result_template(id).ok());
    use_effect(move || {
        if form.peek().as_ref().map(ResultForm::template) != template.read().as_ref() {
            form.set(ctx.session.peek().result_form(id).ok());
        }
    });
    let quick = form.read().as_ref().and_then(ResultForm::quick_entry);
    match quick {
        Some(quick) => rsx! {
            QuickResult {
                row,
                form,
                quick,
                focus,
                after_report,
            }
        },
        None => rsx! {
            EnterResult { row, focus, after_report }
        },
    }
}

/// The values of the result of the match `row` in one row, `quick` of `form`, reported by Enter or
/// its button as they suggest, with the button opening the dialog with them for anything more.
#[component]
fn QuickResult(
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
    let on_mounted = EventHandler::new(crate::play::use_focus(focus, Focus::Report(id)));
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
    let mut on_mounted = crate::play::use_focus(focus, Focus::Report(id));
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

/// The input of `team`'s value of `field` of the part at `path` of `form`, with the id made of
/// `prefix` ([`value_id`]), named `aria_label`, and described by the element `issues` while what
/// was typed is not taken. Once left, it completes the score ([`complete_score`]).
#[component]
fn ValueInput(
    form: Signal<Option<ResultForm>>,
    path: Vec<usize>,
    field: FieldView,
    team: Team,
    prefix: String,
    aria_label: String,
    issues: String,
    #[props(default)] disabled: bool,
    on_mounted: Option<EventHandler<MountedEvent>>,
    on_input: Option<EventHandler<()>>,
) -> Element {
    let complete = use_callback({
        let (prefix, path, key) = (prefix.clone(), path.clone(), field.key.clone());
        move |tabbing: bool| {
            form.write()
                .as_mut()
                .is_some_and(|form| complete_score(form, &prefix, &path, &key, team, tabbing))
        }
    });
    let count = field.kind == ValueKind::Count;
    let entered = field.entered.of(team).clone();
    let invalid = entered.is_some();
    rsx! {
        input {
            r#type: "number",
            // a decimal may be negative
            min: count.then_some("0"),
            max: count.then_some("65535"),
            // a minus as well for a decimal, where the keyboard of decimals has none
            inputmode: if count { "numeric" } else { "text" },
            step: if count { "1" } else { "any" },
            disabled,
            id: value_id(&prefix, &path, &field.key, team),
            aria_label,
            aria_invalid: invalid.then_some("true"),
            aria_describedby: invalid.then_some(issues),
            value: entered.unwrap_or_else(|| shown(*field.values.of(team))),
            onblur: move |_| {
                complete(false);
            },
            onkeydown: move |event: KeyboardEvent| {
                if is_tab(&event) && complete(true) {
                    event.prevent_default();
                }
            },
            onmounted: move |event| {
                if let Some(on_mounted) = on_mounted {
                    on_mounted.call(event);
                }
            },
            oninput: move |event: FormEvent| {
                if let Some(form) = form.write().as_mut() {
                    form.enter(&path, &field.key, team, &event.value());
                }
                if let Some(on_input) = on_input {
                    on_input.call(());
                }
            },
        }
    }
}

/// The id of the input of `team`'s value of the field `key` of the part at `path`, among the inputs
/// whose ids start with `prefix`.
fn value_id(prefix: &str, path: &[usize], key: &FieldKey, team: Team) -> String {
    let path: String = path.iter().map(|index| format!("-{index}")).collect();
    let side = match team {
        Team::A => "a",
        Team::B => "b",
    };
    format!("{prefix}{path}-{}-{side}", key.key())
}

/// The id of why the values of the field `key` of the part at `path` in the dialog are not taken.
fn issues_id(path: &[usize], key: &FieldKey) -> String {
    let path: String = path.iter().map(|index| format!("-{index}")).collect();
    format!("result-issues{path}-{}", key.key())
}

/// Completes the score of the field `key` of the part at `path` once `team`'s value is left
/// ([`ResultForm::complete_score`]), returning whether it did; if the focus moved to the value
/// completed, among the inputs whose ids start with `prefix`, it is selected, so that typing
/// replaces it. If `tabbing`, the value is left by Tab, whose move the caller prevents if it
/// completed: the focus moves on once the page shows the score, so that it reaches a game the
/// score made the match need.
fn complete_score(
    form: &mut ResultForm,
    prefix: &str,
    path: &[usize],
    key: &FieldKey,
    team: Team,
    tabbing: bool,
) -> bool {
    if !form.complete_score(path, key, team) {
        return false;
    }
    // ids made of the page's prefixes, indices and field keys, which a JavaScript string reads as
    // Rust does
    let this = format!("{:?}", value_id(prefix, path, key, team));
    let other = format!("{:?}", value_id(prefix, path, key, team.opponent()));
    // the second frame is after the page rendered the value, and the focus moved on
    _ = document::eval(&format!(
        r#"
        requestAnimationFrame(() => requestAnimationFrame(() => {{
            if ({tabbing}) {{
                // what Tab moves to: the next element taking the focus that is shown
                const focusable = [...document.querySelectorAll(
                    "input, select, textarea, button, a[href], [tabindex]"
                )].filter(element => !element.disabled && element.tabIndex >= 0
                    && element.getClientRects().length > 0);
                const from = document.getElementById({this});
                const index = focusable.indexOf(from);
                if (index >= 0) {{
                    focusable[index + 1]?.focus();
                }}
            }}
            const element = document.activeElement;
            if (element?.id === {other}) {{
                element.select();
            }}
        }}));
        "#
    ));
    true
}

/// Whether `event` is Tab, moving the focus forwards.
fn is_tab(event: &KeyboardEvent) -> bool {
    event.key() == Key::Tab && !event.modifiers().contains(Modifiers::SHIFT)
}

/// What `value` is entered as into a field.
fn shown(value: Option<EnteredValue>) -> String {
    match value {
        Some(EnteredValue::Count(count)) => count.to_string(),
        Some(EnteredValue::Decimal(decimal)) => decimal.get().to_string(),
        None => String::new(),
    }
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

/// `said`, what is said of a part's result, with the parts each side won, if any, in the order of
/// the sides, e.g. "Bob wins (1–3)" or "Not decided yet (1–1)".
fn with_parts_won(tr: &Translator, said: String, parts_won: Option<Sides<u32>>) -> String {
    match parts_won {
        Some(won) => t!(
            tr,
            "result-with-parts-won",
            result = said,
            won = tr.score(won)
        ),
        None => said,
    }
}

/// The button showing the result of the finished match `row` as it was reported, and the dialog
/// showing it, read-only: a row of each of its parts, e.g. its games, with their values and who
/// won them, then the match's own values and result. The button has no text: it covers the
/// nearest positioned element around it, the match's card or pairing, so that a click anywhere on
/// it opens the dialog, and is named for screen readers. The stylesheet underlines the score
/// shown with it, as a link, which tells touch, without hover, that the match opens.
#[component]
pub fn ResultDetails(row: MatchRow) -> Element {
    let ctx = use_context::<Ctx>();
    // the result while the dialog is open
    let mut shown = use_signal(|| None::<ResultView>);
    let tr = ctx.translator.read();
    let id = row.id;
    let (a, b) = (team_name(&tr, &row.a), team_name(&tr, &row.b));
    rsx! {
        button {
            class: "match-details",
            r#type: "button",
            aria_label: t!(tr, "result-details-of", number = row.number),
            onclick: move |_| {
                shown.set(ctx.session.peek().reported_result(id));
                focus_later(&["#result-details button"]);
            },
        }
        Dialog {
            open: Some(shown.read().is_some()),
            on_open_change: move |open: bool| {
                if !open {
                    shown.set(None);
                }
            },
            DialogTitle { {t!(tr, "result-title", number = row.number)} }
            DialogDescription {
                {t!(tr, "result-details-description", a = a.as_str(), b = b.as_str())}
            }
            if let Some(view) = shown() {
                div { id: "result-details", class: "dialog-form",
                    ReportedPart {
                        part: view.root,
                        match_format: view.match_format,
                        a: a.clone(),
                        b: b.clone(),
                    }
                    div { class: "dialog-buttons",
                        button { r#type: "button", onclick: move |_| shown.set(None),
                            {tr.get("close")}
                        }
                    }
                }
            }
        }
    }
}

/// The part `part` of a finished match between `a` and `b` as it was reported: a table of its
/// parts, each with the values of each side and who won it, then its own values and its result.
/// Parts of its parts are not shown, as no match format has them yet.
#[component]
fn ReportedPart(part: PartView, match_format: &'static str, a: String, b: String) -> Element {
    let ctx = use_context::<Ctx>();
    let tr = ctx.translator.read();
    let level_name = |phrase: &Phrase| {
        capitalized(&tr.plugin_phrase(match_format, PluginText::Level(phrase.key()), phrase))
    };
    let field_label = |field: &FieldView| {
        capitalized(&tr.plugin_phrase(
            match_format,
            PluginText::EnteredValue(field.label.key()),
            &field.label,
        ))
    };
    let said = |result: Option<MatchResult>| match result {
        Some(MatchResult::AWins) => t!(tr, "wins", team = a.as_str()),
        Some(MatchResult::BWins) => t!(tr, "wins", team = b.as_str()),
        Some(MatchResult::Draw) => tr.get("draw"),
        None => tr.get("result-open"),
    };
    let value = |value: Option<EnteredValue>| match value {
        Some(EnteredValue::Count(count)) => tr.number(count.into()),
        Some(EnteredValue::Decimal(decimal)) => tr.number(decimal.get()),
        None => "–".to_owned(),
    };
    // the columns of the parts: those of the fields of the first, which every part at one level
    // shares, each side's value in a column of its own, named by the side, and by the field if
    // there are several
    let fields: Vec<FieldView> = part
        .parts
        .first()
        .map(|first| first.fields.clone())
        .unwrap_or_default();
    let several = fields.len() > 1;
    let column = |field: &FieldView, name: &str| {
        if several {
            format!("{} {name}", field_label(field))
        } else {
            name.to_owned()
        }
    };
    let headers: Vec<String> = fields
        .iter()
        .flat_map(|field| [column(field, &a), column(field, &b)])
        .collect();
    // each part's name, its values in the columns' order, who won it and its warnings
    // without values, who won a part is said in a column of its own
    let result_column = fields.is_empty();
    let rows: Vec<ReportedRow> = part
        .parts
        .iter()
        .enumerate()
        .map(|(index, child)| {
            let name = t!(
                tr,
                "part-numbered",
                name = level_name(&child.name),
                number = index + 1
            );
            let (a_won, b_won) = (
                child.result == Some(MatchResult::AWins),
                child.result == Some(MatchResult::BWins),
            );
            let values = fields
                .iter()
                .flat_map(|field| {
                    let values = child
                        .fields
                        .iter()
                        .find(|of_child| of_child.key == field.key)
                        .map(|of_child| of_child.values);
                    [
                        (value(values.and_then(|values| values.a)), a_won),
                        (value(values.and_then(|values| values.b)), b_won),
                    ]
                })
                .collect();
            let warnings = child
                .warnings
                .iter()
                .map(|warning| {
                    t!(
                        tr,
                        "part-warning",
                        part = name.as_str(),
                        warning = tr.sentence(warning)
                    )
                })
                .collect();
            ReportedRow {
                result: said(child.result),
                name,
                values,
                warnings,
            }
        })
        .collect();
    let warnings: Vec<String> = rows
        .iter()
        .flat_map(|row| row.warnings.clone())
        .chain(part.warnings.iter().map(|warning| tr.sentence(warning)))
        .collect();
    let name = level_name(&part.name);
    let outcome = with_parts_won(&tr, said(part.result), part.parts_won);
    rsx! {
        if let Some(first) = part.parts.first() {
            table { class: "reported-parts",
                thead {
                    tr {
                        th { scope: "col", {level_name(&first.name)} }
                        for header in headers {
                            th { scope: "col", class: "value", {header} }
                        }
                        if result_column {
                            th { scope: "col", {tr.get("result-column")} }
                        }
                    }
                }
                tbody {
                    for ReportedRow { name, result, values, .. } in rows {
                        tr {
                            th { scope: "row",
                                {name}
                                // the winner's values are marked, which screen readers may not
                                // read
                                if !result_column {
                                    span { class: "visually-hidden", ", {result}" }
                                }
                            }
                            for (value, won) in values {
                                td { class: if won { "value won" } else { "value" }, {value} }
                            }
                            if result_column {
                                td { {result} }
                            }
                        }
                    }
                }
            }
        }
        for field in &part.fields {
            p { class: "reported-field",
                {
                    t!(
                        tr, "reported-value", value = field_label(field), a = a.as_str(), b = b
                        .as_str(), score = tr.scores(&[field.values])
                    )
                }
            }
        }
        p { class: "reported-outcome",
            {t!(tr, "result-is", part = name.as_str(), result = outcome.as_str())}
        }
        for warning in warnings {
            p { class: "warnings", {warning} }
        }
    }
}

/// A part of a finished match as a row of [`ReportedPart`].
struct ReportedRow {
    /// e.g. Game 2
    name: String,
    /// who won it
    result: String,
    /// in the order of the columns, each marked if it is of the winner
    values: Vec<(String, bool)>,
    warnings: Vec<String>,
}
