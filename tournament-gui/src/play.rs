//! What is done after the start: reporting the results of the matches, round by round or those
//! that can be played now, and following the standings.
//!
//! A result can be typed in by the number of its match. After one is reported or reopened by its
//! buttons, which are gone then, the focus moves on to where the next one is reported, so that the
//! results of a round can be entered by keyboard alone.

use dioxus::prelude::*;
use tournament_app::{
    i18n::{Message as _, PluginText, Text, Translator},
    view::{
        ListedMatch, MatchRow, Medal, RoundSummary, RoundsView, Shown, StandingCell, TypedEntry,
    },
};
use tournament_engine::{MatchId, MatchResult};

use crate::{
    announcement::Announcement,
    capitalized,
    competition::{Ctx, Finished},
    focus::{move_focus, use_focus},
    i18n::t,
    icons,
    phase::answer,
    result::{ResultDetails, ResultOf},
    result_text, result_with_scores, team_name,
    toggletip::Toggletip,
};

/// What gets the focus once the page shows a result reported or reopened by its buttons.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Focus {
    /// The first button reporting a result of the match, or the first value of its result entered
    /// in its row.
    Report(MatchId),
    /// The heading of the matches shown, once none of them is left without a result.
    Heading,
}

#[component]
pub fn Rounds() -> Element {
    let ctx = use_context::<Ctx>();
    // the list the operator chose, if any
    let mut chosen = use_signal(|| None::<Shown>);
    let focus = use_signal(|| None::<Focus>);
    let on_heading_mounted = use_focus(focus, Focus::Heading);
    // whether the entry of results typed in is shown
    let typing = use_signal(|| remembered(RESULT_ENTRY_OPEN));
    // what is typed into it, which while there is any shows the matches it fits instead
    let entry = use_signal(String::new);
    let typed = if typing() {
        entry.read().clone()
    } else {
        String::new()
    };
    let Some(view) = ctx.session.read().rounds_view(chosen(), &typed) else {
        return rsx! {};
    };
    let heading_round = view.shown_round().cloned();
    let RoundsView {
        rounds,
        current,
        shown,
        previous,
        next,
        playable,
        draws,
        scores,
        finished,
        entry: typed,
        matches,
        byes,
        with_round,
        with_stage,
    } = view;
    let (entered, preview) = match typed {
        Some(TypedEntry { text, report }) => (Some(text), Some(report.map_err(|why| why.text()))),
        None => (None, None),
    };
    // the match whose result Enter in the entry reports
    let to_report = preview
        .as_ref()
        .and_then(|report| report.as_ref().ok())
        .map(|row| row.id);
    let tr = ctx.translator.read();
    rsx! {
        section { class: "card", aria_labelledby: "matches",
            div { class: "card-heading",
                // focused at the start
                h2 { id: "matches", tabindex: "-1", {tr.get("matches")} }
                if !finished {
                    ResultEntryToggle { open: typing }
                }
            }
            if typing() && !finished {
                ResultEntry {
                    draws,
                    scores,
                    entry,
                    preview: preview.clone(),
                    focus,
                }
            }
            // out of the way while the matches shown are those the entry fits, which they would not
            // change
            nav {
                class: "toolbar",
                aria_label: tr.get("rounds"),
                hidden: entered.is_some(),
                div {
                    class: "segmented",
                    role: "group",
                    aria_label: tr.get("matches-shown"),
                    button {
                        aria_pressed: shown == Shown::Playable,
                        onclick: move |_| chosen.set(Some(Shown::Playable)),
                        {t!(tr, "playable-now-count", count = playable)}
                    }
                    button {
                        aria_pressed: shown != Shown::Playable,
                        onclick: move |_| chosen.set(Some(Shown::Round(current))),
                        {tr.get("by-round")}
                    }
                }
                if let Shown::Round(round) = shown {
                    div { class: "stepper",
                        button {
                            class: "small",
                            aria_label: tr.get("previous-round"),
                            // focusable at the first round as well, so that the focus stays
                            aria_disabled: previous.is_none().then_some("true"),
                            onclick: move |_| {
                                if let Some(previous) = previous {
                                    chosen.set(Some(Shown::Round(previous)));
                                }
                            },
                            icons::ChevronLeft {}
                        }
                        label { r#for: "round", class: "visually-hidden", {tr.get("rounds")} }
                        select {
                            id: "round",
                            onchange: move |event| {
                                if let Ok(round) = event.value().parse() {
                                    chosen.set(Some(Shown::Round(round)));
                                }
                            },
                            for summary in rounds.iter() {
                                option {
                                    key: "{summary.round}",
                                    value: "{summary.round}",
                                    selected: summary.round == round,
                                    if summary.open > 0 {
                                        {
                                            t!(
                                                tr,
                                                "round-with-open",
                                                round = round_name(&tr, summary),
                                                open = summary.open,
                                            )
                                        }
                                    } else {
                                        {round_name(&tr, summary)}
                                    }
                                }
                            }
                        }
                        button {
                            class: "small",
                            aria_label: tr.get("next-round"),
                            aria_disabled: next.is_none().then_some("true"),
                            onclick: move |_| {
                                if let Some(next) = next {
                                    chosen.set(Some(Shown::Round(next)));
                                }
                            },
                            icons::ChevronRight {}
                        }
                    }
                }
            }
            // focused once no match shown is left without a result, so that the focus is not lost
            h3 { tabindex: "-1", onmounted: on_heading_mounted,
                match (&entered, &heading_round) {
                    (Some(text), _) => t!(tr, "entry-found", text = text.as_str()),
                    (None, Some(summary)) => round_name(&tr, summary),
                    (None, None) => tr.get("playable-now"),
                }
            }
            if matches.is_empty() && byes.is_empty() {
                if let Some(text) = &entered {
                    p { class: "empty", {t!(tr, "entry-nothing-found", text = text.as_str())} }
                } else if shown == Shown::Playable {
                    p { class: "empty", {tr.get("nothing-playable")} }
                }
            }
            if !matches.is_empty() {
                ol { id: "match-list",
                    for ListedMatch { row, next_open } in matches {
                        MatchLine {
                            key: "{row.id:?}",
                            with_round,
                            with_stage,
                            draws,
                            focus,
                            after_report: next_open.map_or(Focus::Heading, Focus::Report),
                            to_report: to_report == Some(row.id),
                            row,
                        }
                    }
                }
            }
            for bye in byes {
                p { class: "bye",
                    {
                        let team = team_name(&tr, &bye.entrants);
                        match bye.stage {
                            Some(stage) => {
                                t!(tr, "bye-in-stage", team = team, stage = tr.stage(& stage))
                            }
                            None => t!(tr, "bye", team = team),
                        }
                    }
                }
            }
        }
    }
}

/// The name of the round of `summary`: the stage it is of, if one, else its number.
fn round_name(tr: &Translator, summary: &RoundSummary) -> String {
    match &summary.stage {
        Some(stage) => capitalized(&tr.stage(stage)),
        None => t!(tr, "round", round = summary.round),
    }
}

/// The button in the heading of the matches showing the entry of results typed in, or hiding it
/// again, which shows whether it is open; the browser remembers what the operator left it at.
#[component]
fn ResultEntryToggle(open: Signal<bool>) -> Element {
    let ctx = use_context::<Ctx>();
    let tr = ctx.translator.read();
    rsx! {
        button {
            id: "result-entry-toggle",
            class: "quiet small",
            r#type: "button",
            aria_expanded: open(),
            onclick: move |_| {
                let opened = !open();
                open.set(opened);
                remember(RESULT_ENTRY_OPEN, opened);
            },
            {tr.get("type-results")}
            icons::ChevronDown {}
        }
    }
}

/// Reports a result typed in as `entry`, the number of its match and the result, e.g. "3 1", or
/// part of the winner's name, saying while it is typed what it would report, or why nothing. Its
/// hint folds behind a toggletip once a result typed in was reported in this browser.
#[component]
fn ResultEntry(
    draws: bool,
    scores: bool,
    entry: Signal<String>,
    /// The match and result reported if submitted now, or why none; nothing while nothing is
    /// typed.
    preview: Option<Result<MatchRow, Text>>,
    focus: Signal<Option<Focus>>,
) -> Element {
    let ctx = use_context::<Ctx>();
    // why what was entered last was not reported, until it is changed
    let mut error = use_signal(|| None::<Text>);
    let mut used = use_signal(|| remembered(RESULT_ENTRY_USED));
    // only reported by the tab that changes the competition
    let disabled = !ctx.changes();
    let tr = ctx.translator.read();
    let preview = preview.map(|report| match report {
        Ok(row) => (
            "preview",
            t!(
                tr,
                "result-entry-preview",
                number = row.number,
                result = result_with_scores(&tr, &row).unwrap_or_default(),
            ),
        ),
        Err(why) => ("preview hint", tr.sentence(&why)),
    });
    let mut hint = t!(
        tr,
        "enter-result-hint",
        draws = if draws { "yes" } else { "no" }
    );
    if scores {
        hint.push(' ');
        hint.push_str(&tr.get("enter-scores-hint"));
    }
    rsx! {
        form {
            class: "result-entry",
            onsubmit: move |event| {
                event.prevent_default();
                let entered = entry.peek().clone();
                match ctx.try_act(|session| session.report_entered(&entered)) {
                    Ok(reported) => {
                        ctx.announce(Announcement::Reported(reported));
                        entry.set(String::new());
                        error.set(None);
                        if !used() {
                            used.set(true);
                            remember(RESULT_ENTRY_USED, true);
                        }
                        // the entry is gone with the last result, and the focus with it
                        if ctx.session.peek().winners().is_some() {
                            focus.set(Some(Focus::Heading));
                        }
                    }
                    Err(refused) => error.set(Some(refused)),
                }
            },
            label { r#for: "result-entry", class: "visually-hidden", {tr.get("enter-result")} }
            input {
                id: "result-entry",
                value: "{entry}",
                autocomplete: "off",
                aria_invalid: error.read().is_some(),
                aria_describedby: "result-entry-error result-entry-preview result-entry-hint",
                disabled,
                oninput: move |event| {
                    entry.set(event.value());
                    error.set(None);
                },
            }
            " "
            button { r#type: "submit", disabled, {tr.get("report")} }
            if used() {
                Toggletip {
                    id: "result-entry-hint",
                    subject: tr.get("typing-results"),
                    text: hint.clone(),
                }
            }
            p { id: "result-entry-error", class: "error", role: "alert",
                if let Some(error) = error() {
                    {tr.sentence(&error)}
                }
            }
            // said by the alert above instead once Enter was refused; a live region there before
            // anything is typed, so that a screen reader says it as it changes
            {
                let (class, text) = preview
                    .filter(|_| error.read().is_none())
                    .map_or(("preview", None), |(class, text)| (class, Some(text)));
                rsx! {
                    p { id: "result-entry-preview", class, role: "status", {text} }
                }
            }
            if !used() {
                p { id: "result-entry-hint", class: "hint", {hint} }
            }
        }
    }
}

/// Where the browser remembers that the operator opened the entry of results.
const RESULT_ENTRY_OPEN: &str = "tournamo.result-entry-open";

/// Where the browser remembers that a result typed in was reported, after which the hint of the
/// entry is folded.
const RESULT_ENTRY_USED: &str = "tournamo.result-entry-used";

/// Whether this browser remembers `key`, see [`remember`].
fn remembered(key: &str) -> bool {
    local_storage()
        .and_then(|storage| storage.get_item(key).ok().flatten())
        .is_some()
}

/// Makes this browser remember `key` if `set`, or else forget it.
fn remember(key: &str, set: bool) {
    // without local storage, it is forgotten on the next visit
    if let Some(storage) = local_storage() {
        _ = if set {
            storage.set_item(key, "true")
        } else {
            storage.remove_item(key)
        };
    }
}

fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

#[component]
fn MatchLine(
    row: MatchRow,
    with_round: bool,
    with_stage: bool,
    draws: bool,
    focus: Signal<Option<Focus>>,
    /// What gets the focus once a result of the match is reported by its buttons.
    after_report: Focus,
    /// Whether Enter in the entry of results reports its result, which marks it.
    to_report: bool,
) -> Element {
    let ctx = use_context::<Ctx>();
    let tr = ctx.translator.read();
    let winner = |side| {
        let won = match side {
            Side::A => MatchResult::AWins,
            Side::B => MatchResult::BWins,
        };
        if row.result == Some(won) {
            "winner"
        } else {
            ""
        }
    };
    // one card: the pairing around the number, then each side's button below its name; once it
    // has a result, one line, the button reopening it after the pairing
    rsx! {
        li {
            class: "match",
            class: if row.result.is_some() { "finished" },
            class: if to_report { "to-report" },
            span { class: "visually-hidden", {tr.get("match")} }
            span { class: "number", "{row.number}" }
            span { class: "side a {winner(Side::A)}", {team_name(&tr, &row.a)} }
            span { class: "visually-hidden", {tr.get("versus")} }
            span { class: "side b {winner(Side::B)}", {team_name(&tr, &row.b)} }
            if with_round || with_stage {
                span { class: "meta",
                    // a stage names the round already
                    if with_round && row.stage.is_none() {
                        span { class: "round", {t!(tr, "round", round = row.round)} }
                    }
                    if with_stage {
                        span { class: "stage", {row.stage.as_ref().map(|stage| tr.stage(stage))} }
                    }
                }
            }
            div { class: "report",
                if row.result.is_some() {
                    ReportedResult { row: row.clone(), draws, focus }
                } else if row.details {
                    ResultOf { row: row.clone(), focus, after_report }
                } else {
                    ReportButtons {
                        row: row.clone(),
                        draws,
                        focus,
                        after_report,
                    }
                }
            }
        }
    }
}

/// The side of a match shown on the left, A, or on the right, B.
#[derive(Clone, Copy)]
enum Side {
    A,
    B,
}

/// The buttons reporting a result of the match `row`, which has none yet.
#[component]
fn ReportButtons(
    row: MatchRow,
    draws: bool,
    focus: Signal<Option<Focus>>,
    after_report: Focus,
) -> Element {
    let ctx = use_context::<Ctx>();
    let id = row.id;
    let on_mounted = use_focus(focus, Focus::Report(id));
    // only reported by the tab that changes the competition
    let disabled = !ctx.changes();
    let tr = ctx.translator.read();
    let (a, b) = (team_name(&tr, &row.a), team_name(&tr, &row.b));
    let report = move |result| {
        move |_| {
            if let Some(reported) = ctx.act(move |session| session.report(id, result)) {
                ctx.announce(Announcement::Reported(reported));
                move_focus(focus, after_report);
            }
        }
    };
    // each below its side's name, so that it names the side only for screen readers
    rsx! {
        button {
            class: "win a",
            disabled,
            aria_label: t!(tr, "wins", team = a.as_str()),
            onmounted: on_mounted,
            onclick: report(MatchResult::AWins),
            {tr.get("wins-short")}
        }
        if draws {
            button {
                class: "draw",
                disabled,
                aria_label: t!(tr, "draw-of", a = a.as_str(), b = b.as_str()),
                onclick: report(MatchResult::Draw),
                {tr.get("draw")}
            }
        }
        button {
            class: "win b",
            disabled,
            aria_label: t!(tr, "wins", team = b.as_str()),
            onclick: report(MatchResult::BWins),
            {tr.get("wins-short")}
        }
    }
}

/// How long after a report the button reopening a match takes no click, in milliseconds: longer
/// than the taps of a double tap are apart.
const REOPEN_DELAY_MS: f64 = 600.0;

/// The result of the match `row`, of a round where a match may end in a draw if `draws`, and the
/// button reopening it, which takes no click until [`REOPEN_DELAY_MS`] after this tab last reported
/// a result.
#[component]
fn ReportedResult(row: MatchRow, draws: bool, focus: Signal<Option<Focus>>) -> Element {
    let ctx = use_context::<Ctx>();
    let id = row.id;
    // the button shows where the result was reported, on a phone where the draw was, so right
    // after a report it takes no click, which would be the second of a double tap; shown later
    // (another round, a reload), it takes one at once
    let wait = use_hook(|| {
        ctx.since_report()
            .map_or(0.0, |since| (REOPEN_DELAY_MS - since).max(0.0))
    });
    let mut settled = use_signal(|| wait == 0.0);
    use_hook(|| {
        if wait > 0.0 {
            spawn(async move {
                _ = document::eval(&format!(
                    "await new Promise((done) => setTimeout(done, {}));",
                    wait.ceil()
                ))
                .await;
                settled.set(true);
            });
        }
    });
    // only reopened by the tab that changes the competition
    let disabled = !ctx.changes() || !settled();
    let tr = ctx.translator.read();
    // a win is shown by the mark at the winner's name, so said only to screen readers, which may
    // not read the mark; a draw has no such mark
    let shown = row.result == Some(MatchResult::Draw);
    let scored = row.parts_won.is_some() || !row.scores.is_empty();
    // the games won, then the scores of the games, as `Translator::match_scores` writes them, the
    // latter giving way where the list is narrow
    let (won, games) = match row.parts_won {
        Some(won) => (
            Some(tr.score(won)),
            (!row.scores.is_empty()).then(|| format!("({})", tr.scores(&row.scores))),
        ),
        None => (None, Some(tr.scores(&row.scores))),
    };
    rsx! {
        if scored {
            // the scores say how it ended; who won is said to screen readers as well
            span { class: "visually-hidden", {result_text(&tr, &row)} }
            span { class: "result scores",
                if let Some(won) = won.clone() {
                    span { class: "parts-won", {won} }
                }
                if won.is_some() && games.is_some() {
                    " "
                }
                if let Some(games) = games {
                    span { class: if won.is_some() { "game-scores" }, {games} }
                }
            }
        } else {
            span { class: if shown { "result" } else { "result visually-hidden" },
                {result_text(&tr, &row)}
            }
        }
        // where a match may end in a draw, a win keeps the draw's room, so that the pairings of
        // every finished match line up
        if draws && !shown && !scored {
            span { class: "result-room", aria_hidden: "true", {tr.get("draw")} }
        }
        if row.details {
            ResultDetails { row: row.clone() }
        }
        div { class: "finished-actions",
            button {
                class: "small",
                disabled,
                aria_label: t!(tr, "reopen-match", number = row.number),
                onclick: move |_| {
                    if let Some(reopened) = ctx.act(move |session| session.undo_report(id)) {
                        ctx.announce(Announcement::Reopened(reopened));
                        move_focus(focus, Focus::Report(id));
                    }
                },
                {tr.get("reopen")}
            }
        }
    }
}

/// The rank of a place with a medal, on a disc in the medal's colour, said to screen readers by the
/// medal's name before it.
#[component]
fn MedalOf(medal: Medal, rank: u32) -> Element {
    let ctx = use_context::<Ctx>();
    let tr = ctx.translator.read();
    let (class, name) = match medal {
        Medal::Gold => ("gold", "medal-gold"),
        Medal::Silver => ("silver", "medal-silver"),
        Medal::Bronze => ("bronze", "medal-bronze"),
    };
    rsx! {
        span { class: "place medal {class}",
            span { class: "visually-hidden", {tr.get(name)} }
            "{rank}"
        }
    }
}

#[component]
pub fn Standings() -> Element {
    let ctx = use_context::<Ctx>();
    let Some(standings) = ctx.session.read().standings() else {
        return rsx! {};
    };
    // the settings of the calculators, e.g. the points for a win
    let form = ctx
        .session
        .read()
        .phase_form_to_edit()
        .ok()
        .map(|form| form.view());
    let tr = ctx.translator.read();
    let headers: Vec<_> = standings
        .columns
        .iter()
        .map(|column| {
            let header =
                capitalized(&tr.plugin(column.calculator, PluginText::Column, column.header));
            let label = capitalized(&tr.plugin(column.calculator, PluginText::Label, column.label));
            let description = column
                .description
                .map(|english| tr.plugin(column.calculator, PluginText::Description, english));
            // the metric's whole name where the header abbreviates it, e.g. "SB"
            let about = match description {
                Some(description) if label != header => Some(format!("{label}: {description}")),
                Some(description) => Some(description),
                None => (label != header).then_some(label),
            };
            let settings: Vec<_> = form
                .as_ref()
                .and_then(|form| form.fields_of(column.calculator))
                .unwrap_or_default()
                .iter()
                .map(|field| format!("{}: {}", tr.field_label(field), answer(&tr, &field.input)))
                .collect();
            let about = match about {
                _ if settings.is_empty() => about,
                Some(about) => Some(format!("{about} ({})", settings.join(", "))),
                None => Some(settings.join(", ")),
            };
            // ids of plugins are namespaced by `::`
            let id = format!("standings-about-{}", column.calculator.replace("::", "-"));
            (header, about, id)
        })
        .collect();
    // above the standings rather than above the matches, which it would push down below the tap
    // that finished them
    let winners = ctx.session.read().winners();
    rsx! {
        if let Some(winners) = winners {
            Finished { winners }
        }
        section { class: "card", aria_labelledby: "standings",
            h2 { id: "standings", {tr.get("standings")} }
            // scrolls sideways if its columns do not fit, the names staying in sight
            div {
                class: "scroll",
                role: "group",
                tabindex: "0",
                aria_labelledby: "standings",
                table { id: "standing-list",
                    // the heading of the card shows it
                    caption { class: "visually-hidden", {tr.get("standings")} }
                    thead {
                        tr {
                            th { class: "rank", scope: "col", {tr.get("rank")} }
                            th { class: "name", scope: "col", {tr.get("name")} }
                            // named by the header alone, which the cells are read with, rather
                            // than with the toggletip's button too; what ranks first right after
                            // the names, the matches played last
                            for (header, about, id) in headers {
                                th { scope: "col", aria_label: "{header}",
                                    "{header}"
                                    if let Some(text) = about {
                                        " "
                                        Toggletip { id, subject: header.clone(), text }
                                    }
                                }
                            }
                            th { scope: "col", {tr.get("played")} }
                        }
                    }
                    tbody {
                        for row in standings.rows {
                            // the places with a medal, once every match is played, stand out
                            // the metrics of an entrant yet to play recede, all alike
                            tr {
                                class: if row.medal.is_some() { "podium" },
                                class: if row.played == 0 { "unplayed" },
                                "data-rank": row.medal.map(|_| row.rank.get()),
                                td { class: "rank",
                                    if let Some(medal) = row.medal {
                                        MedalOf { medal, rank: row.rank.get() }
                                    } else {
                                        span { class: "place", "{row.rank}" }
                                    }
                                }
                                th { class: "name", scope: "row", {team_name(&tr, &row.entrants)} }
                                for (column, cell) in standings.columns.iter().zip(row.cells) {
                                    td { class: "metric",
                                        {
                                            cell.map(|cell| match cell {
                                                StandingCell::Metric(cell) => {
                                                    capitalized(&tr.cell(column.calculator, &cell))
                                                }
                                                StandingCell::Stage(stage) => {
                                                    capitalized(&tr.stage(&stage))
                                                }
                                            })
                                        }
                                    }
                                }
                                td { class: "played", "{row.played}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
