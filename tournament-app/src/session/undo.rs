//! Undoing and redoing the actions of the session.

use tournament_engine::{
    CompetitionTransition, Phase, ReportMatchResult,
    undoable_state_machine::{LogEntry, LogEntryKind, RedoLastError, TransitionId, UndoLastError},
};

use super::Session;
use crate::{
    error::{RedoError, UndoError},
    view::{Action, MatchRow},
};

impl Session {
    /// The action [`Self::undo`] would undo now, if any, e.g. so that undoing the start, which
    /// leaves play, is asked about first. `None` if there is nothing to undo, or another session
    /// has undone the action already.
    pub fn next_undo(&self) -> Option<Action> {
        let id = self.history.next_undo(&self.machine)?;
        Some(self.action_of(id))
    }

    /// Undoes the latest action of this session that is still in effect, which may be an
    /// [`Self::undo_report`], bringing the result back. Returns the action undone. The form of a
    /// match whose report it undoes starts out at the result, see [`Self::result_form`].
    ///
    /// The changes of other sessions, caught up with, are not undone, and stay in effect.
    ///
    /// # Errors
    /// Returns [`UndoError::NothingToUndo`] if every action of the session is undone, and
    /// [`UndoError::ChangedElsewhere`] if a change of another session undid the action or does not
    /// apply without it; then nothing more can be undone, while what can be redone stays.
    pub fn undo(&mut self) -> Result<Action, UndoError> {
        let Some(id) = self.history.next_undo(&self.machine) else {
            // nothing to undo, or undone elsewhere, which the history fails with and ends at
            return Err(match self.history.undo_last(&mut self.machine) {
                Err(UndoLastError::NothingToUndo) => UndoError::NothingToUndo,
                _ => UndoError::ChangedElsewhere,
            });
        };
        // read before the undo, which removes the phase, or the result
        let phase = match self.applied(id) {
            Some(CompetitionTransition::AddPhase(_)) => self.phase().map(Phase::config),
            _ => None,
        };
        let reported = match self.applied(id) {
            Some(CompetitionTransition::ReportMatchResult(report)) => {
                self.reported_contest(report.match_id())
            }
            _ => None,
        };
        self.history
            .undo_last(&mut self.machine)
            .map_err(|_| UndoError::ChangedElsewhere)?;
        if phase.is_some() {
            self.undone_phase = phase;
        }
        self.reopened.extend(reported);
        Ok(self.action_of(id))
    }

    /// Redoes the action this session undid last, as long as it took no other action since.
    /// Returns the action redone. The form of a match it reopens again starts out at the result,
    /// see [`Self::result_form`].
    ///
    /// # Errors
    /// Returns [`RedoError::NothingToRedo`] if there is no such action, and
    /// [`RedoError::ChangedElsewhere`] if a change of another session redid the action or does not
    /// apply with it; then nothing more can be redone, while what can be undone stays.
    pub fn redo(&mut self) -> Result<Action, RedoError> {
        let Some(id) = self.history.next_redo(&self.machine) else {
            // nothing to redo, or redone elsewhere, which the history fails with and ends at
            return Err(match self.history.redo_last(&mut self.machine) {
                Err(RedoLastError::NothingToRedo) => RedoError::NothingToRedo,
                _ => RedoError::ChangedElsewhere,
            });
        };
        // read before the redo, which takes the result back
        let reported = match self.machine.entry(id).map(LogEntry::kind) {
            Some(LogEntryKind::Undo(target)) => match self.applied(*target) {
                Some(CompetitionTransition::ReportMatchResult(report)) => {
                    self.reported_contest(report.match_id())
                }
                _ => None,
            },
            _ => None,
        };
        self.history
            .redo_last(&mut self.machine)
            .map_err(|_| RedoError::ChangedElsewhere)?;
        self.reopened.extend(reported);
        Ok(self.action_of(id))
    }

    /// The transition the log entry `id` applies, if it applies one.
    fn applied(&self, id: TransitionId) -> Option<&CompetitionTransition> {
        match self.machine.entry(id)?.kind() {
            LogEntryKind::Apply(transition) => Some(transition),
            _ => None,
        }
    }

    /// The action of this session whose entry in the log is `id`, as the competition shows it
    /// now: done, or about to be undone or redone, or just undone or redone.
    ///
    /// # Panics
    /// Panics if `id` is no action of the session, or the match of a report is not scheduled, which
    /// it is while the report is in effect, and after it was undone or before it is redone, since
    /// only the entries before a report schedule its match.
    fn action_of(&self, id: TransitionId) -> Action {
        let entry = self
            .machine
            .entry(id)
            .expect("an action is an entry of the log");
        let reported = |report: &ReportMatchResult| {
            let phase = self
                .started_phase()
                .expect("a match is reported once started");
            let r#match = &phase.data().matches()[&report.match_id()];
            MatchRow {
                result: Some(report.match_result()),
                ..self.match_row(phase, r#match)
            }
        };
        match entry.kind() {
            LogEntryKind::Apply(transition) => match transition {
                CompetitionTransition::AddEntrant(added) => {
                    Action::AddEntrant(self.entrant_row(&added.entrant().id()))
                }
                CompetitionTransition::RemoveEntrant(removed) => {
                    Action::RemoveEntrant(self.entrant_row(&removed.entrant_id()))
                }
                CompetitionTransition::AddPhase(_) => Action::AddPhase,
                CompetitionTransition::StartCompetition(_) => Action::Start,
                CompetitionTransition::ReportMatchResult(report) => {
                    Action::Report(reported(report))
                }
                CompetitionTransition::SetCompetitionSeed(_) => {
                    unreachable!("the seed is applied outside the history")
                }
            },
            LogEntryKind::Replace { .. } => Action::EditPhase,
            LogEntryKind::Undo(target) => match self.applied(*target) {
                Some(CompetitionTransition::ReportMatchResult(report)) => {
                    Action::Reopen(reported(report))
                }
                _ => unreachable!("the only undo a session takes as an action reopens a match"),
            },
        }
    }
}
