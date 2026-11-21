//! Reporting results and taking them back.

use tournament_engine::{
    CompetitionStateMachineExt as _, CompetitionTransition, Division, Match, MatchId, MatchResult,
    Part, PhaseId, ReportMatchResult,
    plugins::match_format::Template,
    transitions::{Change, ConflictsError, ReplaceMatchResultError, UndoMatchResultError},
};

use super::{Session, refused};
use crate::{
    error::{EditReportError, Refusal, ReportError, UndoReportError},
    result_form::{ResultForm, ResultView},
    view::{Edited, Reopened, Reported},
};

impl Session {
    /// Reports `result` for the match `id` of the phase `phase`, with nothing else entered.
    ///
    /// # Errors
    /// Returns an error if the phase has not started, has no such match, the match has a result
    /// already, or its format does not take the result.
    pub fn report(
        &mut self,
        phase: PhaseId,
        id: MatchId,
        result: MatchResult,
    ) -> Result<Reported, ReportError> {
        self.report_contest(phase, id, Part::new().with_decision(result))
    }

    /// A form to enter the result of the match `id` of the phase `phase` into, see [`ResultForm`],
    /// with nothing entered.
    ///
    /// # Errors
    /// Returns an error if the competition has not started, has no such match, or the match has a
    /// result already.
    pub fn result_form(&self, phase: PhaseId, id: MatchId) -> Result<ResultForm, ReportError> {
        let (division, r#match) = self.open_match(phase, id)?;
        let format = division.match_format();
        let template = format.template(division.data(), r#match);
        let form = ResultForm::new(phase, id, dyn_clone::clone_box(format), template);
        Ok(form.in_language(self.language))
    }

    /// The [template](ResultForm::template) of the [form](Self::result_form) of the match `id` of
    /// the phase `phase`, without making the form.
    ///
    /// # Errors
    /// Returns an error as [`Self::result_form`] does.
    pub fn result_template(&self, phase: PhaseId, id: MatchId) -> Result<Template, ReportError> {
        let (division, r#match) = self.open_match(phase, id)?;
        Ok(division.match_format().template(division.data(), r#match))
    }

    /// The result of the finished match `id` of the phase `phase` as it was reported, to be
    /// shown, not changed: as a [`ResultForm`] shows it, a result reported against the values that
    /// suggest another one shown as chosen, with its warning. `None` if there is no such match or
    /// it is not finished.
    pub fn reported_result(&self, phase: PhaseId, id: MatchId) -> Option<ResultView> {
        let (_, division, r#match) = self.match_of(phase, id)?;
        if !r#match.is_finished() {
            return None;
        }
        let format = division.match_format();
        let template = format.template(division.data(), r#match);
        let form = ResultForm::reported(
            phase,
            id,
            dyn_clone::clone_box(format),
            template,
            r#match.contest(),
        );
        Some(form.view())
    }

    /// Reports the result entered into `form`.
    ///
    /// # Errors
    /// Returns an error as [`Self::report`] does, if anything typed into the form was not taken
    /// ([`ResultForm::enter`]), and if the result neither says nor suggests who won.
    pub fn report_form(&mut self, form: &ResultForm) -> Result<Reported, ReportError> {
        if form.has_rejected() {
            return Err(ReportError::NotTaken);
        }
        self.report_contest(form.phase(), form.match_id(), form.report())
    }

    /// Reports `contest` as what was entered for the match `id` of the phase `phase`, see
    /// [`Self::report_form`].
    fn report_contest(
        &mut self,
        phase: PhaseId,
        id: MatchId,
        contest: Part,
    ) -> Result<Reported, ReportError> {
        let started = self.phase(phase).is_some_and(|phase| phase.is_started());
        if !started {
            return Err(ReportError::NotStarted);
        }
        let (index, division, _) = self.match_of(phase, id).ok_or(ReportError::UnknownMatch)?;
        let matches_before = division.data().matches().len();
        let report = ReportMatchResult::of(phase, id, contest).ok_or(ReportError::Undecided)?;
        self.history
            .apply_new(&mut self.machine, report.into())
            .map_err(|error| {
                refused(error, ReportError::Rejected, |refusal| match refusal {
                    Refusal::MatchFinished => Some(ReportError::AlreadyFinished),
                    Refusal::DrawNotAllowed => Some(ReportError::DrawNotAllowed),
                    _ => None,
                })
            })?;
        let division = &self.phase(phase).expect("still added").divisions()[index];
        let matches = division.data().matches();
        let row = self.match_row(phase, index, division, &matches[&id]);
        Ok(Reported {
            row,
            new_matches: matches.len() - matches_before,
            finished_phase: self.phase(phase).is_some_and(|phase| phase.is_finished()),
            winners: self.competition_winners(),
        })
    }

    /// Undoes the result reported for the match `id` of the phase `phase`, which may be any
    /// reported match rather than only the one reported last. Every other change stays in effect.
    /// The form of the match starts out empty, see [`Self::result_form`]; to correct the result
    /// instead, see [`Self::edit_report`].
    ///
    /// # Errors
    /// Returns an error if the phase has not started, has no such match, the match has no result,
    /// later results depend on it ([`UndoReportError::DependedOnBy`], naming every one), or a
    /// phase taking entrants from its phase has started ([`UndoReportError::PhaseStarted`]).
    pub fn undo_report(
        &mut self,
        phase: PhaseId,
        id: MatchId,
    ) -> Result<Reopened, UndoReportError> {
        let started = self.phase(phase).is_some_and(|phase| phase.is_started());
        if !started {
            return Err(UndoReportError::NotStarted);
        }
        let Some((index, division, r#match)) = self.match_of(phase, id) else {
            return Err(UndoReportError::UnknownMatch);
        };
        let row = self.match_row(phase, index, division, r#match);
        let matches_before = division.data().matches().len();
        self.machine
            .undo_match_result(&mut self.history, phase, id)
            .map_err(|error| match error {
                UndoMatchResultError::NotReported => UndoReportError::NotReported,
                UndoMatchResultError::Rejected(_) => self.undo_refusal(phase, id),
            })?;
        let matches = self.phase(phase).expect("still added").divisions()[index]
            .data()
            .matches();
        Ok(Reopened {
            row,
            unscheduled: matches_before - matches.len(),
        })
    }

    /// Why undoing the result of the match `match_id` of the phase `phase_id` was rejected: the
    /// results that depend on it, if they can be told.
    fn undo_refusal(&self, phase_id: PhaseId, match_id: MatchId) -> UndoReportError {
        let conflicts = self
            .machine
            .reported_by(phase_id, match_id)
            .map(|report| self.conflicting_numbers(Change::Undo(report)));
        match conflicts {
            Some(Ok(numbers)) if !numbers.is_empty() => UndoReportError::DependedOnBy(numbers),
            Some(Err(ConflictsError::Other(entry, _))) => self
                .started_by(entry)
                .map_or(UndoReportError::DependedOn, UndoReportError::PhaseStarted),
            _ => UndoReportError::DependedOn,
        }
    }

    /// A form to correct the result of the finished match `id` of the phase `phase` in, starting
    /// out at the result as it was reported, to be handed to [`Self::edit_report`].
    ///
    /// # Errors
    /// Returns an error if the phase has not started, has no such match, or the match has no
    /// result.
    pub fn edit_form(&self, phase: PhaseId, id: MatchId) -> Result<ResultForm, EditReportError> {
        let (division, r#match) = self.finished_match(phase, id)?;
        let format = division.match_format();
        let template = format.template(division.data(), r#match);
        let form = ResultForm::reported(
            phase,
            id,
            dyn_clone::clone_box(format),
            template,
            r#match.contest(),
        );
        Ok(form.in_language(self.language))
    }

    /// Replaces the result of the finished match of `form`, see [`Self::edit_form`], by the one
    /// entered into it, as one action: undoing it brings the result before back. Every later
    /// result stays as long as it still applies, e.g. with the winner of the match unchanged. A
    /// result entered as it was reported changes nothing, and is no action.
    ///
    /// # Errors
    /// Returns an error if the phase has not started, has no such match, the match has no result,
    /// anything typed into the form was not taken ([`ResultForm::enter`]), the result neither says
    /// nor suggests who won or is a draw the format does not take, later results no longer apply
    /// with it ([`EditReportError::DoesNotFit`], naming every one), or a phase taking entrants
    /// from its phase has started and would no longer take the same
    /// ([`EditReportError::PhaseStarted`]).
    pub fn edit_report(&mut self, form: &ResultForm) -> Result<Edited, EditReportError> {
        let (phase, id) = (form.phase(), form.match_id());
        self.finished_match(phase, id)?;
        if form.has_rejected() {
            return Err(EditReportError::NotTaken);
        }
        let report = || ReportMatchResult::of(phase, id, form.report());
        let replacement = report().ok_or(EditReportError::Undecided)?;
        self.machine
            .replace_match_result(&mut self.history, replacement)
            .map_err(|error| match error {
                ReplaceMatchResultError::NotReported => EditReportError::NotReported,
                ReplaceMatchResultError::Refused(error) => match Refusal::of(&error) {
                    Some(Refusal::DrawNotAllowed) => EditReportError::DrawNotAllowed,
                    _ => EditReportError::Rejected(error),
                },
                ReplaceMatchResultError::Rejected(_) => {
                    let with: CompetitionTransition = report().expect("decided").into();
                    let conflicts = self.machine.reported_by(phase, id).map(|target| {
                        self.conflicting_numbers(Change::Replace {
                            target,
                            with: &with,
                        })
                    });
                    match conflicts {
                        Some(Ok(numbers)) if !numbers.is_empty() => {
                            EditReportError::DoesNotFit(numbers)
                        }
                        Some(Err(ConflictsError::Other(entry, _))) => self
                            .started_by(entry)
                            .map_or(EditReportError::DependedOn, EditReportError::PhaseStarted),
                        _ => EditReportError::DependedOn,
                    }
                }
            })?;
        let (index, division, r#match) = self.match_of(phase, id).expect("still scheduled");
        Ok(Edited {
            row: self.match_row(phase, index, division, r#match),
        })
    }

    /// The division of the match `id` of the started phase `phase`, and the match, which has a
    /// result.
    fn finished_match(
        &self,
        phase: PhaseId,
        id: MatchId,
    ) -> Result<(&Division, &Match), EditReportError> {
        let started = self.phase(phase).is_some_and(|phase| phase.is_started());
        if !started {
            return Err(EditReportError::NotStarted);
        }
        let (_, division, r#match) = self
            .match_of(phase, id)
            .ok_or(EditReportError::UnknownMatch)?;
        if !r#match.is_finished() {
            return Err(EditReportError::NotReported);
        }
        Ok((division, r#match))
    }

    /// The division of the match `id` of the started phase `phase`, and the match, which has no
    /// result yet.
    fn open_match(&self, phase: PhaseId, id: MatchId) -> Result<(&Division, &Match), ReportError> {
        let started = self.phase(phase).is_some_and(|phase| phase.is_started());
        if !started {
            return Err(ReportError::NotStarted);
        }
        let (_, division, r#match) = self.match_of(phase, id).ok_or(ReportError::UnknownMatch)?;
        if r#match.is_finished() {
            return Err(ReportError::AlreadyFinished);
        }
        Ok((division, r#match))
    }
}
