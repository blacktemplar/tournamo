//! A competition being run by an operator, from creating or opening it to the last result.
//!
//! A [`Session`] owns the [`CompetitionStateMachine`] of the competition and turns what the
//! operator does into its transitions. It keeps the names of the entrants apart from the log, as
//! [`People`], and is saved to a [`Storage`] whole, as its file (see [`mod@file`]). What can be
//! undone and redone is what the session did itself, which is not saved: a loaded competition
//! starts with nothing to undo.
//!
//! [`Storage`]: crate::storage::Storage
//! [`mod@file`]: mod@crate::file

use std::collections::HashMap;

use plugin_factory::config::Config;
use tournament_engine::{
    Competition, CompetitionStateMachine, CompetitionStateMachineExt as _, MatchId, Part, Phase,
    Seed, SetCompetitionSeed, StartCompetition,
    transitions::{self as engine, Change, CompetitionAlreadyStarted, ConflictsError},
    undoable_state_machine::{SessionHistory, StateMachineError, TransitionError},
    uuid::Uuid,
};

use crate::{
    error::{NoPhase, StartError},
    people::People,
    storage::CompetitionKey,
    view::Actions,
};

/// The name of a competition imported from a file that names it by nothing but spaces, and whose
/// own name is blank as well, see [`Session::import`].
pub const UNNAMED: &str = "Competition";

/// The most matches [`Session::scale`] plays out, far more than a day's play.
pub const MOST_MATCHES: usize = 10_000;

/// A competition being run by an operator.
///
/// The first entry of the log of every competition is the [`SetCompetitionSeed`] it was created
/// with, which is no action of the session and so never undone.
pub struct Session {
    key: CompetitionKey,
    name: String,
    machine: CompetitionStateMachine,
    /// Everyone ever entered in the log, removed entrants included, since undoing their removal
    /// brings them back.
    people: People,
    /// What this session did, which `undo` and `redo` walk through.
    history: SessionHistory,
    /// The configuration of the phase whose adding this session undid last, which the form of a
    /// new phase starts out at.
    undone_phase: Option<Config>,
    /// What was reported for each match whose result this session took back, as it was then, the
    /// latest of each, which the form of the match starts out at, see [`Self::result_form`].
    reopened: HashMap<MatchId, Part>,
    /// How often the session changed the people: with the length of the log, which every other
    /// change makes longer, undo and redo included, it counts the changes, see [`Self::changes`].
    people_changes: usize,
    /// [`Self::changes`] when the session was last saved, loaded or caught up.
    saved_changes: usize,
    /// Whether the storage holds the competition the log replays into, which it does not if
    /// nothing is saved yet or it was saved by a version that did not store it.
    competition_saved: bool,
}

impl Session {
    /// Creates a new competition named `name`, with a random seed. Nothing of it is saved yet.
    ///
    /// # Panics
    /// Panics if the platform provides no randomness.
    pub fn new(name: impl Into<String>) -> Self {
        let mut seed = [0; 12];
        getrandom::fill(&mut seed).expect("the platform must provide randomness");
        let mut machine = CompetitionStateMachine::default();
        machine
            .apply_transition(SetCompetitionSeed::new(Seed::new(seed)))
            .expect("a new competition must accept a seed");
        Self::open(
            CompetitionKey::new(Uuid::new_v4()),
            name.into(),
            machine,
            People::default(),
        )
    }

    /// A session of the competition `machine` replays, stored under `key`, as nothing of it were
    /// saved yet and the session had done nothing.
    fn open(
        key: CompetitionKey,
        name: String,
        machine: CompetitionStateMachine,
        people: People,
    ) -> Self {
        Self {
            key,
            name,
            machine,
            people,
            history: SessionHistory::new(),
            undone_phase: None,
            reopened: HashMap::new(),
            people_changes: 0,
            saved_changes: 0,
            competition_saved: false,
        }
    }

    /// The number of changes made to the competition, counting the entries of its log and the
    /// changes of the people since it was created or opened: it grows with every change, so that a
    /// caller can tell whether anything changed since it last looked.
    pub fn changes(&self) -> usize {
        self.machine.log().len() + self.people_changes
    }

    /// The key the competition is stored under.
    pub fn key(&self) -> CompetitionKey {
        self.key
    }

    /// The name the operator gave the competition.
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn competition(&self) -> &Competition {
        self.machine.current()
    }

    /// Everyone ever entered, including removed entrants whose removal can be undone.
    pub fn people(&self) -> &People {
        &self.people
    }

    /// What the operator can do now.
    pub fn actions(&self) -> Actions {
        let competition = self.competition();
        let started = competition.is_started();
        let has_phase = !competition.phases().is_empty();
        Actions {
            add_entrant: !started,
            remove_entrant: !started && !competition.entrants().is_empty(),
            rename_entrant: !competition.entrants().is_empty(),
            add_phase: !started && !has_phase,
            edit_phase: has_phase,
            start: self.start_refusal().is_none(),
            report: self.started_phase().is_some_and(|phase| {
                phase
                    .data()
                    .matches()
                    .values()
                    .any(|r#match| !r#match.is_finished())
            }),
            undo: self.history.next_undo(&self.machine).is_some(),
            redo: self.history.next_redo(&self.machine).is_some(),
        }
    }

    /// Why [`Self::start`] would refuse to start the competition now, or `None` if it would start
    /// it.
    pub fn start_refusal(&self) -> Option<StartError> {
        let competition = self.competition();
        if competition.is_started() {
            Some(StartError::Started)
        } else if competition.phases().is_empty() {
            Some(NoPhase.into())
        } else if competition.entrants().len() < 2 {
            Some(StartError::TooFewEntrants)
        } else {
            self.phase()
                .map(|phase| phase.format().min_entrants())
                .filter(|needed| competition.entrants().len() < *needed)
                .map(StartError::TooFewForFormat)
        }
    }

    /// Starts the competition, which schedules the first matches.
    ///
    /// # Errors
    /// Returns an error if the competition has started already, has no phase, or fewer than two
    /// entrants, or fewer than the format of the phase needs.
    pub fn start(&mut self) -> Result<(), StartError> {
        if let Some(needed @ StartError::TooFewForFormat(_)) = self.start_refusal() {
            return Err(needed);
        }
        self.machine
            .apply_in(&mut self.history, StartCompetition)
            .map_err(|error| {
                let error = rejection(error);
                if error.is::<CompetitionAlreadyStarted>() {
                    return StartError::Started;
                }
                match error.downcast_ref() {
                    Some(engine::StartCompetitionError::NoPhases) => NoPhase.into(),
                    Some(engine::StartCompetitionError::TooFewEntrants) => {
                        StartError::TooFewEntrants
                    }
                    None => StartError::Rejected(error),
                }
            })?;
        Ok(())
    }

    /// The numbers of the matches whose results would no longer apply if `change` were made,
    /// ascending, see
    /// [`conflicts_of`](tournament_engine::CompetitionStateMachineExt::conflicts_of).
    fn conflicting_numbers(&self, change: Change<'_>) -> Result<Vec<usize>, ConflictsError> {
        let mut numbers: Vec<_> = self
            .machine
            .conflicts_of(change)?
            .iter()
            .filter_map(|conflict| {
                let phase = self.competition().phase(&conflict.phase_id)?;
                phase
                    .data()
                    .matches()
                    .get(&conflict.match_id)
                    .map(views::number)
            })
            .collect();
        numbers.sort_unstable();
        Ok(numbers)
    }

    /// The phase of the competition, whether started or not.
    fn phase(&self) -> Option<&Phase> {
        self.competition().only_phase()
    }

    fn started_phase(&self) -> Option<&Phase> {
        self.phase().filter(|_| self.competition().is_started())
    }
}

/// The error of the transition an action of the session was rejected with.
fn rejection(error: StateMachineError) -> TransitionError {
    match error {
        StateMachineError::Transition(_, error) => error,
        // the session only appends new entries, each with a new id and targeting one in effect
        error => unreachable!("an action of the session is rejected by a transition: {error}"),
    }
}

mod bracket;
mod cross_table;
mod entrants;
mod entry;
mod phase;
mod report;
mod scale;
mod storage;
#[cfg(test)]
mod tests;
mod undo;
mod views;
