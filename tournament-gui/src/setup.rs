//! What is done before the start: entering the entrants, adding the phase, starting.

use dioxus::prelude::*;
use tournament_app::{
    i18n::{Message as _, Text},
    session::Session,
    view::EntrantRow,
};
use wasm_bindgen::JsCast as _;

use crate::{
    announcement::Announcement,
    competition::Ctx,
    help,
    i18n::t,
    icons,
    notices::use_cleared_on_language_change,
    phase::{NewPhase, PhaseSummary, focus_later},
    team_name,
};

#[component]
pub fn Entrants() -> Element {
    let ctx = use_context::<Ctx>();
    let mut name = use_signal(String::new);
    // why the name was refused, shown at its field until it is changed
    let mut error = use_signal(|| None::<Text>);
    use_cleared_on_language_change(ctx.translator, error);
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
                p { class: "empty",
                    {tr.get("no-entrants")}
                    " "
                    HelpLink { section: help::RUNNING, text: tr.get("how-it-works") }
                }
            }
            ol { id: "entrant-list",
                for (position, (row, name)) in entrants.into_iter().enumerate() {
                    EntrantItem { key: "{row.id:?}", row, name, position }
                }
            }
            // below the list, so that the entrant just added shows right above the field, which
            // keeps the focus for the next
            form {
                class: "row",
                onsubmit: move |event| {
                    event.prevent_default();
                    // as `Ctx::act`, but with why it is refused at the field
                    if !ctx.changes() {
                        return;
                    }
                    match ctx.try_act(|session| session.add_entrant(&name.peek())) {
                        Ok(id) => {
                            name.set(String::new());
                            scroll_to_field();
                            let added =
                                ctx.session.peek().entrants().into_iter().find(|e| e.id == id);
                            if let Some(added) = added {
                                ctx.announce(Announcement::AddedEntrant(added));
                            }
                        }
                        Err(refused) => error.set(Some(refused)),
                    }
                },
                label { r#for: "entrant-name", class: "visually-hidden", {tr.get("name")} }
                input {
                    id: "entrant-name",
                    value: "{name}",
                    autocomplete: "off",
                    placeholder: tr.get("name"),
                    aria_invalid: error.read().is_some().then_some("true"),
                    aria_describedby: "entrant-name-error entrant-like",
                    onmounted: move |event| async move {
                        if opened_empty {
                            _ = event.data().set_focus(true).await;
                        }
                    },
                    oninput: move |event| {
                        error.set(None);
                        name.set(event.value());
                    },
                    // a list pasted adds an entrant for each of its lines, which a field of one
                    // line would join
                    onpaste: move |event| {
                        let Some(lines) = pasted_lines(&event) else {
                            return;
                        };
                        event.prevent_default();
                        if !ctx.changes() {
                            return;
                        }
                        match ctx.try_act(|session| session.add_entrants(&lines)) {
                            Ok(added) => {
                                error.set(None);
                                scroll_to_field();
                                ctx.announce(Announcement::AddedEntrants(added.len()));
                            }
                            Err(refused) => error.set(Some(refused)),
                        }
                    },
                }
                button {
                    class: "primary",
                    r#type: "submit",
                    disabled: !actions.add_entrant,
                    {tr.get("add-entrant")}
                }
                p { id: "entrant-name-error", class: "error", role: "alert",
                    if let Some(error) = error() {
                        {tr.sentence(&error)}
                    }
                }
                // said as it is typed, before Enter refuses the name; once refused, the refusal
                // says it instead
                p { id: "entrant-like", class: "warnings", role: "status",
                    if let Some(like) = like.filter(|_| error.read().is_none()) {
                        {t!(tr, "entrant-like", name = like.as_str())}
                    }
                }
            }
        }
    }
}

/// An entrant of the list, `row` shown as `name`, at `position` from 0: its name, which is
/// renamed in place, and its removal.
#[component]
fn EntrantItem(row: EntrantRow, name: String, position: usize) -> Element {
    let ctx = use_context::<Ctx>();
    // the name typed while the entrant is renamed
    let mut renaming = use_signal(|| None::<String>);
    // why the new name was refused, shown at its field until it is changed
    let mut error = use_signal(|| None::<Text>);
    use_cleared_on_language_change(ctx.translator, error);
    let actions = ctx.actions();
    let tr = ctx.translator.read();
    let item = format!("#entrant-list > li:nth-child({})", position + 1);
    let field = format!("entrant-new-name-{position}");
    // back to the name, with the focus on the button renaming it
    let stop_renaming = {
        let rename_button = format!("{item} .rename");
        move || {
            renaming.set(None);
            error.set(None);
            focus_later(&[&rename_button]);
        }
    };
    let Some(typed) = renaming() else {
        let removed = row.clone();
        return rsx! {
            li {
                span { class: "name", "{name}" }
                // quiet, so that a long list is no column of outlines
                button {
                    class: "small quiet icon-only rename",
                    aria_label: t!(tr, "rename-entrant", name = name.as_str()),
                    title: t!(tr, "rename-entrant", name = name.as_str()),
                    disabled: !actions.rename_entrant,
                    onclick: move |_| renaming.set(Some(row.name.clone().unwrap_or_default())),
                    icons::Pencil {}
                }
                button {
                    class: "small quiet remove",
                    aria_label: t!(tr, "remove-entrant", name = name.as_str()),
                    disabled: !actions.remove_entrant,
                    onclick: move |_| {
                        let id = removed.id;
                        if ctx.act(|session| session.remove_entrant(id)).is_some() {
                            ctx.announce(Announcement::RemovedEntrant(removed.clone()));
                            // the entrant after it, which takes its place, else the field adding
                            // one
                            let next = format!("{item} .remove");
                            focus_later(&[&next, "#entrant-name"]);
                        }
                    },
                    {tr.get("remove")}
                }
            }
        };
    };
    let (mut on_escape, mut on_cancel, mut on_renamed) =
        (stop_renaming.clone(), stop_renaming.clone(), stop_renaming);
    rsx! {
        li {
            form {
                class: "row rename-entrant",
                onsubmit: move |event| {
                    event.prevent_default();
                    if !ctx.changes() {
                        return;
                    }
                    let id = row.id;
                    let typed = renaming.peek().clone().unwrap_or_default();
                    match ctx.try_act(|session| session.rename_entrant(id, &typed)) {
                        Ok(()) => {
                            let renamed = ctx
                                .session
                                .peek()
                                .entrants()
                                .into_iter()
                                .find(|entrant| entrant.id == id);
                            if let Some(renamed) = renamed.filter(|renamed| *renamed != row) {
                                ctx.announce(Announcement::RenamedEntrant(row.clone(), renamed));
                            }
                            on_renamed();
                        }
                        Err(refused) => error.set(Some(refused)),
                    }
                },
                label { r#for: "{field}", class: "visually-hidden",
                    {t!(tr, "entrant-new-name", name = name.as_str())}
                }
                input {
                    id: "{field}",
                    value: "{typed}",
                    autocomplete: "off",
                    aria_invalid: error.read().is_some().then_some("true"),
                    aria_describedby: "{field}-error",
                    onmounted: {
                        let field = field.clone();
                        move |_| {
                            document::eval(
                                &format!("document.getElementById('{field}')?.select()"),
                            );
                        }
                    },
                    oninput: move |event| {
                        error.set(None);
                        renaming.set(Some(event.value()));
                    },
                    onkeydown: move |event| {
                        if event.key() == Key::Escape {
                            on_escape();
                        }
                    },
                }
                button { class: "small primary", r#type: "submit", {tr.get("rename")} }
                button {
                    class: "small quiet",
                    r#type: "button",
                    onclick: move |_| on_cancel(),
                    {tr.get("cancel")}
                }
                p { id: "{field}-error", class: "error", role: "alert",
                    if let Some(error) = error() {
                        {tr.sentence(&error)}
                    }
                }
            }
        }
    }
}

/// A link saying `text` to the section `section` of the help, see [`help`], in a tab of its own,
/// so that the competition stays open here, and with it what can be undone.
#[component]
fn HelpLink(section: &'static str, text: String) -> Element {
    rsx! {
        a {
            class: "help-link",
            href: "#{section}",
            target: "_blank",
            rel: "noopener",
            {text}
        }
    }
}

/// The text `event` pastes, if it has more than one line that is not blank.
fn pasted_lines(event: &Event<ClipboardData>) -> Option<String> {
    let data = event.data();
    let event = data
        .downcast::<web_sys::Event>()?
        .dyn_ref::<web_sys::ClipboardEvent>()?;
    let text = event.clipboard_data()?.get_data("text").ok()?;
    let lines = text.lines().filter(|line| !line.trim().is_empty()).count();
    (lines > 1).then_some(text)
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
            p {
                HelpLink { section: help::FORMATS, text: tr.get("formats-explained") }
            }
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
