//! The bracket of a knockout phase, every match in it from the start, drawn as columns of stages
//! joined by lines. Read in order, it is the list of the matches of each stage, which is what a
//! screen reader gets.
//!
//! The result of a match is reported by a click on the side that won, whose name is a button
//! until then; the focus moves to the bracket's heading, rather than to a side of the next match,
//! which a second Enter would report without the operator having read it.

use dioxus::prelude::*;
use tournament_app::{
    i18n::Translator,
    view::{
        BracketColumn, BracketEntry, BracketLine, BracketPairing, BracketPart, BracketSection,
        BracketSource, MatchRow, StageName, Unplayed,
    },
};
use tournament_engine::MatchResult;

use crate::{
    announcement::Announcement,
    competition::Ctx,
    i18n::t,
    play::{move_focus, use_focus},
    result_text, scores_text, team_name,
};

/// What gets the focus once the page shows a result reported in the bracket.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Focus {
    /// The heading of the bracket.
    Heading,
}

/// The bracket, if the format of the phase plays one.
#[component]
pub fn Bracket() -> Element {
    let ctx = use_context::<Ctx>();
    let focus = use_signal(|| None::<Focus>);
    let on_heading_mounted = use_focus(focus, Focus::Heading);
    // the element scrolling, and whether more of the bracket is out of its sight to the left and
    // to the right, which fades out at that edge
    let mut scrolling = use_signal(|| None::<web_sys::Element>);
    let mut more = use_signal(|| (false, false));
    let mut measure = move || {
        if let Some(element) = scrolling.peek().as_ref() {
            let left = element.scroll_left();
            let measured = (
                left > 1,
                left + element.client_width() < element.scroll_width() - 1,
            );
            if *more.peek() != measured {
                more.set(measured);
            }
        }
    };
    let Some(bracket) = ctx.session.read().bracket() else {
        return rsx! {};
    };
    let tr = ctx.translator.read();
    // said only to the tab that reports, while there is something to report
    let reports = ctx.actions().report;
    let (more_left, more_right) = more();
    rsx! {
        section { class: "card", aria_labelledby: "bracket",
            // focused once a result is reported, so that the focus is not lost
            h2 { id: "bracket", tabindex: "-1", onmounted: on_heading_mounted,
                {tr.get("bracket")}
            }
            if reports {
                p { class: "hint", {tr.get("bracket-report-hint")} }
            }
            // every section in one, scrolling sideways together if its columns do not fit, and
            // down within the screen if they are higher, their stages staying on top
            div {
                class: "bracket-frame",
                "data-more-left": more_left.then_some("true"),
                "data-more-right": more_right.then_some("true"),
                div {
                    class: "scroll bracket-scroll",
                    role: "group",
                    tabindex: "0",
                    aria_labelledby: "bracket",
                    onmounted: move |event| {
                        scrolling.set(event.data().downcast::<web_sys::Element>().cloned());
                        measure();
                    },
                    onscroll: move |_| measure(),
                    onresize: move |_| measure(),
                    for (index , section) in bracket.sections.into_iter().enumerate() {
                        Section {
                            key: "{index}",
                            index,
                            section,
                            focus,
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn Section(index: usize, section: BracketSection, focus: Signal<Option<Focus>>) -> Element {
    let ctx = use_context::<Ctx>();
    let tr = ctx.translator.read();
    // a single elimination is one bracket, its match for third place below the final
    let heading = match section.part {
        BracketPart::Knockout => None,
        BracketPart::Winners => Some(tr.get("bracket-winners")),
        BracketPart::Losers => Some(tr.get("bracket-losers")),
        BracketPart::GrandFinal => Some(tr.get("bracket-grand-final")),
    };
    let nested = heading.is_some();
    rsx! {
        div { class: "bracket-section",
            if let Some(heading) = heading {
                h3 { id: "bracket-{index}", {heading} }
            }
            div { class: "bracket",
                for (index , column) in section.columns.into_iter().enumerate() {
                    Column {
                        key: "{index}",
                        column,
                        nested,
                        focus,
                    }
                }
            }
        }
    }
}

/// A stage, headed by its name, a level below the section's heading if it has one, and the stage
/// drawn below it, if any.
#[component]
fn Column(column: BracketColumn, nested: bool, focus: Signal<Option<Focus>>) -> Element {
    // the pairings of each row, top to bottom
    let mut rows = vec![Vec::new(); column.rows];
    for pairing in column.pairings {
        let row = pairing.row;
        rows[row].push(pairing);
    }
    rsx! {
        div { class: "bracket-column",
            Stage { stage: column.stage, nested }
            div { class: "bracket-slots",
                ol {
                    for (index , mut pairings) in rows.into_iter().enumerate() {
                        match pairings.len() {
                            // the place of pairings left out, kept so that the others stay level
                            // with those they lead to
                            0 => rsx! {
                                li { key: "{index}", class: "slot", aria_hidden: "true" }
                            },
                            1 => rsx! {
                                Pairing { key: "{index}", pairing: pairings.remove(0), focus }
                            },
                            // two whose winners meet in the next column, each in half of the row
                            _ => rsx! {
                                li { key: "{index}", class: "slots",
                                    ol {
                                        for (index , pairing) in pairings.into_iter().enumerate() {
                                            Pairing { key: "{index}", pairing, focus }
                                        }
                                    }
                                }
                            },
                        }
                    }
                }
                if let Some(below) = column.below {
                    div { class: "below",
                        Stage { stage: below.stage, nested }
                        ol {
                            for (index , pairing) in below.pairings.into_iter().enumerate() {
                                Pairing { key: "{index}", pairing, focus }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The heading of a stage.
#[component]
fn Stage(stage: Option<StageName>, nested: bool) -> Element {
    let ctx = use_context::<Ctx>();
    let tr = ctx.translator.read();
    let stage = stage.as_ref().map(|stage| tr.stage(stage));
    rsx! {
        if nested {
            h4 { class: "bracket-stage", {stage} }
        } else {
            h3 { class: "bracket-stage", {stage} }
        }
    }
}

/// A pairing in its slot of the column, which the lines to and from it are drawn in.
#[component]
fn Pairing(pairing: BracketPairing, focus: Signal<Option<Focus>>) -> Element {
    let ctx = use_context::<Ctx>();
    let tr = ctx.translator.read();
    // its slot kept, so that the others stay where the lines meet them
    match pairing.unplayed {
        Some(Unplayed::Byes) => {
            return rsx! {
                li { class: "slot", aria_hidden: "true",
                    div { class: "pairing unplayed" }
                }
            };
        }
        Some(Unplayed::NotNeeded) => {
            return rsx! {
                li { class: "slot",
                    div { class: "pairing unplayed",
                        span { class: "if-needed", {tr.get("bracket-not-needed")} }
                    }
                }
            };
        }
        None => {}
    }
    let line_out = match pairing.line_out {
        None => "",
        Some(BracketLine::Straight) => "line-straight",
        Some(BracketLine::Down) => "line-down",
        Some(BracketLine::Up) => "line-up",
    };
    let line_in = if pairing.line_in { "line-in" } else { "" };
    let result = pairing.r#match.as_ref().and_then(|row| row.result);
    // a match with a result recedes, as in the list of matches, and so does a pairing decided by a
    // bye, which has nothing to play
    let bye = pairing.r#match.is_none() && pairing.sides.contains(&BracketEntry::Bye);
    let finished = if result.is_some() || bye {
        "finished"
    } else {
        ""
    };
    // only reported by the tab that changes the competition, and only by who won: a result with
    // more, e.g. a score, is entered in the list of matches
    let to_report = (result.is_none() && ctx.changes())
        .then(|| pairing.r#match.clone())
        .flatten()
        .filter(|row| !row.details);
    let scores = pairing
        .r#match
        .as_ref()
        .and_then(|row| scores_text(&tr, row));
    let side = |side: usize| {
        let entry = &pairing.sides[side];
        let class = if result == Some([MatchResult::AWins, MatchResult::BWins][side]) {
            "winner"
        } else {
            match entry {
                BracketEntry::Team(_) => "",
                // nobody, rather than somebody to come
                BracketEntry::Bye => "nobody",
                BracketEntry::WinnerOf(_) | BracketEntry::LoserOf(_) => "pending",
            }
        };
        rsx! {
            span { class: "side {class}",
                {entry_text(&tr, entry)}
                if pairing.after_bye[side] {
                    AfterBye {}
                }
            }
        }
    };
    rsx! {
        li { class: "slot {line_in} {line_out}",
            div { class: "pairing {finished}",
                // also before it is scheduled, since the sides waiting for it name it by its number
                if let Some(number) = pairing.number {
                    span { class: "number",
                        span { class: "visually-hidden", {tr.get("match")} }
                        " {number}"
                    }
                } else {
                    // empty, so that the names line up with those of the numbered pairings
                    span { class: "number", aria_hidden: "true" }
                }
                if let Some(row) = to_report {
                    WinButtons { row, after_bye: pairing.after_bye, focus }
                } else {
                    {side(0)}
                    span { class: "visually-hidden", {tr.get("versus")} }
                    {side(1)}
                }
                if let Some(text) = pairing.r#match.as_ref().and_then(|row| result_text(&tr, row)) {
                    span { class: "visually-hidden", ", {text}" }
                }
                if let Some(scores) = scores {
                    span { class: "scores", {scores} }
                }
                if pairing.only_if_needed {
                    span { class: "if-needed", {tr.get("if-needed")} }
                }
            }
        }
    }
}

/// The tag of a side that moved on by a bye, from a pairing left out of the column before.
#[component]
fn AfterBye() -> Element {
    let ctx = use_context::<Ctx>();
    let tr = ctx.translator.read();
    rsx! {
        span { class: "visually-hidden", "," }
        " "
        span { class: "tag", {tr.get("bracket-after-bye")} }
    }
}

/// The sides of the match `row`, which has no result yet, each a button reporting that it won (a
/// knockout has no draws), tagged where it moved on by a bye.
#[component]
fn WinButtons(row: MatchRow, after_bye: [bool; 2], focus: Signal<Option<Focus>>) -> Element {
    let ctx = use_context::<Ctx>();
    let id = row.id;
    let tr = ctx.translator.read();
    let (a, b) = (team_name(&tr, &row.a), team_name(&tr, &row.b));
    let (a_wins, b_wins) = (
        t!(tr, "wins", team = a.as_str()),
        t!(tr, "wins", team = b.as_str()),
    );
    let report = move |result| {
        move |_| {
            if let Some(reported) = ctx.act(move |session| session.report(id, result)) {
                ctx.announce(Announcement::Reported(reported));
                move_focus(focus, Focus::Heading);
            }
        }
    };
    // the name alone on the screen; what a click does is said under the pointer
    let button = |name: String, label: String, result, after_bye: bool| {
        let button = rsx! {
            button {
                class: "side",
                aria_label: "{label}",
                title: "{label}",
                onclick: report(result),
                "{name}"
            }
        };
        // the tag beside the button, whose name says only who wins
        if after_bye {
            rsx! {
                span { class: "tagged",
                    {button}
                    AfterBye {}
                }
            }
        } else {
            button
        }
    };
    rsx! {
        {button(a, a_wins, MatchResult::AWins, after_bye[0])}
        span { class: "visually-hidden", {tr.get("versus")} }
        {button(b, b_wins, MatchResult::BWins, after_bye[1])}
    }
}

/// Who takes a side, or where they come from while it is not decided.
fn entry_text(tr: &Translator, entry: &BracketEntry) -> String {
    // the stage, and the place in it where it has several pairings
    let stage = |source: &BracketSource| {
        let stage = source
            .stage
            .as_ref()
            .map(|stage| tr.stage(stage))
            .unwrap_or_default();
        match source.position {
            Some(position) => t!(tr, "stage-position", stage = stage, position = position),
            None => stage,
        }
    };
    match entry {
        BracketEntry::Team(team) => team_name(tr, team),
        BracketEntry::Bye => tr.get("bracket-bye"),
        BracketEntry::WinnerOf(source) => match source.number {
            Some(number) => t!(tr, "winner-of-match", number = number),
            None => t!(tr, "winner-of-stage", stage = stage(source)),
        },
        BracketEntry::LoserOf(source) => match source.number {
            Some(number) => t!(tr, "loser-of-match", number = number),
            None => t!(tr, "loser-of-stage", stage = stage(source)),
        },
    }
}
