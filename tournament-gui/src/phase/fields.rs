//! The fields of the form of a phase, each laid out by the kind of its answer.

use dioxus::prelude::*;
use tournament_app::{
    form::{Editable, FormField, Input, Path, PhaseForm, PluginInput, PluginOption, PresetForm},
    i18n::{PluginText, Text, Translator},
};
use tournament_engine::{PhasePlugins, plugin_factory::config::ConfigError};

use super::lists::{MetricsList, OrderedList, UnorderedList};
use crate::{
    capitalized,
    competition::Ctx,
    components::select::{Select, SelectOption},
    focus::focus_later,
    i18n::collation_key,
    icons, sentence,
    toggletip::Toggletip,
};

/// The form whose fields are laid out: of a phase, or of a preset.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum EditedForm {
    Phase(Signal<PhaseForm>),
    Preset(Signal<PresetForm>),
}

impl EditedForm {
    /// Applies `edit` to the form. Its error is dropped: a field laid out is one of the form, and
    /// takes what its input hands over.
    pub(super) fn edit(self, edit: impl FnOnce(&mut dyn Editable) -> Result<(), ConfigError>) {
        _ = match self {
            Self::Phase(mut form) => edit(&mut *form.write()),
            Self::Preset(mut form) => edit(&mut *form.write()),
        };
    }

    /// Why the field at `path` would not take `typed`, see [`Editable::issue_entering`].
    fn issue_entering(self, path: &Path, typed: &str) -> Option<Text> {
        match self {
            Self::Phase(form) => form.read().issue_entering(path, typed),
            Self::Preset(form) => form.read().issue_entering(path, typed),
        }
        .ok()
        .flatten()
    }
}

#[component]
pub(super) fn Fields(form: EditedForm, fields: Vec<FormField>, disabled: bool) -> Element {
    // the rows compared across divisions show metrics where the phase shows its field comparing
    // them
    let compared = fields
        .iter()
        .any(|field| field.plugin.is_none() && field.name == PhasePlugins::ACROSS);
    rsx! {
        for field in fields {
            Field { key: "{field.path}", form, field, compared, disabled }
        }
    }
}

/// One field, laid out by the kind of its answer. `compared` is whether the phase compares its
/// divisions, so that a list of metrics has a column for the comparison.
#[component]
fn Field(form: EditedForm, field: FormField, compared: bool, disabled: bool) -> Element {
    let ctx = use_context::<Ctx>();
    let tr = ctx.translator.read();
    let id = id(&field.path);
    let label = capitalized(&tr.field_label(&field));
    let description = tr.field_description(&field);
    let path = field.path.clone();
    let described_by = format!("{id}-description {id}-issues {id}-warnings");
    let issues = rsx! {
        Issues { id: "{id}-issues", issues: field.issues.clone() }
        Warnings { id: "{id}-warnings", warnings: field.warnings.clone() }
    };
    // what is said about the field, next to its label
    let about = rsx! {
        if let Some(text) = description {
            " "
            Toggletip { id: "{id}-description", subject: tr.field_label(&field), text }
        }
    };
    match field.input.clone() {
        Input::Bool(value) => rsx! {
            div { class: "field",
                label {
                    input {
                        r#type: "checkbox",
                        id: "{id}",
                        checked: value.unwrap_or_default(),
                        disabled,
                        aria_describedby: "{described_by}",
                        onchange: move |event| {
                            form.edit(|form| form.set_bool(&path, event.checked()));
                        },
                    }
                    " {label}"
                }
                {about}
                {issues}
            }
        },
        Input::Integer {
            range,
            value,
            entered,
            hint,
        } => {
            let shown = entered.or_else(|| value.map(|value| value.to_string()));
            rsx! {
                NumberField {
                    form, path, id, label, disabled,
                    value: shown,
                    // a minus as well, where the keyboard of the numbers has none
                    keyboard: if range.min() < 0 { "text" } else { "numeric" },
                    hint: tr.text(&hint),
                    about,
                    issues: field.issues,
                    warnings: field.warnings,
                }
            }
        }
        Input::Decimal {
            range,
            value,
            entered,
            hint,
        } => {
            let shown = entered.or_else(|| value.map(|value| tr.number(value)));
            let negative = range.min().is_none_or(|min| min < 0.0);
            rsx! {
                NumberField {
                    form, path, id, label, disabled,
                    value: shown,
                    keyboard: if negative { "text" } else { "decimal" },
                    hint: tr.text(&hint),
                    about,
                    issues: field.issues,
                    warnings: field.warnings,
                }
            }
        }
        Input::Optional { answered, inner } => {
            // described once, here
            let inner = FormField {
                input: *inner,
                description: None,
                issues: Vec::new(),
                warnings: Vec::new(),
                ..field.clone()
            };
            rsx! {
                div { class: "field",
                    label {
                        input {
                            r#type: "checkbox",
                            checked: answered,
                            disabled,
                            onchange: move |event| {
                                form.edit(|form| form.answer_optional(&path, event.checked()));
                            },
                        }
                        " {label}"
                    }
                    {about}
                    if answered {
                        Field { form, field: inner, compared, disabled }
                    }
                    {issues}
                }
            }
        }
        Input::Plugin(plugin) => rsx! {
            PluginField { form, path, id, label, plugin, disabled, about,
                {issues}
            }
        },
        // a group rather than a fieldset, whose legend would take the toggletip's button into its
        // name
        Input::List(list) => rsx! {
            div {
                class: "field list",
                role: "group",
                aria_labelledby: "{id}-label",
                aria_describedby: "{described_by}",
                div { class: "list-heading",
                    span { class: "label", id: "{id}-label", "{label}" }
                    {about}
                }
                if list.ordered {
                    OrderedList { form, path, id: id.clone(), list, disabled }
                } else if let (false, EditedForm::Phase(form)) = (list.shown.is_empty(), form) {
                    MetricsList { form, list_id: id.clone(), list, compared, disabled }
                } else {
                    UnorderedList { form, path, list_id: id.clone(), list, disabled }
                }
                {issues}
            }
        },
    }
}

/// A field taking a number, entered as text and taken once the operator leaves the field, with
/// what is said about it, `about`, next to its label, and its `issues` and `warnings` below it.
/// `keyboard` is the `inputmode` of the input, the keyboard a touch screen shows. While a number is
/// typed, the issues are why the field would not take it.
///
/// The input is a text: one of the type `number` hands over nothing for what it does not read as
/// a number, e.g. `0,5` where the browser expects a point, so that the form could not say what was
/// entered, and it would read it otherwise than the form.
#[component]
fn NumberField(
    form: EditedForm,
    path: Path,
    id: String,
    label: String,
    value: Option<String>,
    keyboard: String,
    hint: String,
    disabled: bool,
    about: Element,
    issues: Vec<Text>,
    warnings: Vec<Text>,
) -> Element {
    // what is typed and not taken yet, if anything, taken on Enter as well, which does not submit
    // the form
    let mut typed = use_signal(|| None::<String>);
    let issues = match &*typed.read() {
        Some(typed) => form.issue_entering(&path, typed).into_iter().collect(),
        None => issues,
    };
    let invalid = !issues.is_empty();
    let take = path.clone();
    rsx! {
        // on a grid of a column of labels and one of inputs, the numbers it takes below its input
        div { class: "field number-field",
            span { class: "number-label",
                label { r#for: "{id}", "{label}" }
                {about}
            }
            input {
                r#type: "text",
                class: "number",
                id: "{id}",
                inputmode: keyboard,
                autocomplete: "off",
                value: typed().or(value).unwrap_or_default(),
                disabled,
                aria_invalid: invalid.then_some("true"),
                aria_describedby: "{id}-hint {id}-description {id}-issues",
                oninput: move |event| typed.set(Some(event.value())),
                onkeydown: move |event| {
                    if event.key() == Key::Enter {
                        event.prevent_default();
                        if let Some(typed) = typed.take() {
                            form.edit(|form| form.enter(&take, &typed));
                        }
                    }
                },
                onchange: move |event| {
                    form.edit(|form| form.enter(&path, &event.value()));
                    typed.set(None);
                },
            }
            span { class: "hint", id: "{id}-hint", {sentence(&hint)} }
            Issues { id: "{id}-issues", issues }
            Warnings { id: "{id}-warnings", warnings }
        }
    }
}

/// A field taking one plugin, chosen from its options, followed by the fields of the plugin, with
/// what is said about it, `about`, next to its label.
#[component]
fn PluginField(
    form: EditedForm,
    path: Path,
    id: String,
    label: String,
    plugin: PluginInput,
    disabled: bool,
    about: Element,
    children: Element,
) -> Element {
    let ctx = use_context::<Ctx>();
    let tr = ctx.translator.read();
    let chosen = plugin
        .chosen
        .as_ref()
        .map(|chosen| chosen.plugin.id.to_owned());
    let none = plugin.optional.then(|| tr.get("no-plugin"));
    let options = in_label_order(&tr, &plugin.options);
    let choose = path.clone();
    let trigger = format!("#{id}");
    // what the plugin chosen does, below the choice, rather than a second ⓘ next to the field's
    let chosen_description = plugin
        .chosen
        .as_ref()
        .and_then(|chosen| plugin_description(&tr, &chosen.plugin));
    rsx! {
        div { class: "field",
            span { class: "label", id: "{id}-label", "{label}" }
            {about}
            " "
            Select {
                id: "{id}",
                labelled_by: "{id}-label",
                described_by: chosen_description.is_some().then(|| format!("{id}-chosen")),
                value: chosen.or_else(|| plugin.optional.then(String::new)),
                placeholder: tr.get("choose-plugin"),
                disabled,
                // the option of no plugin has no id
                on_change: move |chosen: String| {
                    let chosen = Some(chosen.as_str()).filter(|chosen| !chosen.is_empty());
                    form.edit(|form| form.choose(&choose, chosen));
                    // back from the list, which is hidden once an option is chosen
                    focus_later(&[&trigger]);
                },
                for (index, option) in options.into_iter().enumerate() {
                    PluginChoice {
                        key: "{option.id}",
                        select: id.clone(),
                        index,
                        option,
                    }
                }
                if let Some(none) = none {
                    SelectOption {
                        index: plugin.options.len(),
                        value: String::new(),
                        text_value: none.clone(),
                        span { class: "option-label", "{none}" }
                    }
                }
            }
            if let Some(description) = chosen_description {
                p { class: "hint chosen-description", id: "{id}-chosen",
                    {sentence(&description)}
                }
            }
            {children}
            if let Some(chosen) = plugin.chosen {
                div { class: "plugin-fields",
                    Fields { form, fields: chosen.fields, disabled }
                }
            }
        }
    }
}

/// `option` as an option of the [`Select`] whose id is `select`, at `index`. Its description
/// shows next to the list while the option is under the pointer or has the focus, which the
/// keyboard moves through the options, and in the option, below its label, from a tap or click
/// on its ⓘ until the next, which is how a touch screen shows it before the option is chosen.
#[component]
pub(super) fn PluginChoice(select: String, index: usize, option: PluginOption) -> Element {
    let ctx = use_context::<Ctx>();
    let tr = ctx.translator.read();
    let label = capitalized(&plugin_label(&tr, &option));
    let description = plugin_description(&tr, &option);
    let mut tapped = use_signal(|| false);
    // ids of plugins are namespaced by `::`
    let described_by = format!("{select}-option-{}", option.id.replace("::", "-"));
    rsx! {
        SelectOption {
            index,
            value: option.id.to_owned(),
            text_value: label.clone(),
            aria_describedby: description.is_some().then(|| described_by.clone()),
            span { class: "option",
                span { class: "option-label", "{label}" }
                if option.recommended {
                    " "
                    span { class: "recommended", {tr.get("recommended")} }
                }
                if let Some(description) = description {
                    // no button: a keyboard and a screen reader get the description through the
                    // option, and a button would be an element to use within one; the press is
                    // kept from the option, which would choose it on its release
                    span {
                        class: "option-info",
                        "aria-hidden": "true",
                        onpointerdown: move |event| {
                            event.stop_propagation();
                            event.prevent_default();
                        },
                        onclick: move |event| {
                            event.stop_propagation();
                            tapped.toggle();
                        },
                        icons::Info {}
                    }
                    span {
                        class: "option-description",
                        id: "{described_by}",
                        "data-tapped": tapped().then_some("true"),
                        span { class: "option-description-text", {sentence(&description)} }
                    }
                }
            }
        }
    }
}

/// What is wrong, as a list with the id `id`, which the inputs it is about name as their
/// description; nothing if nothing is. The list takes the focus from [`focus_first_issue`].
#[component]
pub(super) fn Issues(id: String, issues: Vec<Text>) -> Element {
    let ctx = use_context::<Ctx>();
    let tr = ctx.translator.read();
    rsx! {
        if !issues.is_empty() {
            ul { class: "issues", id: "{id}", tabindex: "-1",
                for issue in issues {
                    li { {tr.sentence(&issue)} }
                }
            }
        }
    }
}

/// What the answer of a field leads to that the operator may not want, although the phase can be
/// played with it, as a list with the id `id`; nothing if there is nothing to say.
#[component]
pub(super) fn Warnings(id: String, warnings: Vec<Text>) -> Element {
    let ctx = use_context::<Ctx>();
    let tr = ctx.translator.read();
    rsx! {
        if !warnings.is_empty() {
            ul { class: "warnings", id: "{id}",
                for warning in warnings {
                    li { {tr.sentence(&warning)} }
                }
            }
        }
    }
}

pub(super) fn plugin_label(tr: &Translator, option: &PluginOption) -> String {
    tr.plugin(option.id, PluginText::Label, option.label)
}

pub(super) fn plugin_description(tr: &Translator, option: &PluginOption) -> Option<String> {
    option
        .description
        .map(|english| tr.plugin(option.id, PluginText::Description, english))
}

/// The id of the element of the field at `path`, e.g. `field-metrics-1-points_for_win`.
pub(super) fn id(path: &Path) -> String {
    let path = path
        .to_string()
        .replace(['.', '[', ']'], "-")
        .replace("--", "-");
    format!("field-{}", path.trim_end_matches('-'))
}

/// `options`, those recommended first as they are, the others in the order of their labels in the
/// language of `tr`, rather than in that of their English labels.
pub(super) fn in_label_order(tr: &Translator, options: &[PluginOption]) -> Vec<PluginOption> {
    let mut options = options.to_vec();
    let recommended = options
        .iter()
        .take_while(|option| option.recommended)
        .count();
    options[recommended..].sort_by_cached_key(|option| {
        collation_key(&tr.plugin(option.id, PluginText::Label, option.label))
    });
    options
}
