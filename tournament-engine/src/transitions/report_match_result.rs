use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;
use undoable_state_machine::{Transition, TransitionError};

use crate::{
    entities::{
        competition::{Competition, MatchId, MatchResult, PhaseId},
        details::Decision,
    },
    transitions::CompetitionTransition,
};

/// Reports how a match was decided, which finishes it.
#[derive(Debug, Serialize)]
pub struct ReportMatchResult {
    phase_id: PhaseId,
    match_id: MatchId,
    decision: Decision,
}

impl<'de> Deserialize<'de> for ReportMatchResult {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(rename = "ReportMatchResult")]
        struct Current {
            phase_id: PhaseId,
            match_id: MatchId,
            decision: Decision,
        }
        // Logged in JSON by 0.3.0 and earlier: a `match_result` in place of the `decision`.
        #[derive(Deserialize)]
        #[serde(rename = "ReportMatchResult")]
        struct Saved {
            phase_id: PhaseId,
            match_id: MatchId,
            #[serde(default)]
            decision: Option<Decision>,
            #[serde(default)]
            match_result: Option<MatchResult>,
        }

        if deserializer.is_human_readable() {
            let saved = Saved::deserialize(deserializer)?;
            let decision = saved
                .decision
                .or(saved.match_result.map(Decision::new))
                .ok_or_else(|| serde::de::Error::missing_field("decision"))?;
            Ok(Self::new(saved.phase_id, saved.match_id, decision))
        } else {
            let current = Current::deserialize(deserializer)?;
            Ok(Self::new(
                current.phase_id,
                current.match_id,
                current.decision,
            ))
        }
    }
}

impl ReportMatchResult {
    pub fn new(phase_id: PhaseId, match_id: MatchId, decision: impl Into<Decision>) -> Self {
        Self {
            phase_id,
            match_id,
            decision: decision.into(),
        }
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
        Ok(())
    }

    fn apply(
        &self,
        competition: &mut Competition,
    ) -> Result<(), undoable_state_machine::TransitionError> {
        let phase = competition.phases_mut().get_mut(&self.phase_id).unwrap();
        phase.report_match_result(self.match_id, self.decision.clone());
        Ok(())
    }
}

#[typetag::serde(name = "tournament_engine::report_match_result")]
impl CompetitionTransition for ReportMatchResult {}
