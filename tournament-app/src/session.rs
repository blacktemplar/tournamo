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

use std::{collections::HashMap, num::NonZeroU32};

use plugin_factory::{catalog::AnyAxis, config::Config};
use tournament_engine::{
    Columns, Competition, CompetitionStateMachine, CompetitionStateMachineExt as _, Division,
    Match, MatchId, Part, Phase, PhaseId, Seed, SetCompetitionSeed, Source, StartCompetition,
    plugins::{match_format::Template, teams::Teams},
    transitions::{Change, ConflictsError},
    undoable_state_machine::{SessionHistory, StateMachineError, TransitionError},
    uuid::Uuid,
    vec1::Vec1,
};

use crate::{
    error::{DoesNotFit, MatchesOfPhase, NoPhase, Refusal, StartError},
    i18n::Language,
    people::People,
    storage::CompetitionKey,
    view::{Actions, DivisionRef, MatchesOf, Plugin},
};

/// The name of a competition imported from a file that names it by nothing but spaces, and whose
/// own name is blank as well, see [`Session::import`].
pub const UNNAMED: &str = "Competition";

/// The most characters a name has, of an entrant or of a competition: a longer one is refused,
/// or cut where a file brings it, see [`tidied`].
pub const MAX_NAME: usize = 100;

/// `name` as it is kept: without the whitespace around it, each run of whitespace in it one
/// space, so that names that look alike are alike.
pub fn tidied(name: &str) -> String {
    name.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `name` [`tidied`] and cut to [`MAX_NAME`] characters, e.g. of a file.
pub(crate) fn cut(name: &str) -> String {
    let tidied: String = tidied(name).chars().take(MAX_NAME).collect();
    tidied.trim_end().to_owned()
}

/// The most matches [`Session::scale`] plays out, far more than a day's play.
pub const MOST_MATCHES: usize = 10_000;

/// The result of a match whose report was undone, and the template of the match it was reported
/// with.
#[derive(Debug, Clone)]
struct UndoneReport {
    template: Template,
    contest: Part,
}

impl UndoneReport {
    /// What `r#match` of `division` had reported for it.
    fn of(division: &Division, r#match: &Match) -> Self {
        Self {
            template: division.match_format().template(division.data(), r#match),
            contest: r#match.contest().clone(),
        }
    }
}

/// A phase whose adding was undone: its name, sources and configuration.
#[derive(Debug, Clone)]
struct UndonePhase {
    name: Option<String>,
    sources: Vec1<Source>,
    config: Config,
    columns: Columns,
}

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
    /// The phase whose adding this session undid last, which the form of a new phase starts out
    /// at.
    undone_phase: Option<UndonePhase>,
    /// What was reported for each match whose report this session undid ([`Self::undo`]), the
    /// latest of each, which the form of the match starts out at until the match is reopened, see
    /// [`Self::result_form`].
    undone_reports: HashMap<(PhaseId, MatchId), UndoneReport>,
    /// How often the session changed the people: with the length of the log, which every other
    /// change makes longer, undo and redo included, it counts the changes, see [`Self::changes`].
    people_changes: usize,
    /// [`Self::changes`] when the session was last saved, loaded or caught up.
    saved_changes: usize,
    /// Whether the storage holds the competition the log replays into, which it does not if
    /// nothing is saved yet or it was saved by a version that did not store it.
    competition_saved: bool,
    /// The language the operator types numbers in, see [`Self::set_language`].
    language: Language,
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
            .apply_new(SetCompetitionSeed::new(Seed::new(seed)).into())
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
            undone_reports: HashMap::new(),
            people_changes: 0,
            saved_changes: 0,
            competition_saved: false,
            language: Language::default(),
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

    /// Sets the language the operator types numbers in, e.g. ratings and the numbers of a form
    /// the session makes, which are read as the language writes them, see
    /// [`Rating::parse`](crate::people::Rating::parse). English until set; not saved.
    pub fn set_language(&mut self, language: Language) {
        self.language = language;
    }

    /// The language the operator types numbers in, see [`Self::set_language`].
    pub fn language(&self) -> Language {
        self.language
    }

    /// The name the operator gave the competition.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Takes `name`, which the competition was renamed to in its storage (see
    /// [`Storage::rename`](crate::storage::Storage::rename)), e.g. by another tab. Nothing is
    /// saved: a save keeps the name stored.
    pub fn renamed(&mut self, name: impl Into<String>) {
        self.name = name.into();
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
            rate_entrant: !started && !competition.entrants().is_empty(),
            // after the start, a phase takes entrants from another one
            add_phase: !started || has_phase,
            edit_phase: has_phase,
            start: self.start_refusal().is_none(),
            report: competition
                .phases()
                .values()
                .any(|phase| phase.is_started() && !phase.is_finished()),
            undo: self.history.next_undo(&self.machine).is_some(),
            redo: self.history.next_redo(&self.machine).is_some(),
        }
    }

    /// Why [`Self::start`] would refuse to start the competition now, or `None` if it would start
    /// it.
    pub fn start_refusal(&self) -> Option<StartError> {
        let competition = self.competition();
        let starting = || {
            competition
                .phases()
                .values()
                .filter(|phase| phase.is_fed_by_registration_alone())
        };
        if competition.is_started() {
            Some(StartError::Started)
        } else if starting().next().is_none() {
            Some(NoPhase.into())
        } else if competition.entrants().len() < 2 {
            Some(StartError::TooFewEntrants)
        } else {
            starting().find_map(|phase| {
                match self.cannot_start(phase, self.field_taken(phase.sources()))? {
                    CannotStart::Format(needed) => Some(StartError::TooFewForFormat(needed)),
                    CannotStart::Divisions { divisions, needed } => {
                        Some(StartError::TooFewForDivisions { divisions, needed })
                    }
                    CannotStart::Teams(does_not_fit) => {
                        Some(StartError::FieldDoesNotFit(does_not_fit))
                    }
                }
            })
        }
    }

    /// Starts the competition, which schedules the first matches. The registration is ordered by
    /// rating, best first, entrants of equal ratings and those without one by lot (see
    /// [`Self::set_rating`]); only that order is in the log, not the ratings.
    ///
    /// # Errors
    /// Returns an error if the competition has started already, has no phase, or fewer than two
    /// entrants, or fewer than the format of a phase starting needs, or its groups need.
    pub fn start(&mut self) -> Result<(), StartError> {
        if let Some(
            needed @ (StartError::TooFewForFormat(_)
            | StartError::TooFewForDivisions { .. }
            | StartError::FieldDoesNotFit(_)),
        ) = self.start_refusal()
        {
            return Err(needed);
        }
        let start = StartCompetition::new(self.order_by_rating());
        self.history
            .apply_new(&mut self.machine, start.into())
            .map_err(|error| {
                refused(error, StartError::Rejected, |refusal| match refusal {
                    Refusal::Started => Some(StartError::Started),
                    Refusal::NoPhase => Some(NoPhase.into()),
                    Refusal::TooFewEntrants => Some(StartError::TooFewEntrants),
                    _ => None,
                })
            })?;
        Ok(())
    }

    /// The numbers of the matches whose results would no longer apply if `change` were made,
    /// ascending, see
    /// [`conflicts_of`](tournament_engine::CompetitionStateMachineExt::conflicts_of).
    /// The matches whose results would no longer apply after `change` of the phase `changed`, by
    /// phase: those of `changed` first, then those of each later phase, in the order of the
    /// phases; none of a phase without such a match.
    fn conflicting_by_phase(
        &self,
        change: Change<'_>,
        changed: PhaseId,
    ) -> Result<Vec<MatchesOfPhase>, ConflictsError> {
        let conflicts = self.machine.conflicts_of(change)?;
        let mut by_phase: Vec<_> = self
            .competition()
            .phases()
            .values()
            .filter_map(|phase| {
                let mut numbers: Vec<_> = conflicts
                    .iter()
                    .filter(|conflict| conflict.phase_id == phase.id())
                    .filter_map(|conflict| {
                        let (_, _, r#match) =
                            self.match_of(conflict.phase_id, conflict.match_id)?;
                        Some(views::number(r#match))
                    })
                    .collect();
                numbers.sort_unstable();
                (!numbers.is_empty()).then(|| MatchesOfPhase {
                    phase: (phase.id() != changed).then(|| self.phase_name(phase)),
                    numbers,
                })
            })
            .collect();
        // stable, so the later phases stay in order
        by_phase.sort_by_key(|of| of.phase.is_some());
        Ok(by_phase)
    }

    fn conflicting_numbers(&self, change: Change<'_>) -> Result<Vec<usize>, ConflictsError> {
        let mut numbers: Vec<_> = self
            .machine
            .conflicts_of(change)?
            .iter()
            .filter_map(|conflict| {
                let (_, _, r#match) = self.match_of(conflict.phase_id, conflict.match_id)?;
                Some(views::number(r#match))
            })
            .collect();
        numbers.sort_unstable();
        Ok(numbers)
    }

    /// The phase `id`, whether started or not.
    fn phase(&self, id: PhaseId) -> Option<&Phase> {
        self.competition().phase(&id)
    }

    /// The divisions whose matches `of` are, with their indices; none if the phase has not
    /// started.
    fn divisions_of(&self, of: MatchesOf) -> Vec<(usize, &Division)> {
        self.phase(of.phase)
            .into_iter()
            .flat_map(|phase| phase.divisions().iter().enumerate())
            .filter(|(index, _)| of.division.is_none_or(|division| division == *index))
            .collect()
    }

    /// The division `division`, once its phase is started.
    fn division(&self, division: DivisionRef) -> Option<&Division> {
        self.phase(division.phase)?
            .divisions()
            .get(division.division)
    }

    /// The match `id` of the phase `phase`, with its division and the division's index.
    fn match_of(&self, phase: PhaseId, id: MatchId) -> Option<(usize, &Division, &Match)> {
        let (index, division) = self.phase(phase)?.division_of(&id)?;
        Some((index, division, &division.data().matches()[&id]))
    }

    /// The places `sources` take, of the registration as it is now and of the phases as many as
    /// they take in turn, and the entrants they stand for: what a phase of these sources starts
    /// with, or would.
    fn field_taken(&self, sources: &[Source]) -> Taken {
        sources.iter().fold(Taken::default(), |taken, source| {
            let places = source.count(self.places_of(source.of()));
            let per_place = match source.of() {
                tournament_engine::SourceOf::Registration => 1,
                tournament_engine::SourceOf::Phase(id) => self
                    .competition()
                    .entrants_per_ranking(&id)
                    .map_or(1, |size| size.get() as usize),
            };
            Taken {
                qualifiers: taken.qualifiers + places,
                entrants: taken.entrants + places * per_place,
            }
        })
    }

    /// The number of places of `of`: the entrants of the registration, or the rankings of a
    /// started phase, or else the teams it forms of the places its sources take.
    fn places_of(&self, of: tournament_engine::SourceOf) -> usize {
        match of {
            tournament_engine::SourceOf::Registration => self.competition().entrants().len(),
            tournament_engine::SourceOf::Phase(id) => self.phase(id).map_or(0, |phase| {
                if phase.is_started() {
                    phase.standings().len()
                } else {
                    teams_formed(
                        phase.plugins().teams(),
                        self.field_taken(phase.sources()),
                        self.competition().entrants_per_qualifier(phase.sources()),
                    )
                }
            }),
        }
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

/// The error of an action of the session the engine refused with `error`: what `known` makes of
/// the refusal, if the app knows it and the action can be refused for it, or else what `rejected`
/// makes of the engine's error.
fn refused<E>(
    error: StateMachineError,
    rejected: impl FnOnce(TransitionError) -> E,
    known: impl FnOnce(Refusal) -> Option<E>,
) -> E {
    let error = rejection(error);
    Refusal::of(&error)
        .and_then(known)
        .unwrap_or_else(|| rejected(error))
}

mod basic;
mod bracket;
mod cross_table;
mod entrants;
mod entry;
mod phase;
mod phases;
mod template;
pub use basic::{Dyp, MatchFormats};
pub(crate) use phases::named;
mod report;
mod scale;
mod storage;
#[cfg(test)]
pub(crate) mod tests;
mod undo;
mod views;

/// Why a phase cannot start with the entrants its sources take, see [`Session::cannot_start`].
pub(crate) enum CannotStart {
    /// Its format needs at least this many.
    Format(usize),
    /// Its groups need at least `needed`, the entrants its format needs in each.
    Divisions { divisions: usize, needed: usize },
    /// They cannot be formed into its teams.
    Teams(Box<DoesNotFit>),
}

impl Session {
    /// Whether `phase` cannot start with the field `taken` from its sources: they do not fit
    /// its teams (see [`Teams::fits`]), or its teams are fewer than its format needs, or than it
    /// needs in each of its groups (divisions). The engine starts it with fewer groups all the
    /// same, so that a log never fails to replay; the app refuses, so that the operator lowers
    /// the groups.
    ///
    /// [`Teams::fits`]: tournament_engine::plugins::teams::Teams::fits
    pub(crate) fn cannot_start(&self, phase: &Phase, taken: Taken) -> Option<CannotStart> {
        let qualifiers = taken.qualifiers;
        let plugins = phase.plugins();
        if !plugins.teams().fits(qualifiers) {
            return Some(CannotStart::Teams(Box::new(DoesNotFit {
                phase: self.phase_name(phase),
                entrants: qualifiers,
                teams: teams_plugin(plugins.teams()),
            })));
        }
        let teams = teams_formed(
            plugins.teams(),
            taken,
            self.competition().entrants_per_qualifier(phase.sources()),
        );
        let per_division = plugins.format().min_entrants();
        let divisions = plugins.divisions().get() as usize;
        let needed = per_division.saturating_mul(divisions);
        if teams < per_division {
            Some(CannotStart::Format(per_division))
        } else if teams < needed {
            Some(CannotStart::Divisions { divisions, needed })
        } else {
            None
        }
    }
}

/// The field of a phase as its sources take it: how many qualifiers, and how many entrants those
/// stand for.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Taken {
    pub(crate) qualifiers: usize,
    pub(crate) entrants: usize,
}

/// The plugin `teams`, as a list of them shows it.
pub(crate) fn teams_plugin(teams: &dyn Teams) -> Plugin {
    let id = plugin_factory::id_of(teams);
    let entry = AnyAxis::of::<dyn Teams>().entry(id);
    Plugin {
        id,
        label: entry.map_or(id, |entry| entry.label()),
        description: entry.and_then(|entry| entry.description()),
    }
}

/// How many teams `teams` forms of the field `taken`, each qualifier of `per_qualifier` entrants
/// (`None`: of different sizes); as many as qualifiers if the teams would not be of one size.
pub(crate) fn teams_formed(
    teams: &dyn Teams,
    taken: Taken,
    per_qualifier: Option<NonZeroU32>,
) -> usize {
    teams
        .entrants_per_team(per_qualifier)
        .map_or(taken.qualifiers, |per_team| {
            taken.entrants / per_team.get() as usize
        })
}
