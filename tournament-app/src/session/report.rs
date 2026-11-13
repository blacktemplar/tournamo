//! Reporting results and taking them back.

use tournament_engine::{
    CompetitionStateMachineExt as _, Match, MatchId, MatchResult, Part, Phase, PhaseId,
    ReportMatchResult,
    plugins::match_format::Template,
    transitions::{self as engine, Change, UndoMatchResultError},
};

use super::{Session, rejection};
use crate::{
    error::{ReportError, UndoReportError},
    result_form::{ResultForm, ResultView},
    view::{Reopened, Reported},
};

impl Session {
    /// Reports `result` for the match `id`, with nothing else entered.
    ///
    /// # Errors
    /// Returns an error if the competition has not started, has no such match, the match has a
    /// result already, or its format does not take the result.
    pub fn report(&mut self, id: MatchId, result: MatchResult) -> Result<Reported, ReportError> {
        self.report_contest(id, Part::new().with_decision(result))
    }

    /// A form to enter the result of the match `id` into, see [`ResultForm`]. If this session took
    /// back the result of the match, by reopening it ([`Self::undo_report`]), undoing its report
    /// or redoing a reopening, the form starts out at that result, the one taken back last, so
    /// that correcting a value takes changing only that one, as long as the match format still
    /// takes it; else nothing is entered.
    ///
    /// # Errors
    /// Returns an error if the competition has not started, has no such match, or the match has a
    /// result already.
    pub fn result_form(&self, id: MatchId) -> Result<ResultForm, ReportError> {
        let (phase, r#match) = self.open_match(id)?;
        let format = phase.match_format();
        let template = format.template(phase.data(), r#match);
        // the match format may have changed since, e.g. with the phase edited
        let reopened = self
            .reopened
            .get(&id)
            .filter(|contest| template.root.check(contest).is_ok());
        let format = dyn_clone::clone_box(format);
        Ok(match reopened {
            Some(contest) => ResultForm::reported(id, format, template, contest),
            None => ResultForm::new(id, format, template),
        })
    }

    /// The [template](ResultForm::template) of the [form](Self::result_form) of the match `id`,
    /// without making the form.
    ///
    /// # Errors
    /// Returns an error as [`Self::result_form`] does.
    pub fn result_template(&self, id: MatchId) -> Result<Template, ReportError> {
        let (phase, r#match) = self.open_match(id)?;
        Ok(phase.match_format().template(phase.data(), r#match))
    }

    /// The result of the finished match `id` as it was reported, to be shown, not changed: as a
    /// [`ResultForm`] shows it, a result reported against the values that suggest another one
    /// shown as chosen, with its warning. `None` if there is no such match or it is not finished.
    pub fn reported_result(&self, id: MatchId) -> Option<ResultView> {
        let phase = self.started_phase()?;
        let r#match = phase.data().matches().get(&id)?;
        if !r#match.is_finished() {
            return None;
        }
        let format = phase.match_format();
        let template = format.template(phase.data(), r#match);
        let form = ResultForm::reported(
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
        self.report_contest(form.match_id(), form.report())
    }

    /// Reports `contest` as what was entered for the match `id`, see [`Self::report_form`].
    fn report_contest(&mut self, id: MatchId, contest: Part) -> Result<Reported, ReportError> {
        let phase = self.started_phase().ok_or(ReportError::NotStarted)?;
        if !phase.data().matches().contains_key(&id) {
            return Err(ReportError::UnknownMatch);
        }
        let phase_id = phase.id();
        let matches_before = phase.data().matches().len();
        let report = ReportMatchResult::of(phase_id, id, contest).ok_or(ReportError::Undecided)?;
        self.machine
            .apply_in(&mut self.history, report)
            .map_err(|error| {
                let error = rejection(error);
                match error.downcast_ref() {
                    Some(engine::ReportMatchResultError::MatchAlreadyFinished) => {
                        ReportError::AlreadyFinished
                    }
                    Some(engine::ReportMatchResultError::DrawNotAllowed) => {
                        ReportError::DrawNotAllowed
                    }
                    _ => ReportError::Rejected(error),
                }
            })?;
        let matches = self
            .started_phase()
            .expect("still started")
            .data()
            .matches();
        let phase = self.started_phase().expect("still started");
        let row = self.match_row(phase, &phase.data().matches()[&id]);
        Ok(Reported {
            row,
            new_matches: matches.len() - matches_before,
            winners: self.winners(),
        })
    }

    /// Undoes the result reported for the match `id`, which may be any reported match rather than
    /// only the one reported last. Every other change stays in effect. The form of the match
    /// starts out at the result, see [`Self::result_form`].
    ///
    /// # Errors
    /// Returns an error if the competition has not started, has no such match, the match has no
    /// result, or later results depend on it ([`UndoReportError::DependedOnBy`], naming every
    /// one).
    pub fn undo_report(&mut self, id: MatchId) -> Result<Reopened, UndoReportError> {
        let phase = self.started_phase().ok_or(UndoReportError::NotStarted)?;
        let Some(r#match) = phase.data().matches().get(&id) else {
            return Err(UndoReportError::UnknownMatch);
        };
        let row = self.match_row(phase, r#match);
        let contest = r#match.contest().clone();
        let phase_id = phase.id();
        let matches_before = phase.data().matches().len();
        self.machine
            .undo_match_result(&mut self.history, phase_id, id)
            .map_err(|error| match error {
                UndoMatchResultError::NotReported => UndoReportError::NotReported,
                UndoMatchResultError::Rejected(_) => self.undo_refusal(phase_id, id),
            })?;
        self.reopened.insert(id, contest);
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

    /// Why undoing the result of the match `match_id` of the phase `phase_id` was rejected: the
    /// results that depend on it, if they can be told.
    fn undo_refusal(&self, phase_id: PhaseId, match_id: MatchId) -> UndoReportError {
        let conflicts = self
            .machine
            .reported_by(phase_id, match_id)
            .map(|report| self.conflicting_numbers(Change::Undo(report)));
        match conflicts {
            Some(Ok(numbers)) if !numbers.is_empty() => UndoReportError::DependedOnBy(numbers),
            _ => UndoReportError::DependedOn,
        }
    }

    /// The match `id` with what was reported for it, if it has a result, which undoing its report
    /// takes back.
    pub(super) fn reported_contest(&self, id: MatchId) -> Option<(MatchId, Part)> {
        let r#match = self.started_phase()?.data().matches().get(&id)?;
        r#match
            .is_finished()
            .then(|| (id, r#match.contest().clone()))
    }

    /// The started phase and its match `id`, which has no result yet.
    fn open_match(&self, id: MatchId) -> Result<(&Phase, &Match), ReportError> {
        let phase = self.started_phase().ok_or(ReportError::NotStarted)?;
        let r#match = phase
            .data()
            .matches()
            .get(&id)
            .ok_or(ReportError::UnknownMatch)?;
        if r#match.is_finished() {
            return Err(ReportError::AlreadyFinished);
        }
        Ok((phase, r#match))
    }
}
