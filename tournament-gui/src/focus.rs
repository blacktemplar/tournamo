//! Moving the focus once the page shows a change that moved or removed what had it.

use std::rc::Rc;

use dioxus::prelude::*;
use wasm_bindgen::JsCast as _;

/// Moves the focus, once the page shows what was just changed, to the first element that
/// `selectors` find, if any, e.g. to the button of the next item after one removed.
pub fn focus_later(selectors: &[&str]) {
    // ids and classes of the page, with nothing a JavaScript string would read otherwise than Rust
    let selectors = format!("{selectors:?}");
    // the second frame is after the page rendered the change, which it does in the first
    _ = document::eval(&format!(
        r#"
        requestAnimationFrame(() => requestAnimationFrame(() => {{
            for (const selector of {selectors}) {{
                const element = document.querySelector(selector);
                if (element) {{
                    element.focus();
                    break;
                }}
            }}
        }}));
        "#
    ));
}

/// Focuses the element mounted with the handler it returns whenever `focus` asks for `target`,
/// and then asks for nothing.
pub fn use_focus<T: Copy + PartialEq + 'static>(
    mut focus: Signal<Option<T>>,
    target: T,
) -> impl FnMut(MountedEvent) {
    let mut element = use_signal(|| None::<Rc<MountedData>>);
    use_effect(move || {
        if focus() == Some(target)
            && let Some(element) = element()
        {
            focus.set(None);
            if let Some(element) = element
                .downcast::<web_sys::Element>()
                .and_then(|element| element.dyn_ref::<web_sys::HtmlElement>())
            {
                focus_in_sight(element);
            }
        }
    });
    move |event: MountedEvent| element.set(Some(event.data()))
}

/// Focuses `element`, scrolling only as far as it takes to show it, in either direction, e.g. not
/// sideways in a bracket whose next match is in sight already.
fn focus_in_sight(element: &web_sys::HtmlElement) {
    let options = web_sys::FocusOptions::new();
    options.set_prevent_scroll(true);
    // fails only if the element cannot take the focus
    _ = element.focus_with_options(&options);
    let options = web_sys::ScrollIntoViewOptions::new();
    options.set_block(web_sys::ScrollLogicalPosition::Nearest);
    options.set_inline(web_sys::ScrollLogicalPosition::Nearest);
    element.scroll_into_view_with_scroll_into_view_options(&options);
}

/// Asks for `target` to get the focus, see [`use_focus`].
pub fn move_focus<T: 'static>(mut focus: Signal<Option<T>>, target: T) {
    focus.set(Some(target));
}
