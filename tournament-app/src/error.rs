//! Why a [`Session`](crate::session::Session) refused to do something. An action that fails leaves
//! the session unchanged.
//!
//! Each variant is something the operator is told, as a [`Message`] in the operator's language;
//! its `Display` says that message in English.

use std::{fmt::Display, num::NonZeroU32};

use plugin_factory::{config::ConfigError, draft::Issue};
use thiserror::Error;
use tournament_engine::{
    transitions::{self as engine, CompetitionAlreadyStarted},
    undoable_state_machine::{StateMachineError, TransitionError},
};

use crate::{
    file::Format,
    i18n::{Arg, Message, Text, english},
    view::{EntrantRow, PhaseName},
};

/// Why an entrant was not added.
#[derive(Debug, Error)]
#[error("{}", english(self))]
pub enum AddEntrantError {
    EmptyName,
    DuplicateName(String),
    Started,
    /// The engine refused the entrant for a reason this crate does not know.
    Rejected(TransitionError),
}

/// Why an entrant was not removed.
#[derive(Debug, Error)]
#[error("{}", english(self))]
pub enum RemoveEntrantError {
    UnknownEntrant,
    Started,
    /// The engine refused the removal for a reason this crate does not know.
    Rejected(TransitionError),
}

/// Why an entrant was not renamed.
#[derive(Debug, Error)]
#[error("{}", english(self))]
pub enum RenameEntrantError {
    UnknownEntrant,
    EmptyName,
    DuplicateName(String),
}

/// Why there is no draft of a phase played in a format.
#[derive(Debug, Error)]
#[error("{}", english(self))]
pub enum PhaseDraftError {
    /// No registered phase format has this id.
    UnknownFormat(String),
    /// The format offers an answer its own schema does not take.
    #[error(transparent)]
    Configuration(#[from] ConfigError),
}

/// Why a phase was not added.
#[derive(Debug, Error)]
#[error("{}", english(self))]
pub enum AddPhaseError {
    /// No phase can be built from the draft: its plugins do not fit together, or miss something no
    /// implied plugin provides. What is wrong is what the [`FormView`](crate::form::FormView) of
    /// the draft shows at its fields, in the operator's language.
    Invalid(Vec<Issue>),
    #[error(transparent)]
    Sources(#[from] SourcesError),
    /// The engine refused the phase for a reason this crate does not know.
    Rejected(TransitionError),
}

/// Why where a phase takes its entrants from was refused, as the
/// [`FormView`](crate::form::FormView) shows it too.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{}", english(self))]
pub enum SourcesError {
    /// The phase takes entrants from nowhere.
    NoSources,
    /// A place typed is not a place, see [`PhaseForm::enter_place`].
    ///
    /// [`PhaseForm::enter_place`]: crate::form::PhaseForm::enter_place
    NotTaken,
    /// A source ends before it starts.
    EmptyRange,
    /// A source takes a place another source takes: one of the same phase if `None`, else one of
    /// the phase named.
    Overlap(Option<PhaseName>),
    /// The competition has started, so a phase taking entrants from the registration alone would
    /// never start.
    RegistrationClosed,
}

/// The competition has no phase yet.
#[derive(Debug, Error)]
#[error("{}", english(self))]
pub struct NoPhase;

/// Why the phase was not changed.
#[derive(Debug, Error)]
#[error("{}", english(self))]
pub enum EditPhaseError {
    #[error(transparent)]
    NoPhase(#[from] NoPhase),
    /// No phase can be built from the draft, see [`AddPhaseError::Invalid`].
    Invalid(Vec<Issue>),
    /// The edited phase does not play the matches of these numbers (see
    /// [`MatchRow::number`](crate::view::MatchRow::number)), ascending and at least one, as they
    /// were reported, e.g. since its format does not schedule them; reopening them allows the
    /// edit.
    ResultDoesNotFit(Vec<usize>),
    #[error(transparent)]
    Sources(#[from] SourcesError),
    /// A phase that takes entrants from this one has started; undoing its start allows the edit.
    PhaseStarted(PhaseName),
    /// The edited phase does not fit another change made after the phase was added.
    DoesNotFit(TransitionError),
}

/// Why a phase did not start, or a warning about starting it, see
/// [`Session::start_phase_refusal`](crate::session::Session::start_phase_refusal).
#[derive(Debug, Error)]
#[error("{}", english(self))]
pub enum StartPhaseError {
    NoSuchPhase,
    Started,
    /// The phase takes entrants from the registration alone, so it starts with the competition.
    WithTheCompetition,
    CompetitionNotStarted,
    /// Phases the phase takes entrants from are not finished: each with the matches it has played
    /// of those it has.
    Waiting(Vec<(PhaseName, usize, usize)>),
    /// The format of the phase needs at least this many entrants, more than its sources take.
    TooFewForFormat(usize),
    /// The groups (divisions) the phase is configured with need at least `needed` entrants, the
    /// entrants its format needs in each, more than its sources take.
    TooFewForDivisions {
        divisions: usize,
        needed: usize,
    },
    /// Not a refusal but a warning: the phase starts all the same. A cut of its sources goes
    /// through rows sharing a rank, so the lot decides who of them is taken.
    CutThroughTie(Vec<Tie>),
    /// The engine refused the start for a reason this crate does not know.
    Rejected(TransitionError),
}

/// Rows of the standings of a phase sharing a rank that a cut between places goes through, see
/// [`StartPhaseError::CutThroughTie`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tie {
    /// The phase whose standings the rows are of.
    pub phase: PhaseName,
    pub rank: NonZeroU32,
    /// The teams of the rank before the cut, which the lot puts ahead.
    pub favoured: Vec<Vec<EntrantRow>>,
    /// The teams of the rank after the cut.
    pub others: Vec<Vec<EntrantRow>>,
}

/// Why a phase was not removed.
#[derive(Debug, Error)]
#[error("{}", english(self))]
pub enum RemovePhaseError {
    NoSuchPhase,
    Started,
    /// These phases take entrants from it.
    InUse(Vec<PhaseName>),
    /// The engine refused the removal for a reason this crate does not know.
    Rejected(TransitionError),
}

/// Why the competition did not start.
#[derive(Debug, Error)]
#[error("{}", english(self))]
pub enum StartError {
    Started,
    #[error(transparent)]
    NoPhase(#[from] NoPhase),
    TooFewEntrants,
    /// The format of the phase needs at least this many entrants, as its `min_entrants` says.
    TooFewForFormat(usize),
    /// The groups (divisions) a phase is configured with need at least `needed` entrants, the
    /// entrants its format needs in each.
    TooFewForDivisions {
        divisions: usize,
        needed: usize,
    },
    /// The engine refused the start for a reason this crate does not know.
    Rejected(TransitionError),
}

/// Why a result was not reported.
#[derive(Debug, Error)]
#[error("{}", english(self))]
pub enum ReportError {
    NotStarted,
    UnknownMatch,
    AlreadyFinished,
    DrawNotAllowed,
    /// A result entered that neither says nor suggests who won.
    Undecided,
    /// Something typed into the [`ResultForm`](crate::result_form::ResultForm) was not taken.
    NotTaken,
    /// The engine refused the result for a reason this crate does not know.
    Rejected(TransitionError),
}

/// Why a result entered as text was not reported, see
/// [`Session::report_entered`](crate::session::Session::report_entered).
#[derive(Debug, Error)]
#[error("{}", english(self))]
pub enum ReportEnteredError {
    Unreadable,
    /// The number of a match was entered alone.
    MissingResult(usize),
    NoMatch(usize),
    AlreadyFinished(usize),
    /// No open match has an entrant whose name contains what was entered, which it holds as
    /// entered, without the spaces around it.
    NoNameFound(String),
    /// Open matches have an entrant whose name contains `name`, but not the match `number`.
    NotInMatch {
        name: String,
        number: usize,
    },
    /// The name entered is found in `matches` open matches, e.g. in the match numbered `number`.
    SeveralMatches {
        name: String,
        matches: usize,
        number: usize,
    },
    /// The name entered is found on both sides of the match `number`, so it names no winner.
    BothSides {
        name: String,
        number: usize,
    },
    /// The match of this number does not take the scores entered: they are more than its parts,
    /// one is of a part without a score, or a decimal is of a score of whole numbers, see
    /// [`ResultForm::enter_scores`](crate::result_form::ResultForm::enter_scores).
    ScoresNotTaken(usize),
    /// The scores entered for the match of this number say no result, and none was entered.
    Undecided(usize),
    /// The result entered for the match of this number is not the one its scores entered say.
    AgainstScores(usize),
    #[error(transparent)]
    Report(#[from] ReportError),
}

/// Why a result was not undone.
#[derive(Debug, Error)]
#[error("{}", english(self))]
pub enum UndoReportError {
    NotStarted,
    UnknownMatch,
    NotReported,
    /// A later result depends on this one, e.g. that of the final the undone semifinal decided the
    /// participants of; it has to be undone first.
    DependedOn,
    /// The results of the matches of these numbers (see
    /// [`MatchRow::number`](crate::view::MatchRow::number)), ascending and at least one, depend on
    /// this one, e.g. that of the final the undone semifinal decided the participants of; they
    /// have to be undone first.
    DependedOnBy(Vec<usize>),
    /// A phase taking entrants from the phase of the match has started; undoing its start allows
    /// the undo.
    PhaseStarted(PhaseName),
}

/// The numbers of matches as a message lists them: `5, 7, 9`.
fn listed(numbers: &[usize]) -> String {
    numbers
        .iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Why [`Session::undo`](crate::session::Session::undo) undid nothing.
#[derive(Debug, Error)]
#[error("{}", english(self))]
pub enum UndoError {
    /// The session has done nothing it has not undone.
    NothingToUndo,
    /// A change of another session undid the action, or does not apply without it, so nothing more
    /// can be undone.
    ChangedElsewhere,
}

/// Why [`Session::redo`](crate::session::Session::redo) redid nothing.
#[derive(Debug, Error)]
#[error("{}", english(self))]
pub enum RedoError {
    /// The session has undone nothing since its latest action.
    NothingToRedo,
    /// A change of another session redid the action, or does not apply with it, so nothing more can
    /// be redone.
    ChangedElsewhere,
}

/// Why a stored competition was not loaded, with its source saying why that is. `E` is the error
/// of the storage.
#[derive(Debug, Error)]
#[error("{}", english(self))]
pub enum LoadError<E: Display> {
    NotFound,
    Storage(#[source] E),
    /// It was saved in this format, newer than [`Format::CURRENT`](crate::file::Format::CURRENT).
    Newer(Format),
    /// What is stored does not read, e.g. an entry of the log of a plugin this version does not
    /// have; the source says where.
    Damaged(#[source] serde_json::Error),
    /// The log does not replay into a competition.
    Replay(#[source] StateMachineError),
}

impl<E: Display> From<ImportError> for LoadError<E> {
    /// What stops a file from being imported stops the record holding it from being loaded.
    fn from(error: ImportError) -> Self {
        match error {
            ImportError::NoCompetition(error) | ImportError::Damaged(error) => Self::Damaged(error),
            ImportError::Newer(format) => Self::Newer(format),
        }
    }
}

/// Why a file was not imported.
#[derive(Debug, Error)]
#[error("{}", english(self))]
pub enum ImportError {
    /// The file is no JSON object holding a log and people; the source says where reading it
    /// stopped.
    NoCompetition(#[source] serde_json::Error),
    /// The file is of this format, newer than [`Format::CURRENT`](crate::file::Format::CURRENT).
    Newer(Format),
    /// The file holds a log and people that do not load, e.g. a log of a plugin this version does
    /// not have; the source says where.
    Damaged(#[source] serde_json::Error),
}

/// Why the engine refused the sources of a phase, see [`Refusal::Sources`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourcesRefusal {
    Empty,
    Overlap(Option<tournament_engine::PhaseId>),
    RegistrationClosed,
}

/// Why the engine refused an action, where the app says it in words of its own: the engine errors
/// the app knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Refusal {
    Started,
    UnknownEntrant,
    /// A source the app checks before, see [`SourcesError`]; carries what the engine says.
    Sources(SourcesRefusal),
    NoPhase,
    TooFewEntrants,
    MatchFinished,
    DrawNotAllowed,
}

impl Refusal {
    /// The refusal `error` says, if the app knows it.
    pub(crate) fn of(error: &TransitionError) -> Option<Self> {
        if error.is::<CompetitionAlreadyStarted>() {
            return Some(Self::Started);
        }
        if error.is::<engine::RemoveEntrantError>() {
            return Some(Self::UnknownEntrant);
        }
        if let Some(error) = error.downcast_ref::<engine::AddPhaseError>() {
            return match error {
                engine::AddPhaseError::EmptyRange(_) => Some(Self::Sources(SourcesRefusal::Empty)),
                engine::AddPhaseError::Overlap { phase, .. } => {
                    Some(Self::Sources(SourcesRefusal::Overlap(*phase)))
                }
                engine::AddPhaseError::RegistrationClosed => {
                    Some(Self::Sources(SourcesRefusal::RegistrationClosed))
                }
                engine::AddPhaseError::PhaseExists | engine::AddPhaseError::UnknownPhase(_) => None,
            };
        }
        if let Some(error) = error.downcast_ref() {
            return Some(match error {
                engine::StartCompetitionError::NoPhases => Self::NoPhase,
                engine::StartCompetitionError::TooFewEntrants => Self::TooFewEntrants,
            });
        }
        match error.downcast_ref() {
            Some(engine::ReportMatchResultError::MatchAlreadyFinished) => Some(Self::MatchFinished),
            Some(engine::ReportMatchResultError::DrawNotAllowed) => Some(Self::DrawNotAllowed),
            _ => None,
        }
    }
}

impl Message for AddEntrantError {
    fn text(&self) -> Text {
        match self {
            Self::EmptyName => Text::new("error-empty-name"),
            Self::DuplicateName(name) => {
                Text::new("error-duplicate-name").with("name", name.as_str())
            }
            Self::Started => Text::new("error-entrant-added-after-start"),
            Self::Rejected(error) => rejected(error),
        }
    }
}

impl Message for RemoveEntrantError {
    fn text(&self) -> Text {
        match self {
            Self::UnknownEntrant => Text::new("error-unknown-entrant"),
            Self::Started => Text::new("error-entrant-removed-after-start"),
            Self::Rejected(error) => rejected(error),
        }
    }
}

impl Message for RenameEntrantError {
    fn text(&self) -> Text {
        match self {
            Self::UnknownEntrant => Text::new("error-unknown-entrant"),
            Self::EmptyName => Text::new("error-empty-name"),
            Self::DuplicateName(name) => {
                Text::new("error-duplicate-name").with("name", name.as_str())
            }
        }
    }
}

impl Message for PhaseDraftError {
    fn text(&self) -> Text {
        match self {
            Self::UnknownFormat(format) => {
                Text::new("error-unknown-format").with("format", format.as_str())
            }
            Self::Configuration(error) => {
                Text::new("error-format-misconfigured").with("reason", error.to_string())
            }
        }
    }
}

impl Message for AddPhaseError {
    fn text(&self) -> Text {
        match self {
            Self::Invalid(_) => Text::new("error-invalid-phase"),
            Self::Sources(error) => error.text(),
            Self::Rejected(error) => rejected(error),
        }
    }
}

impl Message for SourcesError {
    fn text(&self) -> Text {
        match self {
            Self::NoSources => Text::new("error-no-sources"),
            Self::NotTaken => Text::new("error-value-not-taken"),
            Self::EmptyRange => Text::new("error-source-empty"),
            Self::Overlap(None) => Text::new("error-sources-overlap"),
            Self::Overlap(Some(phase)) => {
                Text::new("error-source-taken").with("phase", phase.clone())
            }
            Self::RegistrationClosed => Text::new("error-registration-closed"),
        }
    }
}

impl Message for StartPhaseError {
    fn text(&self) -> Text {
        match self {
            Self::NoSuchPhase => Text::new("error-no-such-phase"),
            Self::Started => Text::new("error-phase-started-already"),
            Self::WithTheCompetition => Text::new("error-phase-starts-with-competition"),
            Self::CompetitionNotStarted => Text::new("error-not-started"),
            Self::Waiting(waiting) => Text::new("error-phase-waiting").with(
                "phases",
                Arg::Waiting(
                    waiting
                        .iter()
                        .map(|(phase, played, matches)| (phase.clone(), *played, *matches))
                        .collect(),
                ),
            ),
            Self::TooFewForFormat(needed) => {
                Text::new("error-too-few-for-format").with("count", *needed)
            }
            Self::TooFewForDivisions { divisions, needed } => {
                Text::new("error-too-few-for-divisions")
                    .with("divisions", *divisions)
                    .with("count", *needed)
            }
            Self::CutThroughTie(ties) => {
                let tie = ties.first().expect("a tie at a cut is one at least");
                Text::new("warning-cut-through-tie")
                    .with("phase", tie.phase.clone())
                    .with("rank", tie.rank)
                    .with("favoured", names(&tie.favoured))
                    .with("others", names(&tie.others))
            }
            Self::Rejected(error) => rejected(error),
        }
    }
}

impl Message for RemovePhaseError {
    fn text(&self) -> Text {
        match self {
            Self::NoSuchPhase => Text::new("error-no-such-phase"),
            Self::Started => Text::new("error-phase-removed-after-start"),
            Self::InUse(phases) => Text::new("error-phase-in-use")
                .with("count", phases.len())
                .with("phases", Arg::Phases(phases.clone())),
            Self::Rejected(error) => rejected(error),
        }
    }
}

/// The names of `teams`, each by the names of its entrants: `Alice & Bob, Carol`.
fn names(teams: &[Vec<EntrantRow>]) -> String {
    teams
        .iter()
        .map(|team| {
            team.iter()
                .map(|entrant| entrant.name.as_deref().unwrap_or("?"))
                .collect::<Vec<_>>()
                .join(" & ")
        })
        .collect::<Vec<_>>()
        .join(", ")
}

impl Message for NoPhase {
    fn text(&self) -> Text {
        Text::new("error-no-phase")
    }
}

impl Message for EditPhaseError {
    fn text(&self) -> Text {
        match self {
            Self::NoPhase(no_phase) => no_phase.text(),
            Self::Invalid(_) => Text::new("error-invalid-phase"),
            Self::ResultDoesNotFit(numbers) => Text::new("error-result-does-not-fit")
                .with("count", numbers.len())
                .with("numbers", listed(numbers)),
            Self::Sources(error) => error.text(),
            Self::PhaseStarted(phase) => {
                Text::new("error-later-phase-started").with("phase", phase.clone())
            }
            Self::DoesNotFit(error) => {
                Text::new("error-phase-change-does-not-fit").with("reason", error.to_string())
            }
        }
    }
}

impl Message for StartError {
    fn text(&self) -> Text {
        match self {
            Self::Started => Text::new("error-started-already"),
            Self::NoPhase(no_phase) => no_phase.text(),
            Self::TooFewEntrants => Text::new("error-too-few-entrants"),
            Self::TooFewForFormat(needed) => {
                Text::new("error-too-few-for-format").with("count", *needed)
            }
            Self::TooFewForDivisions { divisions, needed } => {
                Text::new("error-too-few-for-divisions")
                    .with("divisions", *divisions)
                    .with("count", *needed)
            }
            Self::Rejected(error) => rejected(error),
        }
    }
}

impl Message for ReportError {
    fn text(&self) -> Text {
        match self {
            Self::NotStarted => Text::new("error-not-started"),
            Self::UnknownMatch => Text::new("error-unknown-match"),
            Self::AlreadyFinished => Text::new("error-match-finished-already"),
            Self::DrawNotAllowed => Text::new("error-draw-not-allowed"),
            Self::Undecided => Text::new("error-undecided"),
            Self::NotTaken => Text::new("error-value-not-taken"),
            Self::Rejected(error) => rejected(error),
        }
    }
}

impl Message for ReportEnteredError {
    fn text(&self) -> Text {
        match self {
            Self::Unreadable => Text::new("error-result-unreadable"),
            Self::NoMatch(number) => Text::new("error-no-match-numbered").with("number", *number),
            Self::AlreadyFinished(number) => {
                Text::new("error-match-numbered-finished-already").with("number", *number)
            }
            Self::MissingResult(number) => {
                Text::new("error-result-missing").with("number", *number)
            }
            Self::NoNameFound(entered) => {
                Text::new("error-no-name-found").with("entered", entered.as_str())
            }
            Self::NotInMatch { name, number } => Text::new("error-name-not-in-match")
                .with("name", name.as_str())
                .with("number", *number),
            Self::SeveralMatches {
                name,
                matches,
                number,
            } => Text::new("error-name-in-several-matches")
                .with("name", name.as_str())
                .with("matches", *matches)
                .with("number", *number),
            Self::BothSides { name, number } => Text::new("error-name-on-both-sides")
                .with("name", name.as_str())
                .with("number", *number),
            Self::ScoresNotTaken(number) => {
                Text::new("error-scores-not-taken").with("number", *number)
            }
            Self::Undecided(number) => Text::new("error-scores-undecided").with("number", *number),
            Self::AgainstScores(number) => {
                Text::new("error-result-against-scores").with("number", *number)
            }
            Self::Report(error) => error.text(),
        }
    }
}

impl Message for UndoReportError {
    fn text(&self) -> Text {
        match self {
            Self::NotStarted => Text::new("error-not-started"),
            Self::UnknownMatch => Text::new("error-unknown-match"),
            Self::NotReported => Text::new("error-match-not-reported"),
            Self::DependedOn => Text::new("error-result-depended-on"),
            Self::DependedOnBy(numbers) => Text::new("error-result-depended-on-by")
                .with("count", numbers.len())
                .with("numbers", listed(numbers)),
            Self::PhaseStarted(phase) => {
                Text::new("error-later-phase-started").with("phase", phase.clone())
            }
        }
    }
}

impl Message for UndoError {
    fn text(&self) -> Text {
        match self {
            Self::NothingToUndo => Text::new("error-nothing-to-undo"),
            Self::ChangedElsewhere => Text::new("error-undo-changed-elsewhere"),
        }
    }
}

impl Message for RedoError {
    fn text(&self) -> Text {
        match self {
            Self::NothingToRedo => Text::new("error-nothing-to-redo"),
            Self::ChangedElsewhere => Text::new("error-redo-changed-elsewhere"),
        }
    }
}

impl<E: Display> Message for LoadError<E> {
    fn text(&self) -> Text {
        match self {
            Self::NotFound => Text::new("error-competition-not-found"),
            Self::Storage(error) => {
                Text::new("error-competition-unreadable").with_detail(error.to_string())
            }
            Self::Newer(_) => Text::new("error-competition-newer"),
            Self::Damaged(error) => {
                Text::new("error-competition-damaged").with_detail(error.to_string())
            }
            Self::Replay(error) => {
                Text::new("error-log-does-not-replay").with_detail(error.to_string())
            }
        }
    }
}

impl Message for ImportError {
    fn text(&self) -> Text {
        match self {
            Self::NoCompetition(error) => {
                Text::new("error-file-no-competition").with_detail(error.to_string())
            }
            Self::Newer(_) => Text::new("error-file-newer"),
            Self::Damaged(error) => Text::new("error-file-damaged").with_detail(error.to_string()),
        }
    }
}

// The engine's own errors are passed on in English, since what they say is open-ended.

/// The engine refused an action for a reason this crate does not know.
fn rejected(error: &TransitionError) -> Text {
    Text::new("error-rejected").with("reason", error.to_string())
}
