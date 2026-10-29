//! What is done before the start: entering the entrants, adding the phase, starting.

use dioxus::prelude::*;
use tournament_app::{i18n::Message as _, session::Session};

use crate::{
    announcement::Announcement,
    competition::Ctx,
    i18n::t,
    phase::{NewPhase, PhaseSummary, focus_later},
    team_name,
};

#[component]
pub fn Entrants() -> Element {
    let ctx = use_context::<Ctx>();
    let mut name = use_signal(String::new);
    let entrants = ctx.session.read().entrants();
    // an entrant whose name the one typed looks like, e.g. differing in case only
    let like = ctx.session.read().entrant_like(&name.read());
    let actions = ctx.actions();
    let tr = ctx.translator.read();
    let entrants: Vec<_> = entrants
        .into_iter()
        .map(|entrant| {
            let name = team_name(&tr, std::slice::from_ref(&entrant));
            (entrant, name)
        })
        .collect();
    let count = entrants.len();
    // a competition without entrants, e.g. one just created, opens at the field of the first
    let opened_empty = use_hook(|| count == 0);
    rsx! {
        section { class: "card", aria_labelledby: "entrants",
            h2 { id: "entrants",
                {tr.get("entrants")}
                if count > 0 {
                    " "
                    span { class: "count", "({count})" }
                }
            }
            if count == 0 {
                p { class: "empty", {tr.get("no-entrants")} }
            }
            ol { id: "entrant-list",
                for (position, (row, name)) in entrants.into_iter().enumerate() {
                    li { key: "{row.id:?}",
                        span { class: "name", "{name}" }
                        button {
                            class: "small",
                            aria_label: t!(tr, "remove-entrant", name = name.as_str()),
                            disabled: !actions.remove_entrant,
                            onclick: move |_| {
                                let id = row.id;
                                if ctx.act(|session| session.remove_entrant(id)).is_some() {
                                    ctx.announce(Announcement::RemovedEntrant(row.clone()));
                                    // the button of the entrant after it, which takes its place,
                                    // else the field adding one
                                    let next = format!(
                                        "#entrant-list > li:nth-child({}) button",
                                        position + 1
                                    );
                                    focus_later(&[&next, "#entrant-name"]);
                                }
                            },
                            {tr.get("remove")}
                        }
                    }
                }
            }
            // below the list, so that the entrant just added shows right above the field, which
            // keeps the focus for the next
            form {
                class: "row",
                onsubmit: move |event| {
                    event.prevent_default();
                    if let Some(id) = ctx.act(|session| session.add_entrant(&name.peek())) {
                        name.set(String::new());
                        scroll_to_field();
                        let added = ctx.session.peek().entrants().into_iter().find(|e| e.id == id);
                        if let Some(added) = added {
                            ctx.announce(Announcement::AddedEntrant(added));
                        }
                    }
                },
                label { r#for: "entrant-name", class: "visually-hidden", {tr.get("name")} }
                input {
                    id: "entrant-name",
                    value: "{name}",
                    autocomplete: "off",
                    placeholder: tr.get("name"),
                    aria_describedby: "entrant-like",
                    onmounted: move |event| async move {
                        if opened_empty {
                            _ = event.data().set_focus(true).await;
                        }
                    },
                    oninput: move |event| name.set(event.value()),
                }
                button {
                    class: "primary",
                    r#type: "submit",
                    disabled: !actions.add_entrant,
                    {tr.get("add-entrant")}
                }
                // said as it is typed, without refusing the name
                p { id: "entrant-like", class: "warnings", role: "status",
                    if let Some(like) = like {
                        {t!(tr, "entrant-like", name = like.as_str())}
                    }
                }
            }
        }
    }
}

/// Scrolls the field of the name into view, once the entrant just added is rendered above it, so
/// that the list growing with the page does not push it out of sight.
fn scroll_to_field() {
    document::eval(
        "requestAnimationFrame(() => document.getElementById('entrant-name')\
         ?.form.scrollIntoView({ block: 'nearest' }))",
    );
}

/// The phase: its form until it is added, then how it is played; and the start.
#[component]
pub fn Phase() -> Element {
    let ctx = use_context::<Ctx>();
    let actions = ctx.actions();
    let has_phase = !ctx.session.read().competition().phases().is_empty();
    // what keeps the competition from starting, said only to the tab that could start it
    let refusal = ctx
        .changes()
        .then(|| ctx.session.read().start_refusal())
        .flatten()
        .map(|refusal| refusal.text());
    let tr = ctx.translator.read();
    rsx! {
        section { class: "card", aria_labelledby: "phase",
            // focused once the phase is added, unless it can start
            h2 { id: "phase", tabindex: "-1", {tr.get("phase-heading")} }
            if has_phase {
                PhaseSummary {}
            } else {
                NewPhase {}
            }
            // apart from the phase above it, the card's primary action once the phase is added
            div { class: "start",
                button {
                    id: "start",
                    class: if has_phase { "primary" },
                    disabled: !actions.start,
                    aria_describedby: refusal.is_some().then_some("start-refusal"),
                    onclick: move |_| {
                        if ctx.act(Session::start).is_some() {
                            let rounds = ctx.session.peek().rounds();
                            let matches = rounds.iter().map(|round| round.open).sum();
                            ctx.announce(Announcement::Started(matches));
                            // the setup is gone, and the matches take its place
                            focus_later(&["#matches"]);
                        }
                    },
                    {tr.get("start")}
                }
                if let Some(refusal) = &refusal {
                    " "
                    span { id: "start-refusal", class: "hint", {tr.sentence(refusal)} }
                }
            }
        }
    }
}
