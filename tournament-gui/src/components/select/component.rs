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
// kept. The trigger may be described by an element of the page, and shows what the option chosen
// does behind an ⓘ, below it while the pointer is on it or the keyboard's focus, as an option of
// the list does.

/// A choice of one of its options, each a [`SelectOption`], whose trigger has the id `id` and is
/// named by the element `labelled_by` followed by the option chosen, or else by `placeholder`, and
/// described by the element `described_by`, if any, and by `description`, what the option chosen
/// does, which shows while the pointer is on the trigger or it has the focus of the keyboard.
#[component]
pub fn Select(
    id: String,
    labelled_by: String,
    #[props(default)] described_by: Option<String>,
    #[props(default)] description: Option<String>,
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
    // list, which is gone once closed; and the list opened is moved left as far as it would
    // reach past the right edge of the page, e.g. of a trigger right of its label on a phone,
    // where it made the page scroll sideways
    let trigger = id.clone();
    let chosen = format!("{id}-chosen");
    let described_by = described_by
        .into_iter()
        .chain(description.is_some().then(|| chosen.clone()))
        .reduce(|first, second| format!("{first} {second}"));
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
                const fit = () => {{
                    const list = select.querySelector("[role='listbox']");
                    if (!list) return;
                    list.style.left = "";
                    // where the select starts, as the list does unmoved, and the list's width,
                    // both without the scale it opens with; a margin as the page's
                    const margin = 16;
                    const left = select.getBoundingClientRect().left;
                    const over = left + list.offsetWidth
                        - (document.documentElement.clientWidth - margin);
                    if (over > 0) {{
                        list.style.left = `${{-Math.min(over, left - margin)}}px`;
                    }}
                }};
                // the list and its options are inserted a render after the select opens: fitted
                // whenever they change, in the microtask right after, before a frame shows them
                new MutationObserver(() => {{
                    if (select.dataset.state === "open") fit();
                }}).observe(select, {{
                    attributes: true,
                    attributeFilter: ["data-state"],
                    childList: true,
                    subtree: true,
                    characterData: true,
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
                if description.is_some() {
                    span { class: Styles::dx_select_info, aria_hidden: "true",
                        crate::icons::Info {}
                    }
                }
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
            if let Some(description) = description {
                span { class: Styles::dx_select_description, id: "{chosen}",
                    span { class: "option-description-text", {description} }
                }
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
