//! The transitions that drive a [`Competition`] forward, and the enum a transition log holds them
//! as.

use serde::{Deserialize, Serialize};
use thiserror::Error;
use undoable_state_machine::{
    LogEntry, LogEntryKind, SessionHistory, StateMachine, StateMachineError, Transition,
    TransitionError, TransitionId,
};

use crate::{
    SourceOf,
    entities::{Competition, MatchId, PhaseId},
};

pub mod add_entrant;
pub mod add_phase;
pub mod remove_entrant;
pub mod report_match_result;
pub mod set_competition_seed;
pub mod start_competition;
pub mod start_phase;

pub use add_entrant::{AddEntrant, AddEntrantError};
pub use add_phase::{AddPhase, AddPhaseError};
pub use remove_entrant::{RemoveEntrant, RemoveEntrantError};
pub use report_match_result::{ReportMatchResult, ReportMatchResultError};
pub use set_competition_seed::SetCompetitionSeed;
pub use start_competition::{StartCompetition, StartCompetitionError};
pub use start_phase::{StartPhase, StartPhaseError};

/// A transition of a [`Competition`], as a transition log holds it: one of the transitions of this
/// module.
///
/// Serialized as `{<variant>: <transition>}`. The name of a variant is a persistence contract:
/// renaming one needs a converter of the files saved before.
///
/// # Examples
/// ```
/// use tournament_engine::{CompetitionTransition, StartCompetition};
///
/// let transition = CompetitionTransition::from(StartCompetition::default());
/// let json = serde_json::to_string(&transition).unwrap();
///
/// assert_eq!(json, r#"{"StartCompetition":{"order":[]}}"#);
/// assert!(serde_json::from_str::<CompetitionTransition>(&json).is_ok());
/// ```
#[derive(Debug, Serialize, Deserialize)]
pub enum CompetitionTransition {
    SetCompetitionSeed(SetCompetitionSeed),
    AddEntrant(AddEntrant),
    RemoveEntrant(RemoveEntrant),
    AddPhase(AddPhase),
    StartCompetition(StartCompetition),
    ReportMatchResult(ReportMatchResult),
    StartPhase(StartPhase),
}

/// `$body` with `$transition` bound to the transition `$self`, a `CompetitionTransition`, holds,
/// whatever its variant.
macro_rules! with_transition {
    ($self:expr, $transition:ident => $body:expr) => {
        match $self {
            Self::SetCompetitionSeed($transition) => $body,
            Self::AddEntrant($transition) => $body,
            Self::RemoveEntrant($transition) => $body,
            Self::AddPhase($transition) => $body,
            Self::StartCompetition($transition) => $body,
            Self::ReportMatchResult($transition) => $body,
            Self::StartPhase($transition) => $body,
        }
    };
}

impl Transition<Competition> for CompetitionTransition {
    fn check_applicable(&self, competition: &Competition) -> Result<(), TransitionError> {
        with_transition!(self, transition => transition.check_applicable(competition))
    }

    fn apply(&self, competition: &mut Competition) {
        with_transition!(self, transition => transition.apply(competition));
    }
}

macro_rules! from_transitions {
    ($($transition:ident),*) => {$(
        impl From<$transition> for CompetitionTransition {
            fn from(transition: $transition) -> Self {
                Self::$transition(transition)
            }
        }
    )*};
}

from_transitions!(
    SetCompetitionSeed,
    AddEntrant,
    RemoveEntrant,
    AddPhase,
    StartCompetition,
    ReportMatchResult,
    StartPhase
);

/// The state machine of a competition, driven by transitions that can be persisted.
///
/// Serializes as its append-only log, undo entries included. Deserializing replays that log and
/// recomputes [`StateMachine::current`] from it rather than reading a stored copy, so a log that
/// does not replay cleanly fails to deserialize.
///
/// This is the persistence path that keeps the history and therefore undo and redo. The other one
/// is a snapshot of [`StateMachine::current`] alone, which is smaller and cheaper to load.
pub type CompetitionStateMachine = StateMachine<Competition, CompetitionTransition>;

/// A transition that only applies before the competition starts was applied after it started.
#[derive(Debug, Error)]
#[error("competition already started")]
pub struct CompetitionAlreadyStarted;

/// Fails with [`CompetitionAlreadyStarted`] if `competition` has started.
pub(crate) fn ensure_not_started(
    competition: &Competition,
) -> Result<(), CompetitionAlreadyStarted> {
    if competition.is_started() {
        Err(CompetitionAlreadyStarted)
    } else {
        Ok(())
    }
}

/// Why the result reported for a match could not be undone.
#[derive(Debug, Error)]
pub enum UndoMatchResultError {
    #[error("no result that is in effect was reported for this match")]
    NotReported,
    /// The undo was rejected by the state machine, which for an undo means that a later change in
    /// the log depends on the undone one — e.g. the reported result of a match that is only
    /// scheduled because of it. The error names the change that would no longer apply.
    #[error(transparent)]
    Rejected(#[from] StateMachineError),
}

/// Why the result reported for a match could not be replaced.
#[derive(Debug, Error)]
pub enum ReplaceMatchResultError {
    #[error("no result that is in effect was reported for this match")]
    NotReported,
    /// The new report does not apply in place of the one in effect, e.g. a draw in a match format
    /// without draws.
    #[error(transparent)]
    Refused(TransitionError),
    /// The replacement was rejected by the state machine, because a later change in the log does
    /// not apply after it, e.g. the result of a match that is only scheduled because of who won
    /// the replaced one. The error names the change that would no longer apply.
    #[error(transparent)]
    Rejected(StateMachineError),
}

/// Why a phase could not be reconfigured.
#[derive(Debug, Error)]
pub enum ReconfigurePhaseError {
    #[error("no phase with this id was added")]
    NotAdded,
    /// The reconfiguration was rejected by the state machine, because the phase cannot be built
    /// from the config, or a later change in the log does not apply to the reconfigured phase, e.g.
    /// the result reported for a match its new format does not schedule. The error names the
    /// change that would no longer apply.
    #[error(transparent)]
    Rejected(#[from] StateMachineError),
}

/// Why a phase could not be removed.
#[derive(Debug, Error)]
pub enum RemovePhaseError {
    #[error("no phase with this id was added")]
    NotAdded,
    #[error("the phase has started")]
    Started,
    /// These phases take entrants from it.
    #[error("other phases take entrants from the phase")]
    InUse(Vec<PhaseId>),
    /// The undo was rejected by the state machine, because a later change in the log depends on
    /// the phase.
    #[error(transparent)]
    Rejected(#[from] StateMachineError),
}

/// Why the start of a phase could not be undone.
#[derive(Debug, Error)]
pub enum UnstartPhaseError {
    /// No phase with this id has started.
    #[error("the phase has not started")]
    NotStarted,
    /// Results are reported in these phases, which undoing the start would unstart.
    #[error("results are reported in the phase")]
    Played(Vec<PhaseId>),
    /// These phases have started, taking entrants from a phase undoing the start would unstart.
    #[error("phases taking entrants from the phase have started")]
    InUse(Vec<PhaseId>),
    /// The undo was rejected by the state machine, because a later change in the log depends on
    /// the start otherwise.
    #[error(transparent)]
    Rejected(#[from] StateMachineError),
}

/// A change to the log that is not made, for [`CompetitionStateMachineExt::conflicts_of`].
#[derive(Debug, Clone, Copy)]
pub enum Change<'a> {
    /// Undoing the [`LogEntryKind::Apply`] with this id.
    Undo(TransitionId),
    /// Replacing the transition of the [`LogEntryKind::Apply`] `target` by `with`.
    Replace {
        target: TransitionId,
        with: &'a CompetitionTransition,
    },
}

/// A reported result that would no longer apply after a [`Change`]: the one the log entry `entry`
/// reported for the match `match_id` of the phase `phase_id`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Conflict {
    pub entry: TransitionId,
    pub phase_id: PhaseId,
    pub match_id: MatchId,
}

/// Why the conflicts of a [`Change`] cannot be listed.
#[derive(Debug, Error)]
pub enum ConflictsError {
    /// The change targets no [`LogEntryKind::Apply`] in effect.
    #[error("the entry to change is not in effect")]
    NotInEffect,
    /// The transition the [`LogEntryKind::Apply`] with this id runs, or the one a
    /// [`Change::Replace`] runs in its place, would not apply, and reports no result.
    #[error("a change that reports no result would no longer apply")]
    Other(TransitionId, #[source] TransitionError),
}

/// Undoing a change by what it did, and replacing a result by its match and reconfiguring a phase
/// by its id rather than by the [`TransitionId`] of the log entry that reported or added it, which
/// is all [`CompetitionStateMachine`] itself takes; finding those entries, and what a change of
/// them would conflict with.
///
/// A transition is applied by [`StateMachine::apply_new`], or, as an action of a session, by
/// [`SessionHistory::apply_new`], so that [`SessionHistory::undo_last`] and
/// [`SessionHistory::redo_last`] undo and redo it. A [`SetCompetitionSeed`] is better applied
/// outside any session, since everything random that follows depends on it.
pub trait CompetitionStateMachineExt {
    /// Undoes the result reported for the match `match_id` of the phase `phase_id` by undoing the
    /// latest log entry that is still in effect and reported it, as an action of the session of
    /// `history`, and returns the id of the undo entry appended for that. Undoing that action, see
    /// [`SessionHistory::undo_last`], brings the result back.
    ///
    /// This reaches back arbitrarily far: changes made after the report stay in effect, as long as
    /// none of them depends on it.
    ///
    /// # Errors
    /// Returns [`UndoMatchResultError::NotReported`] if no report for that match is in effect, and
    /// [`UndoMatchResultError::Rejected`] if undoing it would leave a later change in the log
    /// inapplicable. In either case neither the log nor `history` changes.
    ///
    /// # Examples
    /// ```
    /// # use tournament_engine::{
    /// #     AddEntrant, AddPhase, CompetitionStateMachine, CompetitionStateMachineExt, Entrant,
    /// #     EntrantId, MatchResult, PhaseId, PhasePluginsBuilder, ReportMatchResult, Seed,
    /// #     SetCompetitionSeed, StartCompetition,
    /// #     undoable_state_machine::SessionHistory, uuid::Uuid,
    /// # };
    /// # let phase_id = PhaseId::new(Uuid::from_u128(1));
    /// # let mut machine = CompetitionStateMachine::default();
    /// # machine.apply_new(SetCompetitionSeed::new(Seed::new(*b"doc-test!!!!")).into())?;
    /// # for entrant in 0..4 {
    /// #     let entrant = Entrant::new(EntrantId::new(Uuid::from_u128(entrant)));
    /// #     machine.apply_new(AddEntrant::new(entrant).into())?;
    /// # }
    /// # let plugins = PhasePluginsBuilder::new().build()?;
    /// # machine.apply_new(AddPhase::new(phase_id, plugins).into())?;
    /// # machine.apply_new(StartCompetition::default().into())?;
    /// // a round robin between four entrants, with the first and the last match reported
    /// let matches: Vec<_> = machine.current().phases()[&phase_id].divisions()[0]
    ///     .data()
    ///     .matches()
    ///     .keys()
    ///     .copied()
    ///     .collect();
    /// for index in [0, 5] {
    ///     machine.apply_new(
    ///         ReportMatchResult::new(phase_id, matches[index], MatchResult::AWins).into(),
    ///     )?;
    /// }
    ///
    /// let mut history = SessionHistory::new();
    /// machine.undo_match_result(&mut history, phase_id, matches[0])?;
    ///
    /// let phase = machine.current().phases()[&phase_id].divisions()[0].data();
    /// assert_eq!(phase.matches()[&matches[0]].result(), None);
    /// assert_eq!(
    ///     phase.matches()[&matches[5]].result(),
    ///     Some(MatchResult::AWins)
    /// );
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    fn undo_match_result(
        &mut self,
        history: &mut SessionHistory,
        phase_id: PhaseId,
        match_id: MatchId,
    ) -> Result<TransitionId, UndoMatchResultError>;

    /// Replaces the result reported for the match of `report` by `report`, as an action of the
    /// session of `history`, and returns the id of the entry appended for that: a
    /// [`LogEntryKind::Replace`] of the [`ReportMatchResult`] in effect for the match (see
    /// [`Self::reported_by`]). Every change made after it applies as before, so it is accepted as
    /// long as the later results still apply, e.g. where the winner stays. Undoing that action,
    /// see [`SessionHistory::undo_last`], brings the earlier result back.
    ///
    /// Returns `None`, leaving the log unchanged, if `report` enters the match as it was reported
    /// already (see [`Part::is_entered_as`](crate::Part::is_entered_as)).
    ///
    /// # Errors
    /// Returns [`ReplaceMatchResultError::NotReported`] if no report for that match is in effect,
    /// [`ReplaceMatchResultError::Refused`] if `report` does not apply in its place, and
    /// [`ReplaceMatchResultError::Rejected`] if a later change in the log does not apply after it
    /// ([`Self::conflicts_of`] names the results among those). In either case neither the log nor
    /// `history` changes.
    ///
    /// # Examples
    /// ```
    /// # use tournament_engine::{
    /// #     AddEntrant, AddPhase, CompetitionStateMachine, CompetitionStateMachineExt, Entrant,
    /// #     EntrantId, MatchResult, PhaseId, PhasePluginsBuilder, ReportMatchResult, Seed,
    /// #     SetCompetitionSeed, StartCompetition,
    /// #     undoable_state_machine::SessionHistory, uuid::Uuid,
    /// # };
    /// # let phase_id = PhaseId::new(Uuid::from_u128(1));
    /// # let mut machine = CompetitionStateMachine::default();
    /// # machine.apply_new(SetCompetitionSeed::new(Seed::new(*b"doc-test!!!!")).into())?;
    /// # for entrant in 0..4 {
    /// #     let entrant = Entrant::new(EntrantId::new(Uuid::from_u128(entrant)));
    /// #     machine.apply_new(AddEntrant::new(entrant).into())?;
    /// # }
    /// # let plugins = PhasePluginsBuilder::new().build()?;
    /// # machine.apply_new(AddPhase::new(phase_id, plugins).into())?;
    /// # machine.apply_new(StartCompetition::default().into())?;
    /// // a round robin between four entrants, its first match won by A
    /// let first = *machine.current().phases()[&phase_id].divisions()[0]
    ///     .data()
    ///     .matches()
    ///     .keys()
    ///     .next()
    ///     .expect("a round robin has matches");
    /// machine.apply_new(ReportMatchResult::new(phase_id, first, MatchResult::AWins).into())?;
    ///
    /// let mut history = SessionHistory::new();
    /// let won_by_b = ReportMatchResult::new(phase_id, first, MatchResult::BWins);
    /// machine.replace_match_result(&mut history, won_by_b)?;
    ///
    /// let result = |machine: &CompetitionStateMachine| {
    ///     machine.current().phases()[&phase_id].divisions()[0]
    ///         .data()
    ///         .matches()[&first]
    ///         .result()
    /// };
    /// assert_eq!(result(&machine), Some(MatchResult::BWins));
    /// history.undo_last(&mut machine)?;
    /// assert_eq!(result(&machine), Some(MatchResult::AWins));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    fn replace_match_result(
        &mut self,
        history: &mut SessionHistory,
        report: ReportMatchResult,
    ) -> Result<Option<TransitionId>, ReplaceMatchResultError>;

    /// Reconfigures the phase `phase.id()` as if it had been added by `phase`, as an action of
    /// the session of `history`, and returns the id of the entry appended for that: a
    /// [`LogEntryKind::Replace`] of the [`AddPhase`] in effect that added it by `phase`. Every
    /// change made after it applies to the reconfigured phase, so the results reported so far stay
    /// as long as the reconfigured phase schedules their matches as well, and every phase started
    /// after it starts with the field it has then.
    ///
    /// The plugins are built from a draft holding the config of the phase, see
    /// [`Phase::config`](crate::Phase::config), changed as needed. Returns `None`, leaving the log
    /// unchanged, if the phase has its name, sources, config and columns already. The phase keeps
    /// the [`TemplateOrigin`](crate::plugins::template::TemplateOrigin) it was added with, whatever
    /// `phase` says.
    ///
    /// # Errors
    /// Returns [`ReconfigurePhaseError::NotAdded`] if no [`AddPhase`] in effect added a phase with
    /// this id, and [`ReconfigurePhaseError::Rejected`] if `phase` does not apply in its place (see
    /// [`AddPhase`]) or a change made after the phase was added does not apply to the
    /// reconfigured phase. In either case neither the log nor `history` changes.
    ///
    /// # Examples
    /// ```
    /// # use tournament_engine::{
    /// #     AddEntrant, AddPhase, CompetitionStateMachine, CompetitionStateMachineExt, Entrant,
    /// #     EntrantId, PhaseId, PhasePluginsBuilder, Seed, SetCompetitionSeed, StartCompetition,
    /// #     plugin_factory::config::PluginConfig, plugins::phase_format::RandomRounds,
    /// #     undoable_state_machine::SessionHistory, uuid::Uuid,
    /// # };
    /// # let phase_id = PhaseId::new(Uuid::from_u128(1));
    /// # let mut machine = CompetitionStateMachine::default();
    /// # machine.apply_new(SetCompetitionSeed::new(Seed::new(*b"doc-test!!!!")).into())?;
    /// # for entrant in 0..4 {
    /// #     let entrant = Entrant::new(EntrantId::new(Uuid::from_u128(entrant)));
    /// #     machine.apply_new(AddEntrant::new(entrant).into())?;
    /// # }
    /// # let plugins = PhasePluginsBuilder::new().build()?;
    /// # machine.apply_new(AddPhase::new(phase_id, plugins).into())?;
    /// # machine.apply_new(StartCompetition::default().into())?;
    /// // a started round robin, changed into two random rounds of it
    /// let competition = machine.current();
    /// let phase = &competition.phases()[&phase_id];
    /// let plugins = competition
    ///     .phase_draft(phase.sources())
    ///     .with_answers(&phase.config())?
    ///     .with(
    ///         "format",
    ///         PluginConfig::of::<RandomRounds>().with("rounds", 2),
    ///     )?
    ///     .build()
    ///     .expect("the plugins fit together");
    ///
    /// let mut history = SessionHistory::new();
    /// machine.reconfigure_phase(&mut history, AddPhase::new(phase_id, plugins))?;
    ///
    /// // the phase was started in its new format: two rounds of two matches
    /// let phase = &machine.current().phases()[&phase_id];
    /// assert_eq!(
    ///     format!("{:?}", phase.plugins().format()),
    ///     "RandomRounds { rounds: 2 }"
    /// );
    /// assert_eq!(phase.divisions()[0].data().matches().len(), 4);
    ///
    /// // undoing the reconfiguration plays the phase as a round robin again
    /// history.undo_last(&mut machine)?;
    /// let phase = &machine.current().phases()[&phase_id];
    /// assert_eq!(format!("{:?}", phase.plugins().format()), "RoundRobin");
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    fn reconfigure_phase(
        &mut self,
        history: &mut SessionHistory,
        phase: AddPhase,
    ) -> Result<Option<TransitionId>, ReconfigurePhaseError>;

    /// Removes the phase `phase_id` by undoing the [`AddPhase`] in effect that added it, as an
    /// action of the session of `history`, and returns the id of the undo entry appended for that.
    /// Undoing that action, see [`SessionHistory::undo_last`], brings the phase back.
    ///
    /// # Errors
    /// Returns [`RemovePhaseError::NotAdded`] if no [`AddPhase`] in effect added a phase with this
    /// id, [`RemovePhaseError::Started`] if it has started, [`RemovePhaseError::InUse`] if other
    /// phases take entrants from it, and [`RemovePhaseError::Rejected`] if a later change in the
    /// log depends on it otherwise. In either case neither the log nor `history` changes.
    fn remove_phase(
        &mut self,
        history: &mut SessionHistory,
        phase_id: PhaseId,
    ) -> Result<TransitionId, RemovePhaseError>;

    /// Undoes the start of the phase `phase_id` by undoing the transition in effect that started
    /// it, as an action of the session of `history`, and returns the id of the undo entry appended
    /// for that. Undoing that action, see [`SessionHistory::undo_last`], starts the phase again.
    ///
    /// A phase that takes entrants from phases was started by a [`StartPhase`]; one fed by the
    /// registration alone started with the competition, by the [`StartCompetition`], so undoing
    /// its start unstarts the competition and every phase that started with it. Starting it again
    /// draws the same lots, which come from the competition's seed.
    ///
    /// # Errors
    /// Returns [`UnstartPhaseError::NotStarted`] if no phase with this id has started,
    /// [`UnstartPhaseError::InUse`] if a phase taking entrants from a phase it would unstart has
    /// started, [`UnstartPhaseError::Played`] if a result is reported in one of those, and
    /// [`UnstartPhaseError::Rejected`] if a later change in the log depends on the start
    /// otherwise. In either case neither the log nor `history` changes.
    fn unstart_phase(
        &mut self,
        history: &mut SessionHistory,
        phase_id: PhaseId,
    ) -> Result<TransitionId, UnstartPhaseError>;

    /// The id of the log entry in effect that started the phase `phase_id`, which
    /// [`Self::unstart_phase`] undoes: its [`StartPhase`], or the [`StartCompetition`] for a phase
    /// fed by the registration alone.
    ///
    /// # Errors
    /// Returns why [`Self::unstart_phase`] would refuse to undo it, except for
    /// [`UnstartPhaseError::Rejected`], which only undoing it finds out.
    fn started_by(&self, phase_id: PhaseId) -> Result<TransitionId, UnstartPhaseError>;

    /// The id of the log entry in effect that reported the result of the match `match_id` of the
    /// phase `phase_id`: the latest [`ReportMatchResult`] in effect for it, which
    /// [`Self::undo_match_result`] undoes. `None` if there is none.
    fn reported_by(&self, phase_id: PhaseId, match_id: MatchId) -> Option<TransitionId>;

    /// The id of the [`AddPhase`] entry in effect that added the phase `phase_id`, which
    /// [`Self::reconfigure_phase`] replaces. `None` if there is none.
    fn added_by(&self, phase_id: PhaseId) -> Option<TransitionId>;

    /// The reported results that would no longer apply if `change` were made, in log order: each
    /// result whose match the changed competition does not schedule, has finished already or does
    /// not accept the result for, and every result that depends on one of those. Empty if the
    /// change would be accepted. The log and the competition are not changed.
    ///
    /// Listing them replays the log, as making the change does, so it is meant for after a change
    /// was refused, e.g. by [`Self::undo_match_result`], to name every result in its way.
    ///
    /// # Errors
    /// Returns [`ConflictsError::NotInEffect`] if `change` targets no [`LogEntryKind::Apply`] in
    /// effect, and [`ConflictsError::Other`] if a transition reporting no result would not apply,
    /// such as the one `change` replaces with.
    ///
    /// # Examples
    /// ```
    /// # use tournament_engine::{
    /// #     AddEntrant, AddPhase, CompetitionStateMachine, CompetitionStateMachineExt, Entrant,
    /// #     EntrantId, MatchId, MatchResult, PhaseId, PhasePluginsBuilder, ReportMatchResult,
    /// #     Seed, SetCompetitionSeed, StartCompetition,
    /// #     plugins::phase_format::SingleElimination,
    /// #     transitions::{Change, Conflict},
    /// #     uuid::Uuid,
    /// # };
    /// # let phase_id = PhaseId::new(Uuid::from_u128(1));
    /// # let mut machine = CompetitionStateMachine::default();
    /// # machine.apply_new(SetCompetitionSeed::new(Seed::new(*b"doc-test!!!!")).into())?;
    /// # for entrant in 0..4 {
    /// #     let entrant = Entrant::new(EntrantId::new(Uuid::from_u128(entrant)));
    /// #     machine.apply_new(AddEntrant::new(entrant).into())?;
    /// # }
    /// # let plugins = PhasePluginsBuilder::new()
    /// #     .format(SingleElimination::new())
    /// #     .build()?;
    /// # machine.apply_new(AddPhase::new(phase_id, plugins).into())?;
    /// # machine.apply_new(StartCompetition::default().into())?;
    /// # let matches = |machine: &CompetitionStateMachine| -> Vec<MatchId> {
    /// #     let phase = &machine.current().phases()[&phase_id];
    /// #     phase.divisions()[0].data().matches().keys().copied().collect()
    /// # };
    /// // a knockout between four entrants: both semifinals reported, then the final
    /// let semifinals = matches(&machine);
    /// let report = |match_id, result| ReportMatchResult::new(phase_id, match_id, result).into();
    /// let first = machine.apply_new(report(semifinals[0], MatchResult::AWins))?;
    /// machine.apply_new(report(semifinals[1], MatchResult::AWins))?;
    /// let r#final = matches(&machine)[2];
    /// let final_reported = machine.apply_new(report(r#final, MatchResult::BWins))?;
    ///
    /// // without the first semifinal, the final is not played
    /// assert_eq!(
    ///     machine.conflicts_of(Change::Undo(first))?,
    ///     [Conflict {
    ///         entry: final_reported,
    ///         phase_id,
    ///         match_id: r#final
    ///     }]
    /// );
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    fn conflicts_of(&self, change: Change<'_>) -> Result<Vec<Conflict>, ConflictsError>;
}

impl CompetitionStateMachineExt for CompetitionStateMachine {
    fn undo_match_result(
        &mut self,
        history: &mut SessionHistory,
        phase_id: PhaseId,
        match_id: MatchId,
    ) -> Result<TransitionId, UndoMatchResultError> {
        let target = self
            .reported_by(phase_id, match_id)
            .ok_or(UndoMatchResultError::NotReported)?;
        let id = TransitionId::new();
        history.apply(self, LogEntry::new(id, LogEntryKind::Undo(target)))?;
        Ok(id)
    }

    fn replace_match_result(
        &mut self,
        history: &mut SessionHistory,
        report: ReportMatchResult,
    ) -> Result<Option<TransitionId>, ReplaceMatchResultError> {
        let target = self
            .reported_by(report.phase_id(), report.match_id())
            .ok_or(ReplaceMatchResultError::NotReported)?;
        if let Some(CompetitionTransition::ReportMatchResult(reported)) =
            self.effective_transition(target)
            && reported.contest().is_entered_as(report.contest())
        {
            return Ok(None);
        }
        let id = TransitionId::new();
        let with = report.into();
        history
            .apply(
                self,
                LogEntry::new(id, LogEntryKind::Replace { target, with }),
            )
            .map_err(|error| match error {
                // the state machine names a replacement that does not apply by the replacing entry
                StateMachineError::Transition(failed, error) if failed == id => {
                    ReplaceMatchResultError::Refused(error)
                }
                error => ReplaceMatchResultError::Rejected(error),
            })?;
        Ok(Some(id))
    }

    fn reconfigure_phase(
        &mut self,
        history: &mut SessionHistory,
        phase: AddPhase,
    ) -> Result<Option<TransitionId>, ReconfigurePhaseError> {
        let target = self
            .added_by(phase.id())
            .ok_or(ReconfigurePhaseError::NotAdded)?;
        if self.current().phase(&phase.id()).is_some_and(|added| {
            added.name() == phase.name()
                && added.sources() == phase.sources()
                && added.config() == phase.plugins().config()
                && added.columns() == phase.columns()
        }) {
            return Ok(None);
        }
        // where the phase came from stays with it
        let template = self
            .current()
            .phase(&phase.id())
            .and_then(|added| added.template().cloned());
        let id = TransitionId::new();
        let with = phase.with_template(template).into();
        history.apply(
            self,
            LogEntry::new(id, LogEntryKind::Replace { target, with }),
        )?;
        Ok(Some(id))
    }

    fn remove_phase(
        &mut self,
        history: &mut SessionHistory,
        phase_id: PhaseId,
    ) -> Result<TransitionId, RemovePhaseError> {
        let target = self.added_by(phase_id).ok_or(RemovePhaseError::NotAdded)?;
        let competition = self.current();
        if competition
            .phase(&phase_id)
            .is_some_and(|phase| phase.is_started())
        {
            return Err(RemovePhaseError::Started);
        }
        let users: Vec<_> = competition
            .phases()
            .values()
            .filter(|phase| {
                phase
                    .sources()
                    .iter()
                    .any(|source| source.of() == SourceOf::Phase(phase_id))
            })
            .map(|phase| phase.id())
            .collect();
        if !users.is_empty() {
            return Err(RemovePhaseError::InUse(users));
        }
        let id = TransitionId::new();
        history.apply(self, LogEntry::new(id, LogEntryKind::Undo(target)))?;
        Ok(id)
    }

    fn unstart_phase(
        &mut self,
        history: &mut SessionHistory,
        phase_id: PhaseId,
    ) -> Result<TransitionId, UnstartPhaseError> {
        let target = self.started_by(phase_id)?;
        let id = TransitionId::new();
        history.apply(self, LogEntry::new(id, LogEntryKind::Undo(target)))?;
        Ok(id)
    }

    fn started_by(&self, phase_id: PhaseId) -> Result<TransitionId, UnstartPhaseError> {
        let competition = self.current();
        let with_competition = competition
            .phase(&phase_id)
            .filter(|phase| phase.is_started())
            .ok_or(UnstartPhaseError::NotStarted)?
            .is_fed_by_registration_alone();
        let unstarted: Vec<_> = competition
            .phases()
            .values()
            .filter(|phase| {
                phase.is_started()
                    && if with_competition {
                        phase.is_fed_by_registration_alone()
                    } else {
                        phase.id() == phase_id
                    }
            })
            .collect();
        let users: Vec<_> = competition
            .phases()
            .values()
            .filter(|phase| {
                phase.is_started()
                    && phase.sources().iter().any(|source| {
                        unstarted
                            .iter()
                            .any(|from| source.of() == SourceOf::Phase(from.id()))
                    })
            })
            .map(|phase| phase.id())
            .collect();
        if !users.is_empty() {
            return Err(UnstartPhaseError::InUse(users));
        }
        let played: Vec<_> = unstarted
            .iter()
            .filter(|phase| {
                phase.divisions().iter().any(|division| {
                    division
                        .data()
                        .matches()
                        .values()
                        .any(|r#match| r#match.is_finished())
                })
            })
            .map(|phase| phase.id())
            .collect();
        if !played.is_empty() {
            return Err(UnstartPhaseError::Played(played));
        }
        self.rev_in_effect_applied()
            .find_map(|(id, transition)| match transition {
                CompetitionTransition::StartPhase(start) if start.phase_id() == phase_id => {
                    Some(id)
                }
                CompetitionTransition::StartCompetition(_) if with_competition => Some(id),
                _ => None,
            })
            .ok_or(UnstartPhaseError::NotStarted)
    }

    fn reported_by(&self, phase_id: PhaseId, match_id: MatchId) -> Option<TransitionId> {
        self.rev_in_effect_applied().find_map(|(id, transition)| {
            matches!(
                transition,
                CompetitionTransition::ReportMatchResult(report)
                    if (report.phase_id(), report.match_id()) == (phase_id, match_id)
            )
            .then_some(id)
        })
    }

    fn added_by(&self, phase_id: PhaseId) -> Option<TransitionId> {
        // a replacement of an `AddPhase` made by `reconfigure_phase` adds the same phase, so the
        // transition that runs in place of the entry adds it, whether replaced or not
        self.rev_in_effect_applied().find_map(|(id, transition)| {
            matches!(
                transition,
                CompetitionTransition::AddPhase(added) if added.id() == phase_id
            )
            .then_some(id)
        })
    }

    fn conflicts_of(&self, change: Change<'_>) -> Result<Vec<Conflict>, ConflictsError> {
        let (Change::Undo(target) | Change::Replace { target, .. }) = change;
        if self.effective_transition(target).is_none() {
            return Err(ConflictsError::NotInEffect);
        }
        // as `StateMachine` replays its log, but with `change` made, and skipping each report that
        // does not apply rather than stopping there: the matches it would have scheduled are
        // missing then, so the reports depending on it are skipped as well
        let mut competition = Competition::default();
        let mut conflicts = Vec::new();
        for entry in self.log() {
            // `None` for any entry but an `Apply` in effect; that of its replacement if replaced
            let Some(transition) = self.effective_transition(entry.id()) else {
                continue;
            };
            let transition = match change {
                Change::Undo(_) if entry.id() == target => continue,
                Change::Replace { with, .. } if entry.id() == target => with,
                _ => transition,
            };
            if let Err(error) = transition.check_applicable(&competition) {
                let CompetitionTransition::ReportMatchResult(report) = transition else {
                    return Err(ConflictsError::Other(entry.id(), error));
                };
                conflicts.push(Conflict {
                    entry: entry.id(),
                    phase_id: report.phase_id(),
                    match_id: report.match_id(),
                });
                continue;
            }
            transition.apply(&mut competition);
        }
        Ok(conflicts)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use plugin_factory::config::Config;
    use uuid::Uuid;
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::*;
    use crate::{
        EnteredValue, Entrant, EntrantId, Match, MatchResult, PhasePluginsBuilder, Seed, Sides,
        Source,
        plugins::{
            match_format::{SCORE, Scored},
            metrics_calculator::{WinsDrawsLosses, WinsDrawsLossesCalculator},
            phase_format::{MonsterDyp, SingleElimination},
        },
    };

    fn phase_id() -> PhaseId {
        PhaseId::new(Uuid::from_u128(0xfa5e))
    }

    #[track_caller]
    fn undo(machine: &mut CompetitionStateMachine, target: TransitionId) -> TransitionId {
        let id = TransitionId::new();
        machine
            .apply(LogEntry::new(id, LogEntryKind::Undo(target)))
            .expect("the undo must be accepted");
        id
    }

    /// A started knockout between four entrants, which schedules two semifinals and, once both of
    /// them are reported, the final.
    fn knockout() -> CompetitionStateMachine {
        knockout_of(4)
    }

    /// A started knockout between `entrants` entrants, which schedules a match once the matches
    /// deciding who plays it are reported.
    fn knockout_of(entrants: u128) -> CompetitionStateMachine {
        started(
            entrants,
            PhasePluginsBuilder::new().format(SingleElimination::new()),
        )
    }

    /// A started knockout between four entrants decided by a score.
    fn scored_knockout() -> CompetitionStateMachine {
        started(
            4,
            PhasePluginsBuilder::new()
                .format(SingleElimination::new())
                .match_format(Scored::new(None)),
        )
    }

    /// A started competition of `entrants` entrants and the phase `plugins` builds.
    fn started(entrants: u128, plugins: PhasePluginsBuilder) -> CompetitionStateMachine {
        let mut machine = CompetitionStateMachine::default();
        machine
            .apply_new(SetCompetitionSeed::new(Seed::new(*b"undo-test!!!")).into())
            .unwrap();
        for entrant in 0..entrants {
            let entrant = Entrant::new(EntrantId::new(Uuid::from_u128(entrant)));
            machine.apply_new(AddEntrant::new(entrant).into()).unwrap();
        }
        let plugins = plugins.build().unwrap();
        machine
            .apply_new(AddPhase::new(phase_id(), plugins).into())
            .unwrap();
        machine
            .apply_new(StartCompetition::default().into())
            .unwrap();
        machine
    }

    /// The config of the phase as it is played.
    fn config(machine: &CompetitionStateMachine) -> Config {
        machine.current().phases()[&phase_id()].config()
    }

    /// The matches of the phase, in the order they were scheduled in.
    fn matches(machine: &CompetitionStateMachine) -> Vec<MatchId> {
        machine.current().phases()[&phase_id()].divisions()[0]
            .data()
            .matches()
            .keys()
            .copied()
            .collect()
    }

    fn result_of(machine: &CompetitionStateMachine, match_id: MatchId) -> Option<MatchResult> {
        machine.current().phases()[&phase_id()].divisions()[0]
            .data()
            .matches()[&match_id]
            .result()
    }

    #[track_caller]
    fn report(machine: &mut CompetitionStateMachine, match_id: MatchId) -> TransitionId {
        machine
            .apply_new(ReportMatchResult::new(phase_id(), match_id, MatchResult::AWins).into())
            .expect("the match must be reportable")
    }

    /// Reports every match until none is open, returning the entry reporting each.
    fn play_out(machine: &mut CompetitionStateMachine) -> HashMap<MatchId, TransitionId> {
        let mut reports = HashMap::new();
        while let Some(open) = matches(machine)
            .into_iter()
            .find(|&r#match| result_of(machine, r#match).is_none())
        {
            reports.insert(open, report(machine, open));
        }
        reports
    }

    /// The conflict of the result `entry` reported for the match `match_id` of the phase.
    fn conflict(entry: TransitionId, match_id: MatchId) -> Conflict {
        Conflict {
            entry,
            phase_id: phase_id(),
            match_id,
        }
    }

    #[test]
    fn a_competition_takes_a_single_phase() {
        let mut machine = CompetitionStateMachine::default();
        let plugins = PhasePluginsBuilder::new().build().unwrap();
        machine
            .apply_new(AddPhase::new(phase_id(), plugins.clone()).into())
            .unwrap();

        let second = AddPhase::new(PhaseId::new(Uuid::from_u128(2)), plugins);

        assert!(machine.apply_new(second.into()).is_err());
        assert_eq!(machine.current().phases().len(), 1);
    }

    /// Undoing the result of a match is an action of the session, so undoing that action brings
    /// the result back.
    #[test]
    fn undoing_the_undo_of_a_match_result_brings_it_back() {
        let mut machine = knockout();
        let first = matches(&machine)[0];
        report(&mut machine, first);
        let mut history = SessionHistory::new();
        machine
            .undo_match_result(&mut history, phase_id(), first)
            .unwrap();

        history
            .undo_last(&mut machine)
            .expect("the undo of the result is an action of the session");

        assert_eq!(result_of(&machine, first), Some(MatchResult::AWins));
    }

    /// Undoing the earlier of two reports leaves the later one in effect, which is the whole point
    /// of undoing by match rather than by walking the log back from its end.
    #[test]
    fn a_report_is_undone_without_touching_the_reports_made_after_it() {
        let mut machine = knockout();
        let [first, second] = matches(&machine)[..] else {
            panic!("a knockout between four entrants starts with two semifinals");
        };
        report(&mut machine, first);
        report(&mut machine, second);

        machine
            .undo_match_result(&mut SessionHistory::new(), phase_id(), first)
            .expect("the report of the first semifinal must be undoable");

        assert_eq!(result_of(&machine, first), None);
        assert_eq!(result_of(&machine, second), Some(MatchResult::AWins));
        assert_eq!(
            matches(&machine).len(),
            2,
            "the final is only scheduled once both semifinals are reported"
        );
    }

    #[test]
    fn a_match_that_was_never_reported_has_nothing_to_undo() {
        let mut machine = knockout();
        let open = matches(&machine)[0];

        let error = machine
            .undo_match_result(&mut SessionHistory::new(), phase_id(), open)
            .expect_err("an open match must not be undoable");

        assert!(matches!(error, UndoMatchResultError::NotReported));
    }

    #[test]
    fn a_report_that_is_already_undone_is_not_undone_twice() {
        let mut machine = knockout();
        let first = matches(&machine)[0];
        report(&mut machine, first);
        let mut history = SessionHistory::new();
        machine
            .undo_match_result(&mut history, phase_id(), first)
            .unwrap();
        let log_len = machine.log().len();

        let error = machine
            .undo_match_result(&mut history, phase_id(), first)
            .expect_err("an undone report must not be undoable again");

        assert!(matches!(error, UndoMatchResultError::NotReported));
        assert_eq!(machine.log().len(), log_len, "the log must stay unchanged");
    }

    /// Only the report that is in effect is undone, which after an undo and a redo of the same
    /// match is an earlier log entry than the one that reported it last.
    #[test]
    fn the_report_that_was_put_back_in_effect_is_the_one_undone() {
        let mut machine = knockout();
        let first = matches(&machine)[0];
        let reported = report(&mut machine, first);
        let undone = undo(&mut machine, reported);
        // reporting the match again and undoing that report leaves the first report the only one
        // that any further undo could still cancel
        let reported_again = report(&mut machine, first);
        undo(&mut machine, reported_again);
        undo(&mut machine, undone);
        assert_eq!(result_of(&machine, first), Some(MatchResult::AWins));

        let id = machine
            .undo_match_result(&mut SessionHistory::new(), phase_id(), first)
            .unwrap();

        assert_eq!(result_of(&machine, first), None);
        assert!(matches!(
            machine.log().last().expect("the undo entry").kind(),
            LogEntryKind::Undo(target) if *target == reported,
        ));
        assert_ne!(id, reported);
    }

    /// A knockout schedules its final from the reported semifinals, so undoing a semifinal would
    /// leave the report of the final without a match to report, which the state machine rejects
    /// rather than dropping that report.
    #[test]
    fn a_report_a_later_report_depends_on_is_not_undone() {
        let mut machine = knockout();
        let semifinals = matches(&machine);
        report(&mut machine, semifinals[0]);
        report(&mut machine, semifinals[1]);
        let r#final = matches(&machine)[2];
        report(&mut machine, r#final);
        let log_len = machine.log().len();

        let error = machine
            .undo_match_result(&mut SessionHistory::new(), phase_id(), semifinals[0])
            .expect_err("the final depends on the semifinal, so undoing it must be rejected");

        assert!(matches!(error, UndoMatchResultError::Rejected(_)));
        assert_eq!(result_of(&machine, semifinals[0]), Some(MatchResult::AWins));
        assert_eq!(result_of(&machine, r#final), Some(MatchResult::AWins));
        assert_eq!(machine.log().len(), log_len, "the log must stay unchanged");
    }

    /// The report of the match `match_id` of the phase with the score `a`–`b`.
    fn scored(match_id: MatchId, a: u16, b: u16) -> ReportMatchResult {
        let score = |count| Some(EnteredValue::Count(count));
        let result = if a > b {
            MatchResult::AWins
        } else {
            MatchResult::BWins
        };
        ReportMatchResult::new(phase_id(), match_id, result)
            .with_value(SCORE, Sides::new(score(a), score(b)))
    }

    /// The score reported for the match `match_id` of the phase.
    fn score_of(machine: &CompetitionStateMachine, match_id: MatchId) -> Option<Sides<u16>> {
        let r#match = &machine.current().phases()[&phase_id()].divisions()[0]
            .data()
            .matches()[&match_id];
        match *r#match.contest().value(&SCORE)? {
            Sides {
                a: Some(EnteredValue::Count(a)),
                b: Some(EnteredValue::Count(b)),
            } => Some(Sides::new(a, b)),
            _ => None,
        }
    }

    /// A scored knockout with both semifinals won 3–1 by side A and the final reported, returning
    /// the semifinals and the final.
    fn final_played() -> (CompetitionStateMachine, [MatchId; 2], MatchId) {
        let mut machine = scored_knockout();
        let [first, second] = matches(&machine)[..] else {
            panic!("a knockout between four entrants starts with two semifinals");
        };
        for semifinal in [first, second] {
            machine.apply_new(scored(semifinal, 3, 1).into()).unwrap();
        }
        let r#final = matches(&machine)[2];
        machine.apply_new(scored(r#final, 3, 0).into()).unwrap();
        (machine, [first, second], r#final)
    }

    /// A semifinal corrected to another score of the same winner leaves the final as it was
    /// played, which reopening the semifinal would not.
    #[test]
    fn a_result_is_replaced_keeping_the_results_that_still_apply() {
        let (mut machine, [first, _], r#final) = final_played();
        let mut history = SessionHistory::new();

        machine
            .replace_match_result(&mut history, scored(first, 3, 2))
            .expect("the same winner keeps the final as it was played");

        assert_eq!(score_of(&machine, first), Some(Sides::new(3, 2)));
        assert_eq!(score_of(&machine, r#final), Some(Sides::new(3, 0)));
        history
            .undo_last(&mut machine)
            .expect("the replacement is an action of the session");
        assert_eq!(score_of(&machine, first), Some(Sides::new(3, 1)));
        history.redo_last(&mut machine).unwrap();
        assert_eq!(score_of(&machine, first), Some(Sides::new(3, 2)));
    }

    /// Undoing a second correction brings back the first, not the result reported.
    #[test]
    fn undoing_a_second_replacement_brings_back_the_first() {
        let (mut machine, [first, _], _) = final_played();
        let mut history = SessionHistory::new();
        machine
            .replace_match_result(&mut history, scored(first, 3, 2))
            .unwrap();
        machine
            .replace_match_result(&mut history, scored(first, 3, 0))
            .unwrap();
        assert_eq!(score_of(&machine, first), Some(Sides::new(3, 0)));

        history.undo_last(&mut machine).unwrap();

        assert_eq!(score_of(&machine, first), Some(Sides::new(3, 2)));
    }

    /// Another winner of a semifinal leaves the final reported between other entrants.
    #[test]
    fn a_replacement_a_later_result_does_not_fit_is_refused() {
        let (mut machine, [first, _], r#final) = final_played();
        let final_reported = machine.reported_by(phase_id(), r#final).unwrap();
        let log_len = machine.log().len();

        let error = machine
            .replace_match_result(&mut SessionHistory::new(), scored(first, 1, 3))
            .expect_err("the final was played by the winner reported");

        assert!(matches!(error, ReplaceMatchResultError::Rejected(_)));
        assert_eq!(machine.log().len(), log_len, "the log must stay unchanged");
        assert_eq!(score_of(&machine, first), Some(Sides::new(3, 1)));
        let target = machine.reported_by(phase_id(), first).unwrap();
        let with = scored(first, 1, 3).into();
        assert_eq!(
            machine
                .conflicts_of(Change::Replace {
                    target,
                    with: &with
                })
                .unwrap(),
            [conflict(final_reported, r#final)]
        );
    }

    #[test]
    fn a_replacement_that_does_not_apply_itself_is_refused() {
        let mut machine = knockout();
        let first = matches(&machine)[0];
        report(&mut machine, first);

        let error = machine
            .replace_match_result(
                &mut SessionHistory::new(),
                ReportMatchResult::new(phase_id(), first, MatchResult::Draw),
            )
            .expect_err("a knockout takes no draw");

        let ReplaceMatchResultError::Refused(error) = error else {
            panic!("the replacement itself does not apply, not a later change: {error:?}");
        };
        assert!(matches!(
            error.downcast_ref(),
            Some(ReportMatchResultError::DrawNotAllowed)
        ));
    }

    #[test]
    fn a_result_entered_as_reported_is_not_replaced() {
        let (mut machine, [first, _], _) = final_played();
        let log_len = machine.log().len();

        let replaced = machine
            .replace_match_result(&mut SessionHistory::new(), scored(first, 3, 1))
            .unwrap();

        assert_eq!(replaced, None);
        assert_eq!(machine.log().len(), log_len);
    }

    #[test]
    fn an_open_match_has_no_result_to_replace() {
        let mut machine = knockout();
        let open = matches(&machine)[0];

        let error = machine
            .replace_match_result(
                &mut SessionHistory::new(),
                ReportMatchResult::new(phase_id(), open, MatchResult::AWins),
            )
            .expect_err("an open match has no result to replace");

        assert!(matches!(error, ReplaceMatchResultError::NotReported));
    }

    /// A reconfigured phase is played as if it had been added that way, so the results of the
    /// matches it schedules as well stay reported.
    #[test]
    fn a_reconfigured_phase_keeps_the_results_of_its_matches() {
        let mut machine = knockout();
        let first = matches(&machine)[0];
        report(&mut machine, first);
        let with_record = PhasePluginsBuilder::new()
            .format(SingleElimination::new())
            .add_metric(WinsDrawsLossesCalculator)
            .build()
            .unwrap();

        machine
            .reconfigure_phase(
                &mut SessionHistory::new(),
                AddPhase::new(phase_id(), with_record.clone()),
            )
            .unwrap()
            .expect("the plugins changed");

        assert_eq!(config(&machine), with_record.config());
        let phase = &machine.current().phases()[&phase_id()];
        assert_eq!(phase.plugins().metrics().len(), 1);
        assert_eq!(result_of(&machine, first), Some(MatchResult::AWins));
        let data = phase.divisions()[0].data();
        let winner = data.matches()[&first].team_a()[0];
        let record = data.rankings()[&winner].metric::<WinsDrawsLosses>();
        assert_eq!(record.map(|record| record.wins), Some(1));
    }

    /// The config of a started phase leaves out the runtime state of its plugins, such as the
    /// bracket of a knockout, so the plugins a draft builds from it are the ones the phase was
    /// added with.
    #[test]
    fn reconfiguring_a_phase_with_its_own_config_changes_nothing() {
        let mut machine = knockout();
        let first = matches(&machine)[0];
        report(&mut machine, first);
        let unchanged = machine
            .current()
            .phase_draft(&[Source::registration()])
            .with_answers(&config(&machine))
            .unwrap()
            .build()
            .unwrap();
        let log_len = machine.log().len();

        assert!(matches!(
            machine.reconfigure_phase(
                &mut SessionHistory::new(),
                AddPhase::new(phase_id(), unchanged)
            ),
            Ok(None)
        ));
        assert_eq!(machine.log().len(), log_len, "the log must stay unchanged");
    }

    #[test]
    fn a_phase_that_was_not_added_cannot_be_reconfigured() {
        let mut machine = knockout();
        let unknown = PhaseId::new(Uuid::from_u128(2));

        assert!(matches!(
            machine.reconfigure_phase(
                &mut SessionHistory::new(),
                AddPhase::new(unknown, PhasePluginsBuilder::new().build().unwrap())
            ),
            Err(ReconfigurePhaseError::NotAdded)
        ));
    }

    /// The teams of a Monster DYP are pairs, so it never schedules the match a knockout of single
    /// entrants reported a result for.
    #[test]
    fn a_reconfiguration_a_reported_result_does_not_fit_is_rejected() {
        let mut machine = knockout();
        let first = matches(&machine)[0];
        report(&mut machine, first);
        let log_len = machine.log().len();

        let monster_dyp = PhasePluginsBuilder::new()
            .format(MonsterDyp::new(1))
            .build()
            .unwrap();

        let result = machine.reconfigure_phase(
            &mut SessionHistory::new(),
            AddPhase::new(phase_id(), monster_dyp),
        );

        assert!(matches!(result, Err(ReconfigurePhaseError::Rejected(_))));
        assert_eq!(machine.log().len(), log_len, "the log must stay unchanged");
    }

    #[test]
    fn a_reconfiguration_is_undone_and_redone_like_any_action() {
        let mut machine = knockout();
        let knockout = config(&machine);
        let monster_dyp = PhasePluginsBuilder::new()
            .format(MonsterDyp::new(1))
            .build()
            .unwrap();
        let mut history = SessionHistory::new();
        machine
            .reconfigure_phase(&mut history, AddPhase::new(phase_id(), monster_dyp.clone()))
            .unwrap();

        history
            .undo_last(&mut machine)
            .expect("the reconfiguration is in effect");
        assert_eq!(config(&machine), knockout);
        history
            .redo_last(&mut machine)
            .expect("the reconfiguration was undone last");

        assert_eq!(config(&machine), monster_dyp.config());
    }

    /// Undoing a reconfiguration changes the phase every change after it applies to, so it is
    /// rejected if a change another session made since does not apply to the phase as it was
    /// configured before.
    #[test]
    fn undoing_a_reconfiguration_a_result_of_another_session_depends_on_is_rejected() {
        let mut machine = knockout();
        let mut history = SessionHistory::new();
        let monster_dyp = PhasePluginsBuilder::new()
            .format(MonsterDyp::new(1))
            .build()
            .unwrap();
        machine
            .reconfigure_phase(&mut history, AddPhase::new(phase_id(), monster_dyp))
            .unwrap();
        // the teams of a Monster DYP are pairs, so the knockout never schedules this match
        let first = matches(&machine)[0];
        report(&mut machine, first);
        let log_len = machine.log().len();

        let result = history.undo_last(&mut machine);

        assert!(matches!(
            result,
            Err(undoable_state_machine::UndoLastError::Rejected(_))
        ));
        assert_eq!(machine.log().len(), log_len, "the log must stay unchanged");
    }

    /// The final is scheduled once both semifinals are reported, but undoing one of them takes
    /// back no result of the final, which is not reported yet.
    #[test]
    fn undoing_a_result_no_reported_result_depends_on_conflicts_with_nothing() {
        let mut machine = knockout();
        let semifinals = matches(&machine);
        let first = report(&mut machine, semifinals[0]);
        report(&mut machine, semifinals[1]);

        assert_eq!(machine.conflicts_of(Change::Undo(first)).unwrap(), []);
        machine
            .undo_match_result(&mut SessionHistory::new(), phase_id(), semifinals[0])
            .expect("no reported result depends on the semifinal");
    }

    #[test]
    fn undoing_a_semifinal_conflicts_with_the_final_reported() {
        let mut machine = knockout();
        let semifinals = matches(&machine);
        let first = report(&mut machine, semifinals[0]);
        report(&mut machine, semifinals[1]);
        let r#final = matches(&machine)[2];
        let final_reported = report(&mut machine, r#final);
        let log_len = machine.log().len();

        let conflicts = machine.conflicts_of(Change::Undo(first)).unwrap();

        assert_eq!(conflicts, [conflict(final_reported, r#final)]);
        assert_eq!(machine.log().len(), log_len, "the log must stay unchanged");
        assert_eq!(result_of(&machine, semifinals[0]), Some(MatchResult::AWins));
        assert!(matches!(
            machine.undo_match_result(&mut SessionHistory::new(), phase_id(), semifinals[0]),
            Err(UndoMatchResultError::Rejected(_))
        ));
    }

    /// Without a quarterfinal, the semifinal its winner plays is not scheduled, and so neither is
    /// the final, which the winner of that semifinal plays; the other semifinal is played anyway.
    #[test]
    fn the_conflicts_of_an_undo_are_every_result_depending_on_it_in_turn() {
        let mut machine = knockout_of(8);
        let reports = play_out(&mut machine);
        let quarterfinal = matches(&machine)[0];
        let data = machine.current().phases()[&phase_id()].divisions()[0].data();
        let winner = data.matches()[&quarterfinal].team_a()[0];
        let later = |round: u32| {
            data.matches()
                .values()
                .filter(move |r#match| r#match.round().get() == round)
        };
        let [semifinal] = later(2)
            .filter(|r#match| {
                r#match.team_a().contains(&winner) || r#match.team_b().contains(&winner)
            })
            .map(Match::id)
            .collect::<Vec<_>>()[..]
        else {
            panic!("the winner of the quarterfinal plays one semifinal");
        };
        let [r#final] = later(3).map(Match::id).collect::<Vec<_>>()[..] else {
            panic!("a knockout without a match for third place has one final");
        };

        let conflicts = machine
            .conflicts_of(Change::Undo(reports[&quarterfinal]))
            .unwrap();

        assert_eq!(
            conflicts,
            [
                conflict(reports[&semifinal], semifinal),
                conflict(reports[&r#final], r#final)
            ]
        );
    }

    /// Only the report in effect for a match is listed, not one undone before the match was
    /// reported again.
    #[test]
    fn a_result_reported_again_conflicts_by_the_report_in_effect() {
        let mut machine = knockout();
        let semifinals = matches(&machine);
        let first = report(&mut machine, semifinals[0]);
        report(&mut machine, semifinals[1]);
        let r#final = matches(&machine)[2];
        let reported = report(&mut machine, r#final);
        undo(&mut machine, reported);
        let reported_again = report(&mut machine, r#final);

        let conflicts = machine.conflicts_of(Change::Undo(first)).unwrap();

        assert_eq!(conflicts, [conflict(reported_again, r#final)]);
    }

    /// The teams of a Monster DYP are pairs, so it schedules none of the matches of the knockout
    /// whose results were reported.
    #[test]
    fn a_replaced_phase_conflicts_with_the_results_of_matches_it_does_not_schedule() {
        let mut machine = knockout();
        let reports = play_out(&mut machine);
        let monster_dyp = AddPhase::new(
            phase_id(),
            PhasePluginsBuilder::new()
                .format(MonsterDyp::new(1))
                .build()
                .unwrap(),
        );
        let target = machine.added_by(phase_id()).unwrap();

        let conflicts = machine
            .conflicts_of(Change::Replace {
                target,
                with: &monster_dyp.into(),
            })
            .unwrap();

        let reported: Vec<_> = matches(&machine)
            .into_iter()
            .map(|r#match| conflict(reports[&r#match], r#match))
            .collect();
        assert_eq!(conflicts, reported);
    }

    #[test]
    fn only_a_change_of_an_entry_in_effect_has_conflicts() {
        let mut machine = knockout();
        let first = matches(&machine)[0];
        let reported = report(&mut machine, first);
        let undone = undo(&mut machine, reported);
        let unchanged = AddPhase::new(phase_id(), PhasePluginsBuilder::new().build().unwrap());

        for change in [
            Change::Undo(reported),
            Change::Undo(undone),
            Change::Undo(TransitionId::new()),
            Change::Replace {
                target: reported,
                with: &unchanged.into(),
            },
        ] {
            assert!(
                matches!(
                    machine.conflicts_of(change),
                    Err(ConflictsError::NotInEffect)
                ),
                "{change:?}"
            );
        }
    }
}
