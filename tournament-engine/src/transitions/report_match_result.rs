use serde::{Deserialize, Serialize};
use thiserror::Error;
use undoable_state_machine::{Transition, TransitionError};

use crate::{
    entities::{
        competition::{Annotated, Competition, MatchId, MatchResult, PhaseId},
        details::{Decision, EnteredValue, FieldKey, Part, Sides, Values},
    },
    plugins::match_format::Misfit,
    type_map::TypeMap,
};

/// Reports how a match was decided, which finishes it, with the values and parts entered for it.
#[derive(Debug, Deserialize)]
#[serde(from = "Saved")]
pub struct ReportMatchResult {
    phase_id: PhaseId,
    match_id: MatchId,
    /// What was entered for the match; always decided.
    contest: Part,
}

impl ReportMatchResult {
    /// Reports `decision` for the match `match_id` of the phase `phase_id`, no values or parts.
    pub fn new(phase_id: PhaseId, match_id: MatchId, decision: impl Into<Decision>) -> Self {
        Self {
            phase_id,
            match_id,
            contest: Part::new().with_decision(decision),
        }
    }

    /// Reports what was entered for the match `match_id` of the phase `phase_id` as its
    /// `contest`: its decision, values and parts (its annotations are not reported). `None` if
    /// it has no decision.
    pub fn of(phase_id: PhaseId, match_id: MatchId, contest: Part) -> Option<Self> {
        contest.decision()?;
        Some(Self {
            phase_id,
            match_id,
            contest: contest.with_annotations(TypeMap::new()),
        })
    }

    /// This report with `values` for `field` of the match, replacing any it had.
    #[must_use]
    pub fn with_value(mut self, field: FieldKey, values: Sides<Option<EnteredValue>>) -> Self {
        self.contest = self.contest.with_value(field, values);
        self
    }

    /// This report with `part` as the last part of the match.
    #[must_use]
    pub fn with_part(mut self, part: Part) -> Self {
        self.contest = self.contest.with_part(part);
        self
    }

    pub fn phase_id(&self) -> PhaseId {
        self.phase_id
    }

    pub fn match_id(&self) -> MatchId {
        self.match_id
    }

    pub fn match_result(&self) -> MatchResult {
        self.decision().result()
    }

    pub fn decision(&self) -> &Decision {
        self.contest.decision().expect("a report is decided")
    }

    /// What is reported for the match: its decision, values and parts.
    pub fn contest(&self) -> &Part {
        &self.contest
    }
}

// The saved form keeps the fields a report had before it held its contest as a `Part`.
impl Serialize for ReportMatchResult {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        #[serde(rename = "ReportMatchResult")]
        struct Saving<'a> {
            phase_id: PhaseId,
            match_id: MatchId,
            decision: &'a Decision,
            values: &'a Values,
            parts: &'a [Part],
        }
        Saving {
            phase_id: self.phase_id,
            match_id: self.match_id,
            decision: self.decision(),
            values: self.contest.values(),
            parts: self.contest.parts(),
        }
        .serialize(serializer)
    }
}

#[derive(Deserialize)]
#[serde(rename = "ReportMatchResult")]
struct Saved {
    phase_id: PhaseId,
    match_id: MatchId,
    decision: Decision,
    values: Values,
    parts: Vec<Part>,
}

impl From<Saved> for ReportMatchResult {
    fn from(saved: Saved) -> Self {
        let mut contest = Part::new().with_decision(saved.decision);
        for (field, values) in saved.values.iter() {
            contest = contest.with_value(field.clone(), *values);
        }
        Self {
            phase_id: saved.phase_id,
            match_id: saved.match_id,
            contest: saved.parts.into_iter().fold(contest, Part::with_part),
        }
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
        if self.match_result() == MatchResult::Draw && !phase.format().allows_draws() {
            return Err(Box::new(ReportMatchResultError::DrawNotAllowed));
        }
        phase
            .match_format()
            .template(phase.data(), r#match)
            .root
            .check(&self.contest)
            .map_err(ReportMatchResultError::from)?;
        Ok(())
    }

    fn apply(
        &self,
        competition: &mut Competition,
    ) -> Result<(), undoable_state_machine::TransitionError> {
        let phase = competition.phases_mut().get_mut(&self.phase_id).unwrap();
        phase.report_match(self.match_id, self.contest.clone());
        Ok(())
    }
}
