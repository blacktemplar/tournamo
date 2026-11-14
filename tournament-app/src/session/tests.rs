use std::{
    cell::Cell,
    cmp::Ordering,
    fmt,
    num::NonZeroU32,
    pin::pin,
    task::{Context, Poll, Waker},
};

use plugin_factory::{
    PluginType,
    config::{Path as ConfigPath, PluginConfig},
};
use tournament_engine::{
    EnteredValue, EntrantId, MatchId, MatchResult, PhasePlugins, Score, Sides,
    plugins::{
        match_format::{BestOf, Scored},
        metrics_calculator::{Cell as MetricCell, MoreWins, RoundOfFirstLossCalculator},
        phase_format::{
            DoubleElimination, MonsterDyp, RandomRounds, RoundRobin, SingleElimination,
        },
    },
    uuid::Uuid,
};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::{
    error::{
        AddEntrantError, AddPhaseError, EditPhaseError, ImportError, LoadError, NoPhase,
        PhaseDraftError, RedoError, RemoveEntrantError, RenameEntrantError, ReportEnteredError,
        ReportError, StartError, UndoError, UndoReportError,
    },
    file::{self},
    storage::{CompetitionKey, Listing, MemoryStorage, Progress, Save, Storage, Stored},
    view::{
        Action, BracketColumn, BracketEntry, BracketLine, BracketPart, BracketSection,
        BracketSource, Chart, Column, Count, EntrantRow, MatchRow, Medal, NameSearch, Outcome,
        Plugin, RoundSummary, RoundsView, Scale, Shown, StageName, StandingCell, Unplayed,
    },
};

mod bracket;
mod cross_table;
mod entrants;
mod entry;
mod phase;
mod report;
mod scale;
mod storage;
mod undo;
mod views;

/// The output of `future`, which a [`MemoryStorage`] finishes right away.
#[track_caller]
fn now<T>(future: impl Future<Output = T>) -> T {
    match pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(output) => output,
        Poll::Pending => panic!("the storage of a test never waits"),
    }
}

/// Builds the session a test starts from: Alice, Bob, Carol and Dave entered in a round robin
/// with the defaults of its format, unless the test says otherwise.
struct Given {
    entrants: Vec<&'static str>,
    format: &'static str,
    /// The format's suggestion if `None`.
    match_format: Option<PluginConfig>,
}

impl Given {
    fn new() -> Self {
        Self {
            entrants: vec!["Alice", "Bob", "Carol", "Dave"],
            format: RoundRobin::ID,
            match_format: None,
        }
    }

    fn entrants(self, entrants: &[&'static str]) -> Self {
        Self {
            entrants: entrants.to_vec(),
            ..self
        }
    }

    fn format(self, format: &'static str) -> Self {
        Self { format, ..self }
    }

    fn match_format(self, match_format: PluginConfig) -> Self {
        Self {
            match_format: Some(match_format),
            ..self
        }
    }

    /// The session with the entrants and the phase added, not started.
    fn added(self) -> Session {
        let mut session = Session::new("cup");
        for name in self.entrants {
            session.add_entrant(name).unwrap();
        }
        let mut draft = session.phase_draft(self.format).unwrap();
        if let Some(match_format) = self.match_format {
            draft = draft
                .with(PhasePlugins::MATCH_FORMAT, match_format)
                .and_then(|draft| draft.with_defaults())
                .unwrap();
        }
        session.add_phase(&draft).unwrap();
        session
    }

    fn started(self) -> Session {
        let mut session = self.added();
        session.start().unwrap();
        session
    }
}

fn names(entrants: &[EntrantRow]) -> Vec<&str> {
    entrants
        .iter()
        .map(|entrant| entrant.name.as_deref().unwrap())
        .collect()
}

/// Every match of the phase, round by round.
fn matches(session: &Session) -> Vec<MatchRow> {
    session
        .rounds()
        .iter()
        .flat_map(|summary| session.round(summary.round).unwrap().matches)
        .collect()
}

/// The match listed at `number`.
fn numbered(session: &Session, number: usize) -> MatchId {
    matches(session)
        .into_iter()
        .find(|row| row.number == number)
        .unwrap()
        .id
}

fn round(number: u32) -> NonZeroU32 {
    NonZeroU32::new(number).unwrap()
}

/// What `stage` is named in English.
fn english(stage: Option<StageName>) -> Option<String> {
    stage.map(|stage| stage.stage.to_string())
}

/// The start says why it would be refused before it is tried, as it is refused.
fn count(count: u16) -> Option<EnteredValue> {
    Some(EnteredValue::Count(count))
}

/// The scores of `a` to `b`, each pair one.
fn scores<const N: usize>(scores: [(u16, u16); N]) -> Vec<Sides<Option<EnteredValue>>> {
    scores
        .into_iter()
        .map(|(a, b)| Sides::new(count(a), count(b)))
        .collect()
}

#[test]
fn the_start_needs_a_phase_and_two_entrants() {
    let mut session = Session::new("cup");
    session.add_entrant("Alice").unwrap();
    session.add_entrant("Bob").unwrap();
    assert!(matches!(
        session.start_refusal(),
        Some(StartError::NoPhase(_))
    ));
    assert!(matches!(session.start(), Err(StartError::NoPhase(_))));

    let mut session = Given::new().entrants(&["Alice"]).added();
    assert!(matches!(
        session.start_refusal(),
        Some(StartError::TooFewEntrants)
    ));
    assert!(matches!(session.start(), Err(StartError::TooFewEntrants)));

    let mut session = Given::new().added();
    assert!(session.start_refusal().is_none());
    session.start().unwrap();

    assert!(matches!(session.start_refusal(), Some(StartError::Started)));
    assert!(matches!(session.start(), Err(StartError::Started)));
}

/// A Monster DYP pairs teams of two, so it needs four entrants; with fewer it would schedule
/// nothing and be finished at once.
#[test]
fn the_start_needs_as_many_entrants_as_the_format_does() {
    let mut session = Given::new()
        .entrants(&["Alice", "Bob", "Carol"])
        .format(MonsterDyp::ID)
        .added();

    assert!(matches!(
        session.start_refusal(),
        Some(StartError::TooFewForFormat(4))
    ));
    assert!(matches!(
        session.start(),
        Err(StartError::TooFewForFormat(4))
    ));
    session.add_entrant("Dave").unwrap();
    session.start().unwrap();
}

#[test]
fn what_can_be_done_follows_the_competition() {
    let mut session = Session::new("cup");
    let actions = session.actions();
    assert!(actions.add_entrant && actions.add_phase);
    assert!(!actions.remove_entrant && !actions.edit_phase && !actions.start);
    assert!(!actions.report && !actions.undo && !actions.redo);

    session.add_entrant("Alice").unwrap();
    assert!(session.actions().remove_entrant && session.actions().undo);
    let draft = session.phase_draft(RoundRobin::ID).unwrap();
    session.add_phase(&draft).unwrap();
    assert!(!session.actions().add_phase && session.actions().edit_phase);
    assert!(!session.actions().start, "one entrant is too few");
    session.add_entrant("Bob").unwrap();
    assert!(session.actions().start);

    session.start().unwrap();
    let actions = session.actions();
    assert!(actions.report && actions.edit_phase);
    assert!(!actions.add_entrant && !actions.remove_entrant && !actions.start);

    session
        .report(numbered(&session, 1), MatchResult::Draw)
        .unwrap();
    assert!(!session.actions().report, "the only match has its result");
    session.undo().unwrap();
    assert!(session.actions().report && session.actions().redo);
}
