use serde::{Deserialize, Serialize};
use thiserror::Error;
use undoable_state_machine::{Transition, TransitionError};

use super::report_match_result::{ReportMatchResult, ReportMatchResultError};
use crate::entities::Competition;

/// Corrects the result of a finished match in place: replaces how it was decided and the values
/// and parts entered for it by those of a report, and keeps everything scheduled after it as it
/// is, even where the format would have scheduled otherwise with the corrected result. The
/// metrics of its division are calculated anew, so the standings count the corrected result.
///
/// Applying it fails with a [`CorrectMatchResultError`] unless the match is finished, the report
/// fits it as a [`ReportMatchResult`] has to, and its format
/// [accepts the correction](crate::plugins::phase_format::PhaseFormat::accepts_correction).
///
/// Saved as the [`ReportMatchResult`] it holds.
#[derive(Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CorrectMatchResult(pub(super) ReportMatchResult);

impl CorrectMatchResult {
    /// Corrects the match of `report` to what `report` reports.
    pub fn new(report: ReportMatchResult) -> Self {
        Self(report)
    }

    /// What the match is corrected to, as a report of it.
    pub fn report(&self) -> &ReportMatchResult {
        &self.0
    }
}

/// Why a [`CorrectMatchResult`] does not apply.
#[derive(Error, Debug)]
pub enum CorrectMatchResultError {
    /// The match has no result to correct.
    #[error("match not finished")]
    MatchNotFinished,
    /// The format of the match keeps something of its result that the correction would change,
    /// e.g. a knockout who moved on.
    #[error("the phase format does not accept the correction")]
    NotAccepted,
    /// The report does not fit the match, as it would not as a [`ReportMatchResult`], e.g. a draw
    /// in a format without draws.
    #[error(transparent)]
    Report(#[from] ReportMatchResultError),
}

impl Transition<Competition> for CorrectMatchResult {
    fn check_applicable(&self, competition: &Competition) -> Result<(), TransitionError> {
        let report = &self.0;
        let phase = competition
            .phases()
            .get(&report.phase_id())
            .ok_or(ReportMatchResultError::PhaseDoesNotExist)
            .map_err(CorrectMatchResultError::from)?;
        let (_, division) = phase
            .division_of(&report.match_id())
            .ok_or(ReportMatchResultError::MatchDoesNotExist)
            .map_err(CorrectMatchResultError::from)?;
        let r#match = &division.data().matches()[&report.match_id()];
        if !r#match.is_finished() {
            return Err(Box::new(CorrectMatchResultError::MatchNotFinished));
        }
        report
            .check_fits(division, r#match)
            .map_err(CorrectMatchResultError::from)?;
        if !division
            .format()
            .accepts_correction(division.data(), r#match, report.contest())
        {
            return Err(Box::new(CorrectMatchResultError::NotAccepted));
        }
        Ok(())
    }

    fn apply(&self, competition: &mut Competition) {
        let report = &self.0;
        competition.correct_match(
            report.phase_id(),
            report.match_id(),
            report.contest().clone(),
        );
    }
}
