//! Undoing and redoing the actions of the session.

use tournament_engine::{
    CompetitionTransition, ReportMatchResult,
    undoable_state_machine::{LogEntryKind, RedoLastError, TransitionId, UndoLastError},
};

use super::{Session, UndonePhase};
use crate::{
    error::{RedoError, UndoError},
    view::{self, Action, MatchRow},
};

impl Session {
    /// The action [`Self::undo`] would undo now, if any, e.g. so that undoing the start, which
    /// leaves play, is asked about first. `None` if there is nothing to undo, or another session
    /// has undone the action already.
    pub fn next_undo(&self) -> Option<Action> {
        let ids = self.history.next_undo(&self.machine)?;
        Some(self.action_of(ids))
    }

    /// Undoes the latest action of this session that is still in effect, which may be an
    /// [`Self::undo_report`], bringing the result back, or every entrant of a list added at once.
    /// Returns the action undone. The form of a match whose report it undoes starts out at the
    /// result, see [`Self::result_form`].
    ///
    /// The changes of other sessions, caught up with, are not undone, and stay in effect.
    ///
    /// # Errors
    /// Returns [`UndoError::NothingToUndo`] if every action of the session is undone, and
    /// [`UndoError::ChangedElsewhere`] if a change of another session undid the action or does not
    /// apply without it; then nothing more can be undone, while what can be redone stays.
    pub fn undo(&mut self) -> Result<Action, UndoError> {
        let Some(ids) = self.history.next_undo(&self.machine).map(<[_]>::to_vec) else {
            // nothing to undo, or undone elsewhere, which the history fails with and ends at
            return Err(match self.history.undo_last(&mut self.machine) {
                Err(UndoLastError::NothingToUndo) => UndoError::NothingToUndo,
                _ => UndoError::ChangedElsewhere,
            });
        };
        // read before the undo, which removes the phase, or the result
        let phase = match ids.as_slice() {
            [id] => match self.applied(*id) {
                Some(CompetitionTransition::AddPhase(added)) => {
                    self.phase(added.id()).map(|phase| UndonePhase {
                        name: phase.name().map(str::to_owned),
                        sources: phase.sources().clone(),
                        config: phase.config(),
                        columns: phase.columns().clone(),
                    })
                }
                _ => None,
            },
            _ => None,
        };
        let reported: Vec<_> = ids
            .iter()
            .filter_map(|id| match self.applied(*id) {
                Some(CompetitionTransition::ReportMatchResult(report)) => {
                    self.reported_contest(report.phase_id(), report.match_id())
                }
                _ => None,
            })
            .collect();
        self.history
            .undo_last(&mut self.machine)
            .map_err(|_| UndoError::ChangedElsewhere)?;
        if phase.is_some() {
            self.undone_phase = phase;
        }
        self.undone_reports.extend(reported);
        Ok(self.action_of(&ids))
    }

    /// Redoes the action this session undid last, as long as it took no other action since.
    /// Returns the action redone.
    ///
    /// # Errors
    /// Returns [`RedoError::NothingToRedo`] if there is no such action, and
    /// [`RedoError::ChangedElsewhere`] if a change of another session redid the action or does not
    /// apply with it; then nothing more can be redone, while what can be undone stays.
    pub fn redo(&mut self) -> Result<Action, RedoError> {
        let Some(ids) = self.history.next_redo(&self.machine).map(<[_]>::to_vec) else {
            // nothing to redo, or redone elsewhere, which the history fails with and ends at
            return Err(match self.history.redo_last(&mut self.machine) {
                Err(RedoLastError::NothingToRedo) => RedoError::NothingToRedo,
                _ => RedoError::ChangedElsewhere,
            });
        };
        self.history
            .redo_last(&mut self.machine)
            .map_err(|_| RedoError::ChangedElsewhere)?;
        Ok(self.action_of(&ids))
    }

    /// The transition the log entry `id` applies, if it applies one.
    fn applied(&self, id: TransitionId) -> Option<&CompetitionTransition> {
        match self.machine.entry(id)?.kind() {
            LogEntryKind::Apply(transition) => Some(transition),
            _ => None,
        }
    }

    /// The action of this session whose entries in the log are `ids`, oldest first, as the
    /// competition shows it now: done, or about to be undone or redone, or just undone or redone.
    ///
    /// # Panics
    /// Panics if `ids` are no action of the session, or as [`Self::action_of_entry`] does.
    fn action_of(&self, ids: &[TransitionId]) -> Action {
        match ids {
            [id] => self.action_of_entry(*id),
            _ => {
                let actions = ids.iter().map(|id| self.action_of_entry(*id));
                let mut entrants = Vec::new();
                let mut phases = Vec::new();
                let mut edited = Vec::new();
                for action in actions {
                    match action {
                        Action::AddEntrant(row) => entrants.push(row),
                        Action::AddPhase(name) => phases.push(name),
                        Action::EditPhase(name) => edited.push(name),
                        _ => unreachable!(
                            "the only actions of several entries add entrants or phases, or edit \
                             phases"
                        ),
                    }
                }
                if !edited.is_empty() {
                    Action::EditPhases(edited)
                } else if phases.is_empty() {
                    Action::AddEntrants(entrants)
                } else {
                    Action::AddPhases(phases)
                }
            }
        }
    }

    /// The action the entry `id` of an action of this session would be on its own, as
    /// [`Self::action_of`] shows it.
    ///
    /// # Panics
    /// Panics if `id` is no entry of an action of the session, or the match of a report is not
    /// scheduled, which it is while the report is in effect, and after it was undone or before it
    /// is redone, since only the entries before a report schedule its match.
    fn action_of_entry(&self, id: TransitionId) -> Action {
        let entry = self
            .machine
            .entry(id)
            .expect("an action is an entry of the log");
        let reported = |report: &ReportMatchResult| {
            let (index, division, r#match) = self
                .match_of(report.phase_id(), report.match_id())
                .expect("the match of a report is scheduled");
            // as reported, whatever the match has now
            let root = division
                .match_format()
                .template(division.data(), r#match)
                .root;
            MatchRow {
                result: Some(report.match_result()),
                scores: view::scores_of(&root, report.contest()),
                parts_won: view::parts_won_of(&root, report.contest()),
                ..self.match_row(report.phase_id(), index, division, r#match)
            }
        };
        let started = |phase| self.name_of_phase(phase).expect("a phase started is added");
        match entry.kind() {
            LogEntryKind::Apply(transition) => match transition {
                CompetitionTransition::AddEntrant(added) => {
                    Action::AddEntrant(self.entrant_row(&added.entrant().id()))
                }
                CompetitionTransition::RemoveEntrant(removed) => {
                    Action::RemoveEntrant(self.entrant_row(&removed.entrant_id()))
                }
                CompetitionTransition::AddPhase(added) => {
                    Action::AddPhase(self.added_phase_name(id, added))
                }
                CompetitionTransition::StartCompetition(_) => Action::Start,
                CompetitionTransition::StartPhase(start) => {
                    Action::StartPhase(started(start.phase_id()))
                }
                CompetitionTransition::ReportMatchResult(report) => {
                    Action::Report(reported(report))
                }
                CompetitionTransition::SetCompetitionSeed(_) => {
                    unreachable!("the seed is applied outside the history")
                }
            },
            LogEntryKind::Replace { target, with } => match with {
                CompetitionTransition::AddPhase(added) => {
                    Action::EditPhase(self.added_phase_name(*target, added))
                }
                CompetitionTransition::ReportMatchResult(report) => {
                    Action::EditResult(reported(report))
                }
                _ => unreachable!("the only replacements a session makes edit a phase or a result"),
            },
            LogEntryKind::Undo(target) => match self.applied(*target) {
                Some(CompetitionTransition::ReportMatchResult(report)) => {
                    Action::Reopen(reported(report))
                }
                Some(CompetitionTransition::AddPhase(added)) => {
                    Action::RemovePhase(self.added_phase_name(*target, added))
                }
                Some(CompetitionTransition::StartCompetition(_)) => Action::Unstart,
                Some(CompetitionTransition::StartPhase(start)) => {
                    Action::UnstartPhase(started(start.phase_id()))
                }
                _ => unreachable!(
                    "the only undo a session takes as an action reopens a match, removes a \
                     phase or undoes a start"
                ),
            },
        }
    }
}
