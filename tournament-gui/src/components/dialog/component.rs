use std::sync::atomic::{AtomicUsize, Ordering};

use dioxus::prelude::*;
use dioxus_primitives::{
    dialog::{self, DialogDescriptionProps, DialogRootProps, DialogTitleProps},
    dioxus_attributes::attributes,
    merge_attributes,
};

#[css_module("/src/components/dialog/style.css")]
struct Styles;

#[component]
pub fn Dialog(props: DialogRootProps) -> Element {
    let base = attributes!(div {
        class: Styles::dx_dialog,
    });
    let merged = merge_attributes(vec![base, props.attributes]);
    // changed: the backdrop has an id of its own, closing on a click rather than on pointerdown
    let id = use_hook(|| {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        format!("dialog-backdrop-{}", NEXT.fetch_add(1, Ordering::Relaxed))
    });
    use_backdrop_click(id.clone(), props.on_open_change);

    rsx! {
        dialog::DialogRoot {
            class: Styles::dx_dialog_backdrop,
            id: Some(id),
            is_modal: props.is_modal,
            open: props.open,
            default_open: props.default_open,
            on_open_change: props.on_open_change,
            dialog::DialogContent {
                class: None,
                attributes: merged,
                {props.children}
            }
        }
    }
}

#[component]
pub fn DialogTitle(props: DialogTitleProps) -> Element {
    let base = attributes!(h2 {
        class: Styles::dx_dialog_title,
    });
    let merged = merge_attributes(vec![base, props.attributes]);

    rsx! {
        dialog::DialogTitle {
            id: props.id,
            attributes: merged,
            {props.children}
        }
    }
}

#[component]
pub fn DialogDescription(props: DialogDescriptionProps) -> Element {
    let base = attributes!(p {
        class: Styles::dx_dialog_description,
    });
    let merged = merge_attributes(vec![base, props.attributes]);

    rsx! {
        dialog::DialogDescription {
            id: props.id,
            attributes: merged,
            {props.children}
        }
    }
}

// changed: the primitive closes the dialog on a pointerdown outside it. After a tap, the
// backdrop is gone (`pointer-events: none` while it fades out) by the time the browser makes the
// click, which then hits whatever lies below, e.g. a link leaving the page. So a pointerdown on
// the backdrop is kept from the primitive's listener (on the document, which a listener on the
// window precedes), and the click on the backdrop closes the dialog instead, if it was pressed
// there too (not when a selection made in the dialog ends on the backdrop).
/// Closes the dialog whose backdrop has the id `id` by `on_open_change` when the backdrop is
/// clicked, and keeps a pointerdown on it from closing the dialog before.
fn use_backdrop_click(id: String, on_open_change: Callback<bool>) {
    let eval = use_hook(move || {
        let mut eval = document::eval(
            "const id = await dioxus.recv();
            let pressed = false;
            const down = e => {
                pressed = e.target.id === id;
                if (pressed) e.stopImmediatePropagation();
            };
            const click = e => { if (pressed && e.target.id === id) dioxus.send(true); };
            window.addEventListener('pointerdown', down, true);
            window.addEventListener('click', click, true);
            await dioxus.recv();
            window.removeEventListener('pointerdown', down, true);
            window.removeEventListener('click', click, true);",
        );
        let _ = eval.send(id);
        spawn(async move {
            while let Ok(true) = eval.recv().await {
                on_open_change.call(false);
            }
        });
        eval
    });
    use_drop(move || {
        let _ = eval.send(false);
    });
}
