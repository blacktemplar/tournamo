//! The input of one side's value of a field of a result, completing the score once left.

use dioxus::prelude::*;
use tournament_app::result_form::{FieldView, ResultForm};
use tournament_engine::{EnteredValue, FieldKey, Team, plugins::match_format::ValueKind};

/// The input of `team`'s value of `field` of the part at `path` of `form`, with the id made of
/// `prefix` ([`value_id`]), named `aria_label`, and described by the element `issues` while what
/// was typed is not taken. Once left, it completes the score ([`complete_score`]).
#[component]
pub(super) fn ValueInput(
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
