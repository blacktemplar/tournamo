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
    draft::Draft,
};
use tournament_engine::{
    EnteredValue, EntrantId, MatchId, MatchResult, PhasePlugins, Score, Sides,
    plugins::{
        deprecated::DoubleEliminationV1,
        match_format::{BestOf, Scored},
        metrics_calculator::{
            Cell as MetricCell, MoreBuchholz, MorePointsPerMatch, MoreWins,
            RoundOfFirstLossCalculator,
        },
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
        PhaseDraftError, RateEntrantError, RedoError, RemoveEntrantError, RenameEntrantError,
        ReportEnteredError, ReportError, SourcesError, StartError, UndoError, UndoReportError,
    },
    file::{self},
    form::PhaseForm,
    people::Rating,
    storage::{CompetitionKey, Listing, MemoryStorage, Progress, Save, Storage, Stored},
    view::{
        Action, BracketColumn, BracketEntry, BracketLine, BracketPart, BracketSection,
        BracketSource, Chart, Column, Count, DivisionRef, EntrantRow, MatchRow, MatchesOf, Medal,
        NameSearch, Outcome, Plugin, RoundSummary, RoundsView, Scale, Shown, StageName,
        StandingCell, Unplayed,
    },
};

mod bracket;
mod cross_table;
mod divisions;
mod entrants;
mod entry;
mod phase;
mod phases;
mod presets;
mod report;
mod scale;
mod storage;
mod teams;
mod undo;
mod views;

impl Session {
    /// The first phase, the only one of most tests.
    pub(crate) fn first(&self) -> PhaseId {
        *self
            .competition()
            .phases()
            .keys()
            .next()
            .expect("the competition has a phase")
    }

    /// The matches of the first phase.
    pub(crate) fn all(&self) -> MatchesOf {
        MatchesOf::phase(self.first())
    }

    /// The only division of the first phase.
    pub(crate) fn only(&self) -> DivisionRef {
        DivisionRef::first(self.first())
    }
}

/// A form of the phase `draft` configures, the first of its competition.
pub(crate) fn form(draft: Draft<PhasePlugins>) -> PhaseForm {
    PhaseForm::new(draft)
}

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
pub(crate) struct Given {
    entrants: Vec<&'static str>,
    format: &'static str,
    /// The format's suggestion if `None`.
    match_format: Option<PluginConfig>,
    divisions: u32,
    /// What the divisions are compared across by.
    across: Vec<PluginConfig>,
    /// Who plays together; as registered if `None`.
    teams: Option<PluginConfig>,
    /// The criteria of the ranking; the format's suggestion if `None`.
    ranking: Option<Vec<PluginConfig>>,
    /// The metrics configured, those it needs implied; the format's suggestion if `None`.
    metrics: Option<Vec<PluginConfig>>,
}

impl Given {
    pub(crate) fn new() -> Self {
        Self {
            entrants: vec!["Alice", "Bob", "Carol", "Dave"],
            format: RoundRobin::ID,
            match_format: None,
            divisions: 1,
            across: Vec::new(),
            teams: None,
            ranking: None,
            metrics: None,
        }
    }

    /// Ranked by `ranking`.
    pub(crate) fn ranking(self, ranking: Vec<PluginConfig>) -> Self {
        Self {
            ranking: Some(ranking),
            ..self
        }
    }

    /// The metrics `metrics` configured, and those they or the ranking need implied.
    pub(crate) fn metrics(self, metrics: Vec<PluginConfig>) -> Self {
        Self {
            metrics: Some(metrics),
            ..self
        }
    }

    /// The teams formed by `T`.
    fn teams<T: PluginType>(self) -> Self {
        Self {
            teams: Some(PluginConfig::of::<T>()),
            ..self
        }
    }

    /// The phase played in `divisions` divisions, dealt in order.
    fn divisions(self, divisions: u32) -> Self {
        Self { divisions, ..self }
    }

    /// The divisions compared across by the criterion `C`, after those before.
    fn across<C: PluginType>(mut self) -> Self {
        self.across.push(PluginConfig::of::<C>());
        self
    }

    pub(crate) fn entrants(self, entrants: &[&'static str]) -> Self {
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
        if let Some(teams) = self.teams {
            draft = draft.with(PhasePlugins::TEAMS, teams).unwrap();
        }
        if let Some(ranking) = self.ranking {
            draft = draft.with(PhasePlugins::RANKING, ranking).unwrap();
        }
        if let Some(metrics) = self.metrics {
            draft = draft.with(PhasePlugins::METRICS, metrics).unwrap();
        }
        let draft = draft
            .with(PhasePlugins::DIVISIONS, self.divisions)
            .and_then(|draft| draft.with(PhasePlugins::ACROSS, self.across))
            .unwrap();
        session.add_phase(&form(draft)).unwrap();
        session
    }

    pub(crate) fn started(self) -> Session {
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
        .rounds(session.all())
        .iter()
        .flat_map(|summary| session.round(session.all(), summary.round).unwrap().matches)
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
    session.add_phase(&form(draft)).unwrap();
    // a competition is played in as many phases as are added
    assert!(session.actions().add_phase && session.actions().edit_phase);
    assert!(!session.actions().start, "one entrant is too few");
    session.add_entrant("Bob").unwrap();
    assert!(session.actions().start);

    session.start().unwrap();
    let actions = session.actions();
    assert!(actions.report && actions.edit_phase);
    assert!(!actions.add_entrant && !actions.remove_entrant && !actions.start);

    session
        .report(session.first(), numbered(&session, 1), MatchResult::Draw)
        .unwrap();
    assert!(!session.actions().report, "the only match has its result");
    session.undo().unwrap();
    assert!(session.actions().report && session.actions().redo);
}
