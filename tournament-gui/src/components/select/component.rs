use dioxus::prelude::*;
use dioxus_icons::lucide::{Check, ChevronDown};
use dioxus_primitives::{
    dioxus_attributes::attributes,
    merge_attributes,
    select::{self, SelectOptionProps},
};

#[css_module("/src/components/select/style.css")]
struct Styles;

// Changed from the component `dx components add` copied: the trigger has an id and is named by a
// label of the page together with the value it shows, since a `label` around the trigger would
// also catch the clicks on the options; the value is a plugin's id, and only the single select is
// kept. The trigger may be described by an element of the page, e.g. what the option chosen does.

/// A choice of one of its options, each a [`SelectOption`], whose trigger has the id `id` and is
/// named by the element `labelled_by` followed by the option chosen, or else by `placeholder`, and
/// described by the element `described_by`, if any.
#[component]
pub fn Select(
    id: String,
    labelled_by: String,
    #[props(default)] described_by: Option<String>,
    value: ReadSignal<Option<String>>,
    #[props(into)] placeholder: String,
    on_change: EventHandler<String>,
    #[props(default)] disabled: bool,
    children: Element,
) -> Element {
    let base = attributes!(div {
        class: Styles::dx_select
    });
    // changed: Escape closes only the list, before the document hears it, where a dialog around
    // the select would close as well, and the focus is back at the trigger rather than on the
    // list, which is gone once closed
    let trigger = id.clone();
    use_effect(move || {
        _ = document::eval(&format!(
            r#"
            const trigger = document.getElementById({trigger:?});
            const select = trigger?.parentElement;
            if (select && !select.closesOnEscape) {{
                select.closesOnEscape = true;
                select.addEventListener("keydown", (event) => {{
                    if (event.key === "Escape" && select.dataset.state === "open") {{
                        event.stopPropagation();
                        event.preventDefault();
                        trigger.click();
                        trigger.focus();
                    }}
                }});
            }}
            "#
        ));
    });
    rsx! {
        select::Select::<String> {
            value: Some(value),
            on_value_change: move |value: Option<String>| {
                if let Some(value) = value {
                    on_change.call(value);
                }
            },
            disabled,
            attributes: merge_attributes(vec![base]),
            select::SelectTrigger {
                class: Styles::dx_select_trigger,
                id: "{id}",
                aria_labelledby: "{labelled_by} {id}",
                aria_describedby: described_by,
                select::SelectValue { placeholder }
                ChevronDown {
                    class: "dx-select-expand-icon",
                    size: "20px",
                    stroke: "var(--primary-color-7)",
                }
            }
            select::SelectList {
                class: Styles::dx_select_list,
                aria_labelledby: "{labelled_by}",
                {children}
            }
        }
    }
}

#[component]
pub fn SelectOption(props: SelectOptionProps<String>) -> Element {
    let base = attributes!(div {
        class: Styles::dx_select_option
    });
    let merged = merge_attributes(vec![base, props.attributes]);

    rsx! {
        select::SelectOption::<String> {
            value: props.value,
            text_value: props.text_value,
            disabled: props.disabled,
            id: props.id,
            index: props.index,
            aria_label: props.aria_label,
            aria_roledescription: props.aria_roledescription,
            attributes: merged,
            {props.children}
            select::SelectItemIndicator {
                Check {
                    size: "1rem",
                    stroke: "var(--secondary-color-5)",
                }
            }
        }
    }
}
