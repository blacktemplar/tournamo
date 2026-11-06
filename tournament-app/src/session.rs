//! A competition being run by an operator, from creating or opening it to the last result.
//!
//! A [`Session`] owns the [`CompetitionStateMachine`] of the competition and turns what the
//! operator does into its transitions. It keeps the names of the entrants apart from the log, as
//! [`People`], and is saved to a [`Storage`] whole, as its file (see [`mod@file`]). What can be
//! undone and redone is what the session did itself, which is not saved: a loaded competition
//! starts with nothing to undo.

use std::{
    any::Any,
    collections::{BTreeMap, HashMap, HashSet},
    num::NonZeroU32,
};

use plugin_factory::{
    catalog::{AnyAxis, PluginEntry},
    config::{Config, PluginConfig},
    draft::Draft,
};
use tournament_engine::{
    AddEntrant, AddPhase, Annotated as _, Bye, Competition, CompetitionStateMachine,
    CompetitionStateMachineExt as _, CompetitionTransition, Decision, Entrant, EntrantId, Match,
    MatchId, MatchResult, Part, Phase, PhaseData, PhaseId, PhasePlugins, Ranking, RankingId,
    RemoveEntrant, ReportMatchResult, Seed, SetCompetitionSeed, StartCompetition, TypeMap, Values,
    plugins::{
        metrics_calculator::{Cell, MetricsCalculator},
        phase_format::PhaseFormat,
        ranker::SliceRanker,
    },
    transitions::{
        self as engine, CompetitionAlreadyStarted, ReconfigurePhaseError, UndoMatchResultError,
    },
    undoable_state_machine::{
        LogEntry, LogEntryKind, RedoLastError, SessionHistory, StateMachineError, TransitionError,
        UndoLastError,
    },
    uuid::Uuid,
};
use unicode_normalization::UnicodeNormalization as _;

use crate::{
    error::{
        AddEntrantError, AddPhaseError, EditPhaseError, ImportError, LoadError, NoPhase,
        NothingToRedo, NothingToUndo, PhaseDraftError, RemoveEntrantError, RenameEntrantError,
        ReportEnteredError, ReportError, StartError, UndoReportError,
    },
    file::{self, File},
    form::PhaseForm,
    people::{People, Person},
    result_form::ResultForm,
    storage::{CompetitionKey, Listing, Progress, Save, Storage, Stored},
    view::{
        Action, Actions, ByeRow, Column, Count, Entered, EntrantRow, Glossary, MatchRow, Medal,
        NameSearch, Plugin, Reopened, Reported, Round, RoundSummary, Scale, StageName,
        StandingCell, StandingRow, Standings,
    },
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
    /// The actions of `history` that can be undone, the latest last, and those that can be
    /// redone, the one undone last at the end, which it does not tell.
    done: Vec<Action>,
    undone: Vec<Action>,
    /// The configuration of the phase whose adding this session undid last, which the form of a
    /// new phase starts out at.
    undone_phase: Option<Config>,
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
        Self {
            key: CompetitionKey::new(Uuid::new_v4()),
            name: name.into(),
            machine,
            people: People::default(),
            history: SessionHistory::new(),
            done: Vec::new(),
            undone: Vec::new(),
            undone_phase: None,
            people_changes: 0,
            saved_changes: 0,
            competition_saved: false,
        }
    }

    /// Opens the competition stored in `storage` under `key`. The session has done nothing yet, so
    /// there is nothing to undo. If a version that did not store the competition the log replays
    /// into saved it last, that is unsaved.
    ///
    /// # Errors
    /// Returns an error if nothing is stored under `key`, the storage cannot be read, or what is
    /// stored is no log that replays into a competition.
    pub async fn load<S: Storage>(
        storage: &S,
        key: CompetitionKey,
    ) -> Result<Self, LoadError<S::Error>> {
        let stored = read(storage, key).await?;
        let file = Record::read(&stored.record)?;
        let machine = CompetitionStateMachine::build(file.log).map_err(LoadError::Replay)?;
        Ok(Self {
            key,
            name: stored.name,
            saved_changes: machine.log().len(),
            machine,
            people: file.people,
            history: SessionHistory::new(),
            done: Vec::new(),
            undone: Vec::new(),
            undone_phase: None,
            people_changes: 0,
            competition_saved: file.competition.is_some(),
        })
    }

    /// Catches up with what was saved of the competition in `storage` since this session loaded or
    /// caught up last, e.g. by a session in another tab, see [`Self::catch_up_with`]. Returns how
    /// many entries it appended.
    ///
    /// # Errors
    /// Returns an error if nothing is stored under the session's key, the storage cannot be read,
    /// or what is stored cannot be read or an entry does not apply, see [`Self::catch_up_with`].
    ///
    /// # Panics
    /// Panics if the session has unsaved changes, see [`Self::catch_up_with`].
    pub async fn catch_up<S: Storage>(
        &mut self,
        storage: &S,
    ) -> Result<usize, LoadError<S::Error>> {
        let stored = read(storage, self.key).await?;
        self.catch_up_with(&stored)
    }

    /// Catches up with `stored`, what a storage holds of this session's competition: appends the
    /// entries of its log after those of the session's, each as [`Self::load`] would have
    /// replayed it, and takes the people as stored. Returns how many entries it appended. They are
    /// no actions of this session, so there is nothing more to undo. [`Self::catch_up`] reads and
    /// catches up in one; reading apart lets a GUI hold no borrow of the session while it reads.
    ///
    /// An entry that undoes or replaces an earlier one replays the log after that one, as it did
    /// when it was appended; any other entry only applies to the current state.
    ///
    /// # Errors
    /// Returns an error if what is stored cannot be read or an entry does not apply. If it cannot
    /// be read, nothing is appended; if an entry does not apply, the entries before it stay
    /// appended.
    ///
    /// # Panics
    /// Panics if the session has unsaved entries or people: what was stored after them is of
    /// another session that took over the competition, and the log would fork. An unsaved
    /// competition is no change of the session's own, e.g. of one opened read-only that an older
    /// version saved.
    pub fn catch_up_with<E>(&mut self, stored: &Stored) -> Result<usize, LoadError<E>> {
        assert!(
            self.changes() == self.saved_changes,
            "a session catches up only with nothing unsaved"
        );
        let file = Record::read(&stored.record)?;
        self.people = file.people;
        self.competition_saved = file.competition.is_some();
        let entries = file.log.into_iter().skip(self.machine.log().len());
        let appended = entries.len();
        for entry in entries {
            self.machine.apply(entry).map_err(LoadError::Replay)?;
            self.saved_changes = self.changes();
        }
        Ok(appended)
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

    /// Opens the already decoded file as a competition named `name`, with no undo history.
    /// Nothing is saved yet; the caller chooses its storage and when to save it.
    pub fn from_file(file: File<CompetitionStateMachine, People>, name: impl Into<String>) -> Self {
        let File {
            log: machine,
            people,
            ..
        } = file;
        Self {
            key: CompetitionKey::new(Uuid::new_v4()),
            name: name.into(),
            machine,
            people,
            history: SessionHistory::new(),
            done: Vec::new(),
            undone: Vec::new(),
            undone_phase: None,
            people_changes: 0,
            saved_changes: 0,
            competition_saved: false,
        }
    }

    /// The competition's file data, borrowing its log, people and competition. Its optional name
    /// can be changed or omitted before serialization, without changing the session.
    pub fn file(&self) -> File<&CompetitionStateMachine, &People, &Competition> {
        File {
            format: file::Format::CURRENT,
            name: Some(self.name.clone()),
            log: &self.machine,
            people: &self.people,
            competition: Some(self.machine.current()),
        }
    }

    /// Whether the storage lacks anything: a change since the last save, or the competition the log
    /// replays into, see [`Self::load`].
    pub fn has_unsaved_changes(&self) -> bool {
        self.changes() != self.saved_changes || !self.competition_saved
    }

    /// The competition as it is to be saved, to be written to a storage and then handed to
    /// [`Self::mark_saved`]; [`Self::save`] does both. Saving a session that has not been saved yet
    /// creates the stored competition.
    ///
    /// # Panics
    /// Panics if the log, a person or the competition does not serialize as JSON.
    pub fn to_save(&self) -> Save {
        Save {
            listing: Listing {
                key: self.key,
                name: self.name.clone(),
                progress: Some(self.progress()),
            },
            record: file::to_record(self.file()),
            changes: self.changes(),
        }
    }

    /// Notes that `save`, which [`Self::to_save`] of this session returned, has been written to its
    /// storage. What changed after it was taken stays unsaved, so that saves whose writes finish
    /// out of order leave nothing unsaved once every write has.
    ///
    /// # Panics
    /// Panics if `save` is of another competition.
    pub fn mark_saved(&mut self, save: &Save) {
        assert_eq!(
            save.listing.key, self.key,
            "the save must be of this session's competition"
        );
        self.saved_changes = self.saved_changes.max(save.changes);
        self.competition_saved = true;
    }

    /// Writes the competition to `storage`.
    ///
    /// # Errors
    /// Returns the error of the storage, in which case the changes stay unsaved and the next save
    /// writes them.
    pub async fn save<S: Storage>(&mut self, storage: &S) -> Result<(), S::Error> {
        let save = self.to_save();
        storage.write(&save).await?;
        self.mark_saved(&save);
        Ok(())
    }

    /// The competition as a file, with its name, log and people, see [`crate::file`].
    pub fn export(&self) -> String {
        file::to_json(&self.file())
    }

    /// A new competition with what the file `file` holds, e.g. one [`Self::export`] or the CLI
    /// wrote, named as in the file or else `name`, or else [`UNNAMED`] where both are blank, and
    /// without the spaces around it, followed by the first number from 2 on in
    /// parentheses that makes it a name none of `stored` has, if one has it, e.g. `"Cup (2)"`. It
    /// is stored under a key of its own, so that importing a file twice makes two competitions;
    /// nothing of it is saved yet.
    ///
    /// # Errors
    /// Returns an error if `file` is no competition's file, one of a newer version, or one whose
    /// log does not replay, see [`ImportError`].
    pub fn import(file: &str, name: &str, stored: &[Listing]) -> Result<Self, ImportError> {
        let file = File::<CompetitionStateMachine, People>::read(file)?;
        // a blank name, which the list could not show, is no name; nor is a blank file name
        let name = [file.name.as_deref(), Some(name)]
            .into_iter()
            .flatten()
            .map(str::trim)
            .find(|name| !name.is_empty())
            .unwrap_or(UNNAMED)
            .to_owned();
        let taken = |name: &str| stored.iter().any(|listing| listing.name == name);
        let name = if taken(&name) {
            (2..)
                .map(|number| format!("{name} ({number})"))
                .find(|copy| !taken(copy))
                .expect("a name of the numbers is not taken")
        } else {
            name
        };
        Ok(Self::from_file(file, name))
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
            undo: !self.done.is_empty(),
            redo: !self.undone.is_empty(),
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

    /// The entrants, in the order they were added.
    pub fn entrants(&self) -> Vec<EntrantRow> {
        self.competition()
            .entrants()
            .keys()
            .map(|id| self.entrant_row(id))
            .collect()
    }

    /// The name of an entrant that `name` would look like, e.g. `Alice` for `alice` or for an
    /// `Alice` whose characters are composed otherwise: equal ignoring case and the way Unicode
    /// composes them (NFKC), and leading and trailing whitespace. [`Self::add_entrant`] refuses
    /// such a name, which a screen can say while it is typed.
    pub fn entrant_like(&self, name: &str) -> Option<String> {
        self.entrant_like_besides(name, None)
    }

    /// As [`Self::entrant_like`], but never `besides`, e.g. the entrant being renamed.
    fn entrant_like_besides(&self, name: &str, besides: Option<EntrantId>) -> Option<String> {
        let name = folded(name);
        if name.is_empty() {
            return None;
        }
        self.competition()
            .entrants()
            .keys()
            .filter(|id| Some(**id) != besides)
            .filter_map(|id| self.people.name(id))
            .find(|entered| folded(entered) == name)
            .map(str::to_owned)
    }

    /// Adds an entrant shown by `name`, with leading and trailing whitespace removed.
    ///
    /// # Errors
    /// Returns an error if `name` is empty or looks like another entrant's, see
    /// [`Self::entrant_like`], or if the competition has started.
    pub fn add_entrant(&mut self, name: &str) -> Result<EntrantId, AddEntrantError> {
        let name = name.trim();
        self.check_names(std::iter::once(name))?;
        self.add_named_entrant(name)
    }

    /// Adds all of `names` or none if any name is empty, duplicated, or entrants cannot be added.
    /// Names are kept as supplied; whitespace-only names are empty. Each entrant is a separate
    /// undoable action, as when calling [`Self::add_entrant`] for each one.
    ///
    /// # Errors
    /// Returns an error if a name is empty or looks like an existing or earlier name,
    /// see [`Self::entrant_like`], or the competition has started.
    pub fn add_named_entrants(
        &mut self,
        names: &[String],
    ) -> Result<Vec<EntrantId>, AddEntrantError> {
        self.check_names(names.iter().map(String::as_str))?;
        names
            .iter()
            .map(|name| self.add_named_entrant(name))
            .collect()
    }

    fn check_names<'a>(
        &self,
        names: impl IntoIterator<Item = &'a str>,
    ) -> Result<(), AddEntrantError> {
        let mut taken: HashMap<_, _> = self
            .competition()
            .entrants()
            .keys()
            .filter_map(|id| self.people.name(id))
            .map(|name| (folded(name), name))
            .collect();
        for name in names {
            if name.trim().is_empty() {
                return Err(AddEntrantError::EmptyName);
            }
            if let Some(entered) = taken.insert(folded(name), name) {
                return Err(AddEntrantError::DuplicateName(entered.to_owned()));
            }
        }
        Ok(())
    }

    // All names in a batch are checked first. The engine accepts adding its first entrant
    // exactly when it accepts adding every subsequent one, so a refusal adds none of them.
    fn add_named_entrant(&mut self, name: &str) -> Result<EntrantId, AddEntrantError> {
        let id = EntrantId::new(Uuid::new_v4());
        let action = Action::AddEntrant(EntrantRow {
            id,
            name: Some(name.to_owned()),
        });
        self.act(action, |machine, history| {
            machine.apply_in(history, AddEntrant::new(Entrant::new(id)))
        })
        .map_err(|error| {
            let error = rejection(error);
            if error.is::<CompetitionAlreadyStarted>() {
                AddEntrantError::Started
            } else {
                AddEntrantError::Rejected(error)
            }
        })?;
        self.people.add(Person::new(id, name));
        self.people_changes += 1;
        Ok(id)
    }

    /// Adds an entrant for each line of `lines` that is not blank, e.g. of a list pasted, as
    /// [`Self::add_entrant`] adds one: all of them, in order, or none if any is refused. Each is
    /// an action of its own.
    ///
    /// # Errors
    /// Returns an error if every line is blank, a name looks like another entrant's or that of an
    /// earlier line, see [`Self::entrant_like`], or the competition has started.
    pub fn add_entrants(&mut self, lines: &str) -> Result<Vec<EntrantId>, AddEntrantError> {
        let names: Vec<&str> = lines
            .lines()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .collect();
        if names.is_empty() {
            return Err(AddEntrantError::EmptyName);
        }
        if self.competition().is_started() {
            return Err(AddEntrantError::Started);
        }
        let mut earlier = HashMap::new();
        for name in &names {
            if let Some(entered) = self.entrant_like(name) {
                return Err(AddEntrantError::DuplicateName(entered));
            }
            if let Some(earlier) = earlier.insert(folded(name), *name) {
                return Err(AddEntrantError::DuplicateName(earlier.to_owned()));
            }
        }
        // what `add_entrant` refuses is checked above, so none of them is refused after another
        // was added
        names
            .into_iter()
            .map(|name| self.add_entrant(name))
            .collect()
    }

    /// Removes the entrant `id`. Its name is kept, in case the removal is undone.
    ///
    /// # Errors
    /// Returns an error if there is no such entrant, or the competition has started.
    pub fn remove_entrant(&mut self, id: EntrantId) -> Result<(), RemoveEntrantError> {
        let action = Action::RemoveEntrant(self.entrant_row(&id));
        self.act(action, |machine, history| {
            machine.apply_in(history, RemoveEntrant::new(id))
        })
        .map_err(|error| {
            let error = rejection(error);
            if error.is::<CompetitionAlreadyStarted>() {
                RemoveEntrantError::Started
            } else if error.is::<engine::RemoveEntrantError>() {
                RemoveEntrantError::UnknownEntrant
            } else {
                RemoveEntrantError::Rejected(error)
            }
        })?;
        Ok(())
    }

    /// Shows the entrant `id` by `name` from now on, with leading and trailing whitespace removed,
    /// also after the start. Only its name changes, which is no entry of the log, so this is no
    /// action that [`Self::undo`] undoes; the actions done before name the entrant as it was then.
    ///
    /// # Errors
    /// Returns an error if there is no such entrant, or if `name` is empty or looks like another
    /// entrant's, see [`Self::entrant_like`].
    pub fn rename_entrant(&mut self, id: EntrantId, name: &str) -> Result<(), RenameEntrantError> {
        if !self.competition().entrants().contains_key(&id) {
            return Err(RenameEntrantError::UnknownEntrant);
        }
        let name = name.trim();
        if name.is_empty() {
            return Err(RenameEntrantError::EmptyName);
        }
        if let Some(entered) = self.entrant_like_besides(name, Some(id)) {
            return Err(RenameEntrantError::DuplicateName(entered));
        }
        if self.people.name(&id) != Some(name) {
            self.people.add(Person::new(id, name));
            self.people_changes += 1;
        }
        Ok(())
    }

    /// Every format a phase can be played in.
    pub fn formats() -> Vec<Plugin> {
        plugins(AnyAxis::of::<dyn PhaseFormat>())
    }

    /// Every format, criterion of the standings and metric a phase can be played with.
    pub fn glossary() -> Glossary {
        Glossary {
            formats: Self::formats(),
            criteria: plugins(AnyAxis::of::<dyn SliceRanker>()),
            metrics: plugins(AnyAxis::of::<dyn MetricsCalculator>()),
        }
    }

    /// How many rounds and matches the phase `draft` configures takes with the entrants entered
    /// now, found by playing it out apart from this competition, once with every match won by
    /// side A and once by side B, which sets how far the numbers depend on the results; `None` if
    /// it does not start with them or takes more than [`MOST_MATCHES`].
    pub fn scale(&self, draft: &Draft<PhasePlugins>) -> Option<Scale> {
        let plugins = completed(draft).ok()?;
        let [a, b] = [MatchResult::AWins, MatchResult::BWins]
            .map(|result| self.play_out(plugins.clone(), result));
        let ((a_rounds, a_matches), (b_rounds, b_matches)) = (a?, b?);
        let count = |a: usize, b: usize| Count {
            least: a.min(b),
            most: a.max(b),
        };
        Some(Scale {
            rounds: count(a_rounds, b_rounds),
            matches: count(a_matches, b_matches),
        })
    }

    /// The rounds and matches of a phase of `plugins` with as many entrants as this competition,
    /// every match ending in `result`, see [`Self::scale`].
    fn play_out(&self, plugins: PhasePlugins, result: MatchResult) -> Option<(usize, usize)> {
        let mut machine = CompetitionStateMachine::default();
        // any seed, since what a format draws does not change how long it takes
        let seed = SetCompetitionSeed::new(Seed::new([0; 12]));
        machine.apply_transition(seed).ok()?;
        for index in 0..self.competition().entrants().len() {
            let id = EntrantId::new(Uuid::from_u128(index as u128 + 1));
            machine
                .apply_transition(AddEntrant::new(Entrant::new(id)))
                .ok()?;
        }
        let phase = PhaseId::new(Uuid::nil());
        machine
            .apply_transition(AddPhase::new(phase, plugins))
            .ok()?;
        machine.apply_transition(StartCompetition).ok()?;
        for _ in 0..=MOST_MATCHES {
            let data = machine.current().phases().get(&phase)?.data();
            let open = data
                .matches()
                .values()
                .find(|r#match| r#match.result().is_none());
            let Some(open) = open else {
                let rounds = data.matches().values().map(Match::round).max();
                let rounds = rounds.map_or(0, |round| round.get() as usize);
                return Some((rounds, data.matches().len()));
            };
            let report = ReportMatchResult::new(phase, open.id(), result);
            machine.apply_transition(report).ok()?;
        }
        None
    }

    /// A draft of a phase of this competition played in the format `format`, a [`Plugin::id`],
    /// with schema defaults and format suggestions, see [`PhasePlugins::with_suggestions`].
    /// It can be changed before it is handed to [`Self::add_phase`].
    ///
    /// # Errors
    /// Returns an error if there is no such format, or it offers an answer it does not take.
    pub fn phase_draft(&self, format: &str) -> Result<Draft<PhasePlugins>, PhaseDraftError> {
        let entry = AnyAxis::of::<dyn PhaseFormat>()
            .entries()
            .find(|entry| entry.id() == format)
            .ok_or_else(|| PhaseDraftError::UnknownFormat(format.to_owned()))?;
        Ok(PhasePlugins::with_suggestions(
            self.competition()
                .phase_draft()
                .with(PhasePlugins::FORMAT, PluginConfig::new(entry.id()))?,
        )?)
    }

    /// A form of a new phase of this competition, answered as the phase was played whose adding
    /// this session undid last, so that adding it again adds the same phase; if it undid none,
    /// with every field answered as offered: in the format a phase offers, with what that
    /// recommends, following suggestions until customized. An undone phase's settings are kept.
    /// Once filled, its [draft](PhaseForm::draft) is handed to [`Self::add_phase`].
    ///
    /// # Errors
    /// Returns an error if a plugin offers an answer it does not take.
    pub fn phase_form(&self) -> Result<PhaseForm, PhaseDraftError> {
        let draft = self.competition().phase_draft();
        match &self.undone_phase {
            Some(config) => Ok(PhaseForm::new(draft.with_answers(config)?)),
            None => {
                let mut form = PhaseForm::new(draft);
                form.apply_suggestions()?;
                Ok(form)
            }
        }
    }

    /// A form of the phase as it is played, see [`Self::phase_draft_to_edit`]. Once changed, its
    /// [draft](PhaseForm::draft) is handed to [`Self::edit_phase`].
    ///
    /// # Errors
    /// Returns [`NoPhase`] if the competition has no phase.
    pub fn phase_form_to_edit(&self) -> Result<PhaseForm, NoPhase> {
        self.phase_draft_to_edit().map(PhaseForm::new)
    }

    /// Adds the phase `draft` configures, which is the only phase the competition is played in.
    /// `draft` is a draft of this competition's phase, e.g. from [`Self::phase_draft`].
    ///
    /// A calculator that has nothing to be configured is added for a metric the phase needs and
    /// nothing calculates, see [`Draft::add_implied`]; `draft` itself is left as it is.
    ///
    /// # Errors
    /// Returns an error if the plugins `draft` configures do not fit together, the competition has
    /// a phase already, or it has started.
    pub fn add_phase(&mut self, draft: &Draft<PhasePlugins>) -> Result<(), AddPhaseError> {
        let plugins = completed(draft).map_err(AddPhaseError::Invalid)?;
        self.act(Action::AddPhase, |machine, history| {
            machine.apply_in(
                history,
                AddPhase::new(PhaseId::new(Uuid::new_v4()), plugins),
            )
        })
        .map_err(|error| {
            let error = rejection(error);
            if error.is::<CompetitionAlreadyStarted>() {
                AddPhaseError::Started
            } else if error.is::<engine::AddPhaseError>() {
                AddPhaseError::PhaseAlreadyAdded
            } else {
                AddPhaseError::Rejected(error)
            }
        })?;
        Ok(())
    }

    /// A draft holding the configuration the phase is played with, see [`Phase::config`], to be
    /// changed and handed to [`Self::edit_phase`].
    ///
    /// # Errors
    /// Returns [`NoPhase`] if the competition has no phase.
    ///
    /// # Panics
    /// Panics if the draft does not take the configuration of the phase, which it was built from.
    pub fn phase_draft_to_edit(&self) -> Result<Draft<PhasePlugins>, NoPhase> {
        let config = self.phase().ok_or(NoPhase)?.config();
        Ok(self
            .competition()
            .phase_draft()
            .with_answers(&config)
            .expect("a phase's draft takes the configuration the phase was built from"))
    }

    /// Plays the phase as `draft` configures it, as if it had been added that way, and returns
    /// whether that changed anything. `draft` is completed as by [`Self::add_phase`].
    ///
    /// # Errors
    /// Returns an error if the competition has no phase, the plugins `draft` configures do not fit
    /// together, or a change made after the phase was added, e.g. a reported result
    /// ([`EditPhaseError::ResultDoesNotFit`]), does not fit the edited phase.
    pub fn edit_phase(&mut self, draft: &Draft<PhasePlugins>) -> Result<bool, EditPhaseError> {
        let phase_id = self.phase().ok_or(NoPhase)?.id();
        let plugins = completed(draft).map_err(EditPhaseError::Invalid)?;
        let edited = self
            .act(Action::EditPhase, |machine, history| {
                machine.reconfigure_phase(history, phase_id, plugins)
            })
            .map_err(|error| match error {
                ReconfigurePhaseError::Rejected(error) => match self.reported_number(&error) {
                    Some(number) => EditPhaseError::ResultDoesNotFit(number),
                    None => EditPhaseError::DoesNotFit(rejection(error)),
                },
                ReconfigurePhaseError::NotAdded => {
                    unreachable!("the phase of a session is added by an `AddPhase` in effect")
                }
            })?;
        Ok(edited.is_some())
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
        self.act(Action::Start, |machine, history| {
            machine.apply_in(history, StartCompetition)
        })
        .map_err(|error| {
            let error = rejection(error);
            if error.is::<CompetitionAlreadyStarted>() {
                return StartError::Started;
            }
            match error.downcast_ref() {
                Some(engine::StartCompetitionError::NoPhases) => NoPhase.into(),
                Some(engine::StartCompetitionError::TooFewEntrants) => StartError::TooFewEntrants,
                None => StartError::Rejected(error),
            }
        })?;
        Ok(())
    }

    /// How far the competition is, as its [`Listing`] keeps it.
    pub fn progress(&self) -> Progress {
        if !self.competition().is_started() {
            return Progress::Setup {
                entrants: self.competition().entrants().len(),
            };
        }
        if let Some(summary) = self.rounds().into_iter().find(|summary| summary.open > 0) {
            return Progress::Playing {
                round: summary.round,
                open: summary.open,
            };
        }
        let winners = self.winners().unwrap_or_default();
        Progress::Won {
            winners: winners
                .into_iter()
                .map(|team| team.into_iter().map(|entrant| entrant.name).collect())
                .collect(),
        }
    }

    /// The teams ranked first once every match scheduled has a result, more than one if they are
    /// tied, by the names of their entrants, each team by its entrants; `None` before the start or
    /// while a match has no result.
    pub fn winners(&self) -> Option<Vec<Vec<EntrantRow>>> {
        let data = self.started_phase()?.data();
        if data
            .matches()
            .values()
            .any(|r#match| r#match.result().is_none())
        {
            return None;
        }
        let mut winners: Vec<_> = data
            .standings()
            .filter(|ranking| ranking.rank() == NonZeroU32::MIN)
            .map(|ranking| self.team(data, &[ranking.id()]))
            .collect();
        // tied teams are in no order of their own
        winners.sort_by_cached_key(|team| {
            team.iter()
                .map(|entrant| entrant.name.as_deref().map(str::to_lowercase))
                .collect::<Vec<_>>()
        });
        Some(winners)
    }

    /// The rounds the phase has scheduled matches or byes in, in order; none before the start.
    pub fn rounds(&self) -> Vec<RoundSummary> {
        let Some(phase) = self.started_phase() else {
            return Vec::new();
        };
        let data = phase.data();
        let empty = |round| RoundSummary {
            round,
            matches: 0,
            open: 0,
            stage: None,
        };
        let mut rounds = BTreeMap::new();
        // the stage of each round, until a match or a bye of another one makes it `None`
        let mut stages: BTreeMap<NonZeroU32, Option<StageName>> = BTreeMap::new();
        let mut note_stage = |round, annotations| {
            let stage = stage(phase, annotations);
            stages
                .entry(round)
                .and_modify(|kept| {
                    if *kept != stage {
                        *kept = None;
                    }
                })
                .or_insert(stage);
        };
        for r#match in data.matches().values() {
            let round = r#match.round();
            let summary = rounds.entry(round).or_insert_with(|| empty(round));
            summary.matches += 1;
            if !r#match.is_finished() {
                summary.open += 1;
            }
            note_stage(round, r#match.annotations());
        }
        for bye in data.byes().values() {
            let round = bye.round();
            rounds.entry(round).or_insert_with(|| empty(round));
            note_stage(round, bye.annotations());
        }
        rounds
            .into_values()
            .map(|summary| RoundSummary {
                stage: stages.remove(&summary.round).flatten(),
                ..summary
            })
            .collect()
    }

    /// The matches and byes of `round`, or `None` if it has neither, e.g. before the start.
    pub fn round(&self, round: NonZeroU32) -> Option<Round> {
        let phase = self.started_phase()?;
        let data = phase.data();
        let mut matches: Vec<_> = data
            .matches()
            .values()
            .filter(|r#match| r#match.round() == round)
            .map(|r#match| self.match_row(phase, r#match))
            .collect();
        // a bracket numbers its matches before it schedules them, in another order
        matches.sort_by_key(|row| row.number);
        let byes_of_round: Vec<_> = data
            .byes()
            .values()
            .filter(|bye| bye.round() == round)
            .map(|bye| (bye, stage(phase, bye.annotations())))
            .collect();
        let stages: HashSet<_> = matches
            .iter()
            .map(|row| &row.stage)
            .chain(byes_of_round.iter().map(|(_, stage)| stage))
            .collect();
        let several_stages = stages.len() > 1;
        let byes: Vec<_> = byes_of_round
            .into_iter()
            .map(|(bye, stage)| ByeRow {
                entrants: self.team(data, &[bye.ranking()]),
                stage: stage.filter(|_| several_stages),
            })
            .collect();
        (!matches.is_empty() || !byes.is_empty()).then_some(Round {
            round,
            matches,
            byes,
            draws: phase.format().allows_draws(),
        })
    }

    /// The matches that can be played now, in the order they were scheduled: those without a
    /// result none of whose entrants still has to play a match of an earlier round. None before
    /// the start.
    pub fn playable(&self) -> Vec<MatchRow> {
        let Some(phase) = self.started_phase() else {
            return Vec::new();
        };
        let data = phase.data();
        let open = || {
            data.matches()
                .values()
                .filter(|r#match| !r#match.is_finished())
        };
        let entrants = |r#match: &Match| -> Vec<EntrantId> {
            r#match
                .team_a()
                .iter()
                .chain(r#match.team_b())
                .flat_map(|ranking| data.rankings()[ranking].entrants())
                .copied()
                .collect()
        };
        // the earliest round each entrant still has to play a match in
        let mut next_round = HashMap::new();
        for r#match in open() {
            for entrant in entrants(r#match) {
                let round = next_round.entry(entrant).or_insert(r#match.round());
                *round = (*round).min(r#match.round());
            }
        }
        open()
            .filter(|r#match| {
                entrants(r#match)
                    .iter()
                    .all(|entrant| next_round[entrant] == r#match.round())
            })
            .map(|r#match| self.match_row(phase, r#match))
            .collect()
    }

    /// Reports the result `entry` says, see [`Self::read_entered`].
    ///
    /// # Errors
    /// Returns an error if [`Self::read_entered`] reports none, or the result is not reported, see
    /// [`Self::report`].
    pub fn report_entered(&mut self, entry: &str) -> Result<Reported, ReportEnteredError> {
        let row = self.read_entered(entry).report?;
        let result = row.result.expect("an entry says a result");
        self.report(row.id, result).map_err(|error| match error {
            ReportError::AlreadyFinished => ReportEnteredError::AlreadyFinished(row.number),
            error => error.into(),
        })
    }

    /// The matches `entry` fits, and the result it says, which [`Self::report_entered`] would
    /// report. Spaces around it do not count. `entry` is either
    ///
    /// - the number of a match, as [`MatchRow::number`], and its result, separated by whitespace
    ///   or a `-`: `1` for a win of side A, `2` for one of side B, and `X` (or `x`) for a draw,
    ///   which needs no separator, e.g. `"3 1"`, `"3-1"`, `"12 X"` or `"12x"`. It fits that match,
    ///   also with a result already or without a result entered (`"3"`); or
    /// - part of the name of the winner, found as [`NameSearch`] finds it, optionally followed by
    ///   the number of the match and by `X` for a draw, in either order, each apart by whitespace,
    ///   e.g. `"ali"`, `"ali X"`, `"ali 5"` or `"ali 5 X"`. It fits the open matches with an
    ///   entrant of that name, and says a result only if they are one side of one match (one match
    ///   for a draw), whichever of them is playable now. A number
    ///   or an `X` at the end is read as a part of the name as long as a name contains it, e.g.
    ///   `"Malcolm X"` is Malcolm X's win, and `"Malcolm X X"` a draw of his match.
    ///
    /// The result is an error if the competition has not started, `entry` is empty, it is the
    /// number of a match alone, there is no match of the number entered, it has a result already,
    /// its format has no draws while a draw is entered, or the name entered fits no open match,
    /// both sides of one, or several matches.
    pub fn read_entered(&self, entry: &str) -> Entered {
        let nothing = |error: ReportEnteredError| Entered {
            found: Vec::new(),
            report: Err(error),
        };
        let Some(phase) = self.started_phase() else {
            return nothing(ReportError::NotStarted.into());
        };
        let entry = entry.trim();
        if entry.is_empty() {
            return nothing(ReportEnteredError::Unreadable);
        }
        let Some((number, result)) = read_entry(entry) else {
            return self.read_named(phase, entry);
        };
        let Some(r#match) = phase
            .data()
            .matches()
            .values()
            .find(|r#match| self::number(r#match) == number)
        else {
            return nothing(ReportEnteredError::NoMatch(number));
        };
        let row = self.match_row(phase, r#match);
        let report = match result {
            _ if r#match.is_finished() => Err(ReportEnteredError::AlreadyFinished(number)),
            None => Err(ReportEnteredError::MissingResult(number)),
            Some(MatchResult::Draw) if !phase.format().allows_draws() => {
                Err(ReportError::DrawNotAllowed.into())
            }
            Some(result) => Ok(MatchRow {
                result: Some(result),
                ..row.clone()
            }),
        };
        Entered {
            found: vec![row],
            report,
        }
    }

    /// What `entry`, part of a name and perhaps the number of a match and a draw, fits and says,
    /// see [`Self::read_entered`].
    fn read_named(&self, phase: &Phase, entry: &str) -> Entered {
        let mut open: Vec<_> = phase
            .data()
            .matches()
            .values()
            .filter(|r#match| !r#match.is_finished())
            .map(|r#match| self.match_row(phase, r#match))
            .collect();
        open.sort_by_key(|row| row.number);
        let readings = NamedEntry::readings(entry);
        // the longest name that fits
        let Some((reading, fitting)) = readings.iter().find_map(|reading| {
            let fitting = reading.fitting(&open);
            (!fitting.is_empty()).then_some((reading, fitting))
        }) else {
            // a name followed by a number, which fits only without it
            let error = readings
                .iter()
                .find_map(|reading| {
                    let number = reading.number?;
                    let anywhere = NamedEntry {
                        number: None,
                        ..*reading
                    };
                    (!anywhere.fitting(&open).is_empty()).then(|| ReportEnteredError::NotInMatch {
                        name: reading.name.to_owned(),
                        number,
                    })
                })
                .unwrap_or_else(|| ReportEnteredError::NoNameFound(entry.to_owned()));
            return Entered {
                found: Vec::new(),
                report: Err(error),
            };
        };
        let mut found: Vec<MatchRow> = Vec::new();
        for (row, _) in &fitting {
            if found.last().is_none_or(|last| last.id != row.id) {
                found.push((*row).clone());
            }
        }
        let report = if reading.draw && !phase.format().allows_draws() {
            Err(ReportError::DrawNotAllowed.into())
        } else {
            match (fitting.as_slice(), found.as_slice()) {
                ([(row, result)], _) => Ok(MatchRow {
                    result: Some(*result),
                    ..(*row).clone()
                }),
                (_, [row]) => Err(ReportEnteredError::BothSides {
                    name: reading.name.to_owned(),
                    number: row.number,
                }),
                (_, rows) => Err(ReportEnteredError::SeveralMatches {
                    name: reading.name.to_owned(),
                    matches: rows.len(),
                    number: rows[0].number,
                }),
            }
        };
        Entered { found, report }
    }

    /// Reports `result` for the match `id`, with nothing else entered.
    ///
    /// # Errors
    /// Returns an error if the competition has not started, has no such match, the match has a
    /// result already, or its format does not take the result.
    pub fn report(&mut self, id: MatchId, result: MatchResult) -> Result<Reported, ReportError> {
        self.report_decision(id, Decision::new(result), Values::new(), Vec::new())
    }

    /// A form to enter the result of the match `id` into, see [`ResultForm`].
    ///
    /// # Errors
    /// Returns an error if the competition has not started, has no such match, or the match has a
    /// result already.
    pub fn result_form(&self, id: MatchId) -> Result<ResultForm, ReportError> {
        let phase = self.started_phase().ok_or(ReportError::NotStarted)?;
        let r#match = phase
            .data()
            .matches()
            .get(&id)
            .ok_or(ReportError::UnknownMatch)?;
        if r#match.is_finished() {
            return Err(ReportError::AlreadyFinished);
        }
        let rules = phase.match_rules();
        let template = rules.template(phase.data(), r#match);
        Ok(ResultForm::new(id, dyn_clone::clone_box(rules), template))
    }

    /// Reports the result entered into `form`.
    ///
    /// # Errors
    /// Returns an error as [`Self::report`] does, and if the result neither says nor suggests who
    /// won.
    pub fn report_form(&mut self, form: &ResultForm) -> Result<Reported, ReportError> {
        let (decision, values, parts) = form.report();
        let decision = decision.ok_or(ReportError::Undecided)?;
        self.report_decision(form.match_id(), decision, values, parts)
    }

    fn report_decision(
        &mut self,
        id: MatchId,
        decision: Decision,
        values: Values,
        parts: Vec<Part>,
    ) -> Result<Reported, ReportError> {
        let phase = self.started_phase().ok_or(ReportError::NotStarted)?;
        let Some(r#match) = phase.data().matches().get(&id) else {
            return Err(ReportError::UnknownMatch);
        };
        let row = MatchRow {
            result: Some(decision.result()),
            ..self.match_row(phase, r#match)
        };
        let phase_id = phase.id();
        let matches_before = phase.data().matches().len();
        let report = values.into_iter().fold(
            ReportMatchResult::new(phase_id, id, decision),
            |report, (key, sides)| report.with_value(key, sides),
        );
        let report = parts.into_iter().fold(report, ReportMatchResult::with_part);
        self.act(Action::Report(row.clone()), |machine, history| {
            machine.apply_in(history, report)
        })
        .map_err(|error| {
            let error = rejection(error);
            match error.downcast_ref() {
                Some(engine::ReportMatchResultError::MatchAlreadyFinished) => {
                    ReportError::AlreadyFinished
                }
                Some(engine::ReportMatchResultError::DrawNotAllowed) => ReportError::DrawNotAllowed,
                _ => ReportError::Rejected(error),
            }
        })?;
        let matches = self
            .started_phase()
            .expect("still started")
            .data()
            .matches();
        Ok(Reported {
            row,
            new_matches: matches.len() - matches_before,
            winners: self.winners(),
        })
    }

    /// Undoes the result reported for the match `id`, which may be any reported match rather than
    /// only the one reported last. Every other change stays in effect.
    ///
    /// # Errors
    /// Returns an error if the competition has not started, has no such match, the match has no
    /// result, or a later result depends on it.
    pub fn undo_report(&mut self, id: MatchId) -> Result<Reopened, UndoReportError> {
        let phase = self.started_phase().ok_or(UndoReportError::NotStarted)?;
        let Some(r#match) = phase.data().matches().get(&id) else {
            return Err(UndoReportError::UnknownMatch);
        };
        let row = self.match_row(phase, r#match);
        let phase_id = phase.id();
        let matches_before = phase.data().matches().len();
        self.act(Action::Reopen(row.clone()), |machine, history| {
            machine.undo_match_result(history, phase_id, id)
        })
        .map_err(|error| match error {
            UndoMatchResultError::NotReported => UndoReportError::NotReported,
            UndoMatchResultError::Rejected(error) => self
                .reported_number(&error)
                .map_or(UndoReportError::DependedOn, UndoReportError::DependedOnBy),
        })?;
        let matches = self
            .started_phase()
            .expect("still started")
            .data()
            .matches();
        Ok(Reopened {
            row,
            unscheduled: matches_before - matches.len(),
        })
    }

    /// The standings of the phase, with a column for every calculator that offers one; `None`
    /// before the start.
    pub fn standings(&self) -> Option<Standings> {
        let phase = self.started_phase()?;
        let calculators = AnyAxis::of::<dyn MetricsCalculator>();
        // the calculators each criterion ranks by, in the order of the criteria
        let ranked_by: Vec<&str> = phase
            .slice_rankers()
            .iter()
            .filter_map(|criterion| {
                AnyAxis::of::<dyn SliceRanker>().entry(plugin_factory::id_of(&**criterion))
            })
            .flat_map(|criterion| criterion.providers_in(calculators))
            .map(PluginEntry::id)
            .collect();
        let mut columns: Vec<_> = phase
            .metrics_calculators()
            .iter()
            .filter_map(|calculator| {
                Some((plugin_factory::id_of(&**calculator), calculator.column()?))
            })
            .collect();
        // stable, so that the others stay in running order
        columns.sort_by_key(|(calculator, _)| {
            ranked_by
                .iter()
                .position(|ranked| ranked == calculator)
                .unwrap_or(usize::MAX)
        });
        let data = phase.data();
        let finished = data.matches().values().all(Match::is_finished);
        Some(Standings {
            columns: columns
                .iter()
                .map(|(calculator, column)| {
                    let entry = calculators.entry(calculator);
                    Column {
                        calculator,
                        header: column.header(),
                        label: entry.map_or(column.header(), PluginEntry::label),
                        description: entry.and_then(PluginEntry::description),
                        ranks: ranked_by.contains(calculator),
                    }
                })
                .collect(),
            rows: data
                .standings()
                .map(|ranking| StandingRow {
                    rank: ranking.rank(),
                    entrants: self.team(data, &[ranking.id()]),
                    played: ranking.played_matches().len(),
                    cells: columns
                        .iter()
                        .map(|(_, column)| {
                            Some(standing_cell(phase, ranking, column.cell(ranking)?))
                        })
                        .collect(),
                    medal: finished.then(|| Medal::of(ranking.rank())).flatten(),
                })
                .collect(),
        })
    }

    /// The action [`Self::undo`] would undo now, if any, e.g. so that undoing the start, which
    /// leaves play, is asked about first.
    pub fn next_undo(&self) -> Option<&Action> {
        self.done.last()
    }

    /// Undoes the latest action of this session that is still in effect, which may be an
    /// [`Self::undo_report`], bringing the result back. Returns the action undone.
    ///
    /// # Errors
    /// Returns [`NothingToUndo`] if every action of the session is undone.
    pub fn undo(&mut self) -> Result<Action, NothingToUndo> {
        // read before the undo, which removes the phase
        let phase = (self.done.last() == Some(&Action::AddPhase))
            .then(|| self.phase().map(Phase::config))
            .flatten();
        match self.history.undo_last(&mut self.machine) {
            Ok(_) => {
                let action = self.done.pop().expect("an action was undone");
                self.undone.push(action.clone());
                if phase.is_some() {
                    self.undone_phase = phase;
                }
                Ok(action)
            }
            Err(UndoLastError::NothingToUndo) => Err(NothingToUndo),
            Err(error) => unreachable!(
                "only the session appends to its log, so its latest action can be undone: {error}"
            ),
        }
    }

    /// Redoes the action this session undid last, as long as it took no other action since.
    /// Returns the action redone.
    ///
    /// # Errors
    /// Returns [`NothingToRedo`] if there is no such action.
    pub fn redo(&mut self) -> Result<Action, NothingToRedo> {
        match self.history.redo_last(&mut self.machine) {
            Ok(_) => {
                let action = self.undone.pop().expect("an action was redone");
                self.done.push(action.clone());
                Ok(action)
            }
            Err(RedoLastError::NothingToRedo) => Err(NothingToRedo),
            Err(error) => unreachable!(
                "only the session appends to its log, so what it undid last can be redone: {error}"
            ),
        }
    }

    /// Runs `run`, which appends to the log as the action `action` of the session, if anything.
    fn act<T, E>(
        &mut self,
        action: Action,
        run: impl FnOnce(&mut CompetitionStateMachine, &mut SessionHistory) -> Result<T, E>,
    ) -> Result<T, E> {
        let log_len = self.machine.log().len();
        let done = run(&mut self.machine, &mut self.history)?;
        if self.machine.log().len() > log_len {
            self.done.push(action);
            self.undone.clear();
        }
        Ok(done)
    }

    /// The number of the match whose result the entry of the log that `error` names reports, if it
    /// is one reporting a result of a match of the phase, e.g. the later result an undo refused
    /// would no longer apply to, or the one an edit of the phase refused does not fit.
    fn reported_number(&self, error: &StateMachineError) -> Option<usize> {
        let StateMachineError::Transition(id, _) = error else {
            return None;
        };
        let entry = self.machine.log().iter().find(|entry| entry.id() == *id)?;
        let LogEntryKind::Apply(transition) = entry.kind() else {
            return None;
        };
        let transition: &dyn Any = &**transition;
        let reported = transition.downcast_ref::<ReportMatchResult>()?;
        let r#match = self
            .started_phase()?
            .data()
            .matches()
            .get(&reported.match_id())?;
        Some(number(r#match))
    }

    /// The phase of the competition, whether started or not.
    fn phase(&self) -> Option<&Phase> {
        self.competition().phases().values().next()
    }

    fn started_phase(&self) -> Option<&Phase> {
        self.phase().filter(|_| self.competition().is_started())
    }

    fn entrant_row(&self, id: &EntrantId) -> EntrantRow {
        EntrantRow {
            id: *id,
            name: self.people.name(id).map(str::to_owned),
        }
    }

    fn team(&self, phase: &PhaseData, rankings: &[RankingId]) -> Vec<EntrantRow> {
        rankings
            .iter()
            .flat_map(|id| phase.rankings()[id].entrants())
            .map(|id| self.entrant_row(id))
            .collect()
    }

    /// The row of `r#match`, a match of `phase`.
    fn match_row(&self, phase: &Phase, r#match: &Match) -> MatchRow {
        let data = phase.data();
        MatchRow {
            id: r#match.id(),
            number: number(r#match),
            round: r#match.round(),
            stage: stage(phase, r#match.annotations()),
            a: self.team(data, r#match.team_a()),
            b: self.team(data, r#match.team_b()),
            result: r#match.result(),
        }
    }
}

/// The number of `r#match`, as [`MatchRow::number`] shows it.
fn number(r#match: &Match) -> usize {
    usize::try_from(r#match.number().get()).expect("a number of a match fits a usize")
}

/// The number of a match and its result, if any, that `entry` says, see
/// [`Session::read_entered`].
fn read_entry(entry: &str) -> Option<(usize, Option<MatchResult>)> {
    let entry = entry.trim();
    let digits = entry
        .find(|char: char| !char.is_ascii_digit())
        .unwrap_or(entry.len());
    let (number, rest) = entry.split_at(digits);
    let number = number.parse().ok()?;
    if rest.is_empty() {
        return Some((number, None));
    }
    let result = rest.trim_start_matches(|char: char| char.is_whitespace() || char == '-');
    // a win needs a separator, or its side would be read as a digit of the number
    let separated = result.len() < rest.len();
    let result = match result {
        "1" if separated => MatchResult::AWins,
        "2" if separated => MatchResult::BWins,
        "X" | "x" => MatchResult::Draw,
        _ => return None,
    };
    Some((number, Some(result)))
}

/// A way to read an entry of part of a name, see [`Session::read_entered`].
#[derive(Debug, Clone, Copy)]
struct NamedEntry<'a> {
    name: &'a str,
    /// The number of the match it is restricted to.
    number: Option<usize>,
    draw: bool,
}

impl<'a> NamedEntry<'a> {
    /// The ways to read `entry`, the longest name first: all of it, then without the last word if
    /// that is a number or an `X`, then without the last two if they are one of each.
    fn readings(entry: &'a str) -> Vec<Self> {
        let mut reading = Self {
            name: entry,
            number: None,
            draw: false,
        };
        let mut readings = vec![reading];
        for _ in 0..2 {
            let Some((before, last)) = reading.name.rsplit_once(char::is_whitespace) else {
                break;
            };
            if reading.number.is_none()
                && let Ok(number) = last.parse()
                && last.chars().all(|char| char.is_ascii_digit())
            {
                reading.number = Some(number);
            } else if !reading.draw && (last == "X" || last == "x") {
                reading.draw = true;
            } else {
                break;
            }
            reading.name = before.trim_end();
            readings.push(reading);
        }
        readings
    }

    /// The open matches of `open` this reading fits, with the result it says of each: a win of
    /// each side with an entrant of its name, or a draw of a match with one.
    fn fitting<'r>(&self, open: &'r [MatchRow]) -> Vec<(&'r MatchRow, MatchResult)> {
        let search = NameSearch::new(self.name);
        let finds = |side: &[EntrantRow]| side.iter().any(|entrant| search.finds(entrant));
        open.iter()
            .filter(|row| self.number.is_none_or(|number| row.number == number))
            .flat_map(|row| {
                let (a, b) = (finds(&row.a), finds(&row.b));
                let results = if self.draw {
                    vec![MatchResult::Draw]
                        .into_iter()
                        .filter(|_| a || b)
                        .collect()
                } else {
                    [(a, MatchResult::AWins), (b, MatchResult::BWins)]
                        .into_iter()
                        .filter(|(found, _)| *found)
                        .map(|(_, result)| result)
                        .collect::<Vec<_>>()
                };
                results.into_iter().map(move |result| (row, result))
            })
            .collect()
    }
}

/// `cell` of `ranking` as the standings show it: a round, a phrase with the number `round`, by the
/// stage of the ranking's match or bye in it, where the format names one.
fn standing_cell(phase: &Phase, ranking: &Ranking, cell: Cell) -> StandingCell {
    let round = match &cell {
        Cell::Text(phrase) => phrase
            .numbers()
            .iter()
            .find(|(name, _)| *name == "round")
            .map(|(_, round)| *round),
        Cell::Number(_) => None,
    };
    let Some(round) = round else {
        return StandingCell::Metric(cell);
    };
    let data = phase.data();
    let matches = ranking
        .played_matches()
        .iter()
        .filter_map(|id| data.matches().get(id))
        .filter(|r#match| r#match.round().get() == round)
        .map(Match::annotations);
    let byes = data
        .byes()
        .values()
        .filter(|bye| bye.ranking() == ranking.id() && bye.round().get() == round)
        .map(Bye::annotations);
    match matches
        .chain(byes)
        .next()
        .and_then(|annotations| stage(phase, annotations))
    {
        Some(stage) => StandingCell::Stage(stage),
        None => StandingCell::Metric(cell),
    }
}

/// The stage the format of `phase` names a match or a bye with `annotations` by.
fn stage(phase: &Phase, annotations: &TypeMap) -> Option<StageName> {
    let format = phase.format();
    Some(StageName {
        format: plugin_factory::id_of(format),
        stage: format.stage(annotations)?,
    })
}

/// The plugins `draft`, a phase draft, configures, with the plugins its answers imply added, see
/// [`Draft::add_implied`].
fn completed(
    draft: &Draft<PhasePlugins>,
) -> Result<PhasePlugins, Vec<plugin_factory::draft::Issue>> {
    let mut draft = draft.clone();
    draft.add_implied();
    draft.build()
}

/// The error of the transition an action of the session was rejected with.
fn rejection(error: StateMachineError) -> TransitionError {
    match error {
        StateMachineError::Transition(_, error) => error,
        // the session only appends new entries, each with a new id and targeting one in effect
        error => unreachable!("an action of the session is rejected by a transition: {error}"),
    }
}

/// What is stored of the competition `key` in `storage`.
async fn read<S: Storage>(storage: &S, key: CompetitionKey) -> Result<Stored, LoadError<S::Error>> {
    storage
        .read(key)
        .await
        .map_err(LoadError::Storage)?
        .ok_or(LoadError::NotFound)
}

/// A stored record as a session reads it: the entries of its log, not replayed yet.
type Record = File<Vec<LogEntry<Box<dyn CompetitionTransition>>>, People>;

/// Every plugin of `axis`, in the order of their ids.
fn plugins(axis: AnyAxis) -> Vec<Plugin> {
    axis.entries()
        .map(|entry| Plugin {
            id: entry.id(),
            label: entry.label(),
            description: entry.description(),
        })
        .collect()
}

/// `name` as it is compared to tell names that look alike, see [`Session::entrant_like`].
fn folded(name: &str) -> String {
    name.trim().nfkc().collect::<String>().to_lowercase()
}

mod bracket;
mod cross_table;
#[cfg(test)]
mod tests;
