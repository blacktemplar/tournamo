//! Reporting results and taking them back.

use tournament_engine::{
    CompetitionStateMachineExt as _, Division, Match, MatchId, MatchResult, Part, PhaseId,
    ReportMatchResult,
    plugins::match_format::Template,
    transitions::{Change, ConflictsError, UndoMatchResultError},
};

use super::{Session, refused};
use crate::{
    error::{Refusal, ReportError, UndoReportError},
    result_form::{ResultForm, ResultView},
    view::{Reopened, Reported},
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

    /// A form to enter the result of the match `id` of the phase `phase` into, see [`ResultForm`].
    /// If this session took
    /// back the result of the match, by reopening it ([`Self::undo_report`]), undoing its report
    /// or redoing a reopening, the form starts out at that result, the one taken back last, so
    /// that correcting a value takes changing only that one, as long as the match format still
    /// takes it; else nothing is entered.
    ///
    /// # Errors
    /// Returns an error if the competition has not started, has no such match, or the match has a
    /// result already.
    pub fn result_form(&self, phase: PhaseId, id: MatchId) -> Result<ResultForm, ReportError> {
        let (division, r#match) = self.open_match(phase, id)?;
        let format = division.match_format();
        let template = format.template(division.data(), r#match);
        // the match format may have changed since, e.g. with the phase edited
        let reopened = self
            .reopened
            .get(&(phase, id))
            .filter(|contest| template.root.check(contest).is_ok());
        let format = dyn_clone::clone_box(format);
        Ok(match reopened {
            Some(contest) => ResultForm::reported(phase, id, format, template, contest),
            None => ResultForm::new(phase, id, format, template),
        })
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
    /// The form of the match starts out at the result, see [`Self::result_form`].
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
        let contest = r#match.contest().clone();
        let matches_before = division.data().matches().len();
        self.machine
            .undo_match_result(&mut self.history, phase, id)
            .map_err(|error| match error {
                UndoMatchResultError::NotReported => UndoReportError::NotReported,
                UndoMatchResultError::Rejected(_) => self.undo_refusal(phase, id),
            })?;
        self.reopened.insert((phase, id), contest);
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

    /// The match `id` of the phase `phase` with what was reported for it, if it has a result,
    /// which undoing its report takes back.
    pub(super) fn reported_contest(
        &self,
        phase: PhaseId,
        id: MatchId,
    ) -> Option<((PhaseId, MatchId), Part)> {
        let (_, _, r#match) = self.match_of(phase, id)?;
        r#match
            .is_finished()
            .then(|| ((phase, id), r#match.contest().clone()))
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
