//! The phases of a competition after its start: the links to each and to the ranking, starting a
//! phase, and the ranking of the competition.

use dioxus::prelude::*;
use tournament_app::{
    error::StartPhaseError,
    i18n::{Message as _, Text, Translator},
    session::Session,
    storage::CompetitionKey,
    view::{PhaseOverview, PhaseState, SourceRow},
};
use tournament_engine::PhaseId;

use crate::{
    announcement::Announcement,
    competition::{Ctx, phase_fragment, ranking_fragment},
    components::dialog::{Dialog, DialogDescription, DialogTitle},
    focus::focus_later,
    go_to,
    i18n::t,
    icons,
    phase::AddPhaseDialog,
    play::MedalOf,
    team_name,
};

/// The links to the phases, each with its state, and to the ranking of the competition, and the
/// button adding a phase; nothing while the competition has one phase.
#[component]
pub fn PhaseLinks(competition: CompetitionKey, ranking: bool) -> Element {
    let ctx = use_context::<Ctx>();
    let phases = ctx.session.read().phases();
    if phases.len() < 2 {
        return rsx! {
            if ctx.actions().add_phase {
                div { class: "phase-links-add", AddPhaseDialog {} }
            }
        };
    }
    let shown = ctx.phase();
    let tr = ctx.translator.read();
    let count = phases.len();
    rsx! {
        nav { class: "phase-links", aria_label: tr.get("phases"),
            ul {
                for (index, phase) in phases.iter().enumerate() {
                    li { key: "{index}",
                        a {
                            href: "#{phase_fragment(competition, index + 1, count, None)}",
                            aria_current: (!ranking && shown == Some(phase.id)).then_some("page"),
                            StateIcon { state: phase.state.clone() }
                            span { {tr.phase_name(&phase.name)} }
                        }
                    }
                }
                li {
                    a {
                        href: "#{ranking_fragment(competition)}",
                        aria_current: ranking.then_some("page"),
                        icons::Trophy {}
                        span { {tr.get("ranking")} }
                    }
                }
            }
            if ctx.actions().add_phase {
                AddPhaseDialog {}
            }
        }
    }
}

/// The state of a phase as an icon, named for screen readers.
#[component]
fn StateIcon(state: PhaseState) -> Element {
    let ctx = use_context::<Ctx>();
    let tr = ctx.translator.read();
    let (icon, text) = match state {
        PhaseState::NotStarted { .. } => (rsx! { icons::Circle {} }, "phase-not-started"),
        PhaseState::Running => (rsx! { icons::HalfCircle {} }, "phase-running"),
        PhaseState::Finished { .. } => (rsx! { icons::Check {} }, "phase-done"),
    };
    rsx! {
        span { class: "phase-state",
            {icon}
            span { class: "visually-hidden", {tr.get(text)} " " }
        }
    }
}

/// Where a phase takes its entrants from, as a list says it, e.g. "Places 1–8 of Group phase".
pub fn sources_text(tr: &Translator, phases: &[PhaseOverview], sources: &[SourceRow]) -> String {
    let texts: Vec<_> = sources
        .iter()
        .map(|source| {
            let of = source.phase.and_then(|id| {
                phases
                    .iter()
                    .find(|phase| phase.id == id)
                    .map(|phase| tr.phase_name(&phase.name))
            });
            let everyone = source.from.get() == 1 && source.to.is_none();
            match (of, everyone) {
                (None, true) => tr.get("everyone-registered"),
                (Some(of), true) => t!(tr, "every-place-of", phase = of),
                (of, false) => {
                    let of = of.unwrap_or_else(|| tr.get("registration"));
                    match source.to {
                        Some(to) => t!(
                            tr,
                            "places-of",
                            from = source.from.get(),
                            to = to.get(),
                            phase = of
                        ),
                        None => t!(tr, "places-from-of", from = source.from.get(), phase = of),
                    }
                }
            }
        })
        .collect();
    crate::capitalized(&texts.join(&tr.get("list-separator")))
}

/// The button starting the phase `phase`, the primary action where `primary`: `aria-disabled`
/// with the reason while it cannot start; a tie at a cut is asked about first.
#[component]
pub fn StartPhaseButton(phase: PhaseId, primary: bool) -> Element {
    let ctx = use_context::<Ctx>();
    let page = use_context::<crate::Page>();
    let mut tie = use_signal(|| None::<Text>);
    let refusal = ctx.session.read().start_phase_refusal(phase);
    let refused = match &refusal {
        None | Some(StartPhaseError::CutThroughTie(_)) => None,
        Some(refusal) => Some(refusal.text()),
    };
    let name = ctx.session.read().name_of_phase(phase);
    let tr = ctx.translator.read();
    let label = match &name {
        Some(name) => t!(tr, "start-phase", phase = tr.phase_name(name)),
        None => tr.get("start"),
    };
    let disabled = !ctx.changes() || refused.is_some();
    let described = format!("start-refusal-{}", phase_id_text(phase));
    let start = move || {
        if ctx
            .act(move |session: &mut Session| session.start_phase(phase))
            .is_some()
        {
            let session = ctx.session.peek();
            let phases = session.phases();
            let matches =
                phases
                    .iter()
                    .find(|overview| overview.id == phase)
                    .map_or(0, |overview| {
                        overview
                            .divisions
                            .iter()
                            .map(|division| division.matches - division.played)
                            .sum()
                    });
            let number = phases
                .iter()
                .position(|overview| overview.id == phase)
                .map_or(1, |index| index + 1);
            ctx.announce(Announcement::StartedPhase(name.clone(), matches));
            if let crate::Screen::Competition(key, _) = *page.screen.peek() {
                go_to(&phase_fragment(key, number, phases.len(), None));
            }
            // the page shows the phase started, headed by its matches
            focus_later(&["#matches"]);
        }
    };
    let start_again = start.clone();
    rsx! {
        button {
            class: if primary { "primary" },
            aria_disabled: disabled.then_some("true"),
            aria_describedby: refused.is_some().then(|| described.clone()),
            onclick: move |_| {
                if disabled {
                    return;
                }
                // bound first: a guard in the scrutinee lives through the arms, and starting
                // writes the session
                let refusal = ctx.session.peek().start_phase_refusal(phase);
                match refusal {
                    Some(warning @ StartPhaseError::CutThroughTie(_)) => {
                        tie.set(Some(warning.text()));
                    }
                    _ => start(),
                }
            },
            {label}
        }
        if let Some(refused) = &refused {
            " "
            span { id: "{described}", class: "hint", {tr.sentence(refused)} }
        }
        Dialog {
            open: Some(tie.read().is_some()),
            on_open_change: move |open: bool| {
                if !open {
                    tie.set(None);
                }
            },
            DialogTitle { {tr.get("tie-at-cut-title")} }
            DialogDescription {
                if let Some(tie) = tie() {
                    {tr.sentence(&tie)}
                }
                " "
                {tr.get("tie-at-cut-description")}
            }
            div { class: "dialog-buttons",
                button {
                    class: "primary",
                    onclick: move |_| {
                        tie.set(None);
                        start_again();
                    },
                    {tr.get("start-anyway")}
                }
                button { onclick: move |_| tie.set(None), {tr.get("cancel")} }
            }
        }
    }
}

/// The id of `phase` as the id of an element takes it.
pub fn phase_id_text(phase: PhaseId) -> String {
    format!("{phase:?}")
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect()
}

/// The ranking of the competition: every entrant's place, the phase they reached, and a note
/// while it is not final.
#[component]
pub fn Ranking() -> Element {
    let ctx = use_context::<Ctx>();
    let session = ctx.session.read();
    let ranking = session.ranking();
    let phases = session.phases();
    drop(session);
    let tr = ctx.translator.read();
    rsx! {
        div { class: "columns play",
            div { class: "column wide",
                section { class: "card", aria_labelledby: "ranking",
                    h2 { id: "ranking", tabindex: "-1", {tr.get("ranking")} }
                    if !ranking.is_final {
                        p { class: "hint", {tr.get("ranking-provisional")} }
                    }
                    div {
                        class: "scroll",
                        role: "group",
                        tabindex: "0",
                        aria_labelledby: "ranking",
                        table { id: "ranking-list",
                            caption { class: "visually-hidden", {tr.get("ranking")} }
                            thead {
                                tr {
                                    th { class: "rank", scope: "col", {tr.get("place")} }
                                    th { class: "name", scope: "col", {tr.get("name")} }
                                    th { scope: "col", {tr.get("phase-reached")} }
                                }
                            }
                            tbody {
                                for row in ranking.rows {
                                    tr { class: if row.medal.is_some() { "podium" },
                                        td { class: "rank",
                                            if let Some(medal) = row.medal {
                                                MedalOf { medal, rank: row.place.get() }
                                            } else {
                                                span { class: "place", "{row.place}" }
                                            }
                                        }
                                        th { class: "name", scope: "row",
                                            {team_name(&tr, &row.entrants)}
                                        }
                                        td {
                                            {
                                                phases
                                                    .iter()
                                                    .find(|phase| phase.id == row.phase)
                                                    .map(|phase| tr.phase_name(&phase.name))
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
