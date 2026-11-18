//! Copying a table as text, to paste it into a spreadsheet or a message.

use dioxus::prelude::*;
use tournament_app::view::tab_separated;

use crate::{announcement::Announcement, competition::Ctx, icons};

/// A button copying `rows`, the cells of a table as it shows them, its header first, as text
/// (see [`tab_separated`]), named by what it copies, `label`, e.g. "Copy the standings".
#[component]
pub fn CopyTable(label: String, rows: Vec<Vec<String>>) -> Element {
    let ctx = use_context::<Ctx>();
    let tr = ctx.translator.read();
    let text = tab_separated(&rows);
    rsx! {
        button {
            class: "copy",
            r#type: "button",
            aria_label: "{label}",
            onclick: move |_| {
                let text = text.clone();
                async move {
                    let copy = document::eval(
                        "await navigator.clipboard.writeText(await dioxus.recv()); return true;",
                    );
                    if copy.send(text).is_ok() && copy.await.is_ok() {
                        ctx.announce(Announcement::Copied);
                    }
                }
            },
            icons::Copy {}
            {tr.get("copy")}
        }
    }
}
