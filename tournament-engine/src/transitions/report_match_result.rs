use serde::{Deserialize, Serialize};
use thiserror::Error;
use undoable_state_machine::{Transition, TransitionError};

use crate::{
    entities::{
        competition::{Competition, MatchId, MatchResult, PhaseId},
        details::{Decision, EnteredValue, FieldKey, Part, Sides, Values},
    },
    plugins::match_format::Misfit,
    transitions::CompetitionTransition,
};

/// Reports how a match was decided, which finishes it, with the values and parts entered for it.
#[derive(Debug, Serialize, Deserialize)]
pub struct ReportMatchResult {
    phase_id: PhaseId,
    match_id: MatchId,
    decision: Decision,
    values: Values,
    parts: Vec<Part>,
}

impl ReportMatchResult {
    /// Reports `decision` for the match `match_id` of the phase `phase_id`, no values or parts.
    pub fn new(phase_id: PhaseId, match_id: MatchId, decision: impl Into<Decision>) -> Self {
        Self {
            phase_id,
            match_id,
            decision: decision.into(),
            values: Values::new(),
            parts: Vec::new(),
        }
    }

    /// This report with `values` for `field` of the match, replacing any it had.
    #[must_use]
    pub fn with_value(mut self, field: FieldKey, values: Sides<Option<EnteredValue>>) -> Self {
        match self.values.iter_mut().find(|(key, _)| *key == field) {
            Some((_, old)) => *old = values,
            None => self.values.push((field, values)),
        }
        self
    }

    /// This report with `part` as the last part of the match.
    #[must_use]
    pub fn with_part(mut self, part: Part) -> Self {
        self.parts.push(part);
        self
    }

    pub fn phase_id(&self) -> PhaseId {
        self.phase_id
    }

    pub fn match_id(&self) -> MatchId {
        self.match_id
    }

    pub fn match_result(&self) -> MatchResult {
        self.decision.result()
    }

    pub fn decision(&self) -> &Decision {
        &self.decision
    }

    pub fn values(&self) -> &Values {
        &self.values
    }

    pub fn parts(&self) -> &[Part] {
        &self.parts
    }
}

#[derive(Error, Debug)]
pub enum ReportMatchResultError {
    #[error("phase does not exist")]
    PhaseDoesNotExist,
    #[error("match does not exist")]
    MatchDoesNotExist,
    #[error("match already finished")]
    MatchAlreadyFinished,
    #[error("the phase format does not allow draws")]
    DrawNotAllowed,
    #[error("the result does not fit the match format: {0}")]
    Misfit(#[from] Misfit),
}

impl Transition<Competition> for ReportMatchResult {
    fn check_applicable(&self, competition: &Competition) -> Result<(), TransitionError> {
        let phase = competition
            .phases()
            .get(&self.phase_id)
            .ok_or(ReportMatchResultError::PhaseDoesNotExist)?;
        let r#match = phase
            .data()
            .matches()
            .get(&self.match_id)
            .ok_or(ReportMatchResultError::MatchDoesNotExist)?;
        if r#match.is_finished() {
            return Err(Box::new(ReportMatchResultError::MatchAlreadyFinished));
        }
        if self.decision.result() == MatchResult::Draw && !phase.format().allows_draws() {
            return Err(Box::new(ReportMatchResultError::DrawNotAllowed));
        }
        phase
            .match_format()
            .template(phase.data(), r#match)
            .root
            .check(Some(&self.decision), &self.values, &self.parts)
            .map_err(ReportMatchResultError::from)?;
        Ok(())
    }

    fn apply(
        &self,
        competition: &mut Competition,
    ) -> Result<(), undoable_state_machine::TransitionError> {
        let phase = competition.phases_mut().get_mut(&self.phase_id).unwrap();
        phase.report_match(
            self.match_id,
            self.decision.clone(),
            self.values.clone(),
            self.parts.clone(),
        );
        Ok(())
    }
}

#[typetag::serde(name = "tournament_engine::report_match_result")]
impl CompetitionTransition for ReportMatchResult {}
