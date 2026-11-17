use serde::{Deserialize, Serialize};
use thiserror::Error;
use undoable_state_machine::{Transition, TransitionError};

#[cfg(doc)]
use crate::plugins::teams::Teams;
use crate::{
    SourceOf,
    entities::{Competition, PhaseId},
};

/// Starts a phase that takes entrants from another phase, with its field as things stand, see
/// [`Competition::field_of`]. A phase fed by the registration alone starts with the competition,
/// see [`StartCompetition`](crate::StartCompetition).
///
/// Applying it fails with a [`StartPhaseError`] unless the competition has started, the phase
/// exists, has not started and has a source in a phase, every phase it has a source in is
/// finished, and its field can be formed into its teams.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartPhase {
    phase_id: PhaseId,
}

impl StartPhase {
    pub fn new(phase_id: PhaseId) -> Self {
        Self { phase_id }
    }

    pub fn phase_id(&self) -> PhaseId {
        self.phase_id
    }
}

/// Why a [`StartPhase`] does not apply.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum StartPhaseError {
    #[error("the competition has not started")]
    CompetitionNotStarted,
    #[error("no phase with this id was added")]
    NoSuchPhase,
    #[error("the phase has started")]
    AlreadyStarted,
    /// The phase takes entrants only from the registration, so it starts with the competition.
    #[error("the phase takes entrants from no phase")]
    NoPhaseSource,
    /// The phase takes entrants from a phase that is not finished.
    #[error("a phase the phase takes entrants from is not finished")]
    SourceNotFinished(PhaseId),
    /// The field of the phase `phase` cannot be formed into its teams, e.g. an odd number of
    /// entrants into pairs, see [`Teams::fits`].
    #[error("the field of the phase cannot be formed into its teams")]
    FieldDoesNotFit { phase: PhaseId },
}

impl Transition<Competition> for StartPhase {
    fn check_applicable(&self, competition: &Competition) -> Result<(), TransitionError> {
        if !competition.is_started() {
            return Err(Box::new(StartPhaseError::CompetitionNotStarted));
        }
        let phase = competition
            .phase(&self.phase_id)
            .ok_or(StartPhaseError::NoSuchPhase)?;
        if phase.is_started() {
            return Err(Box::new(StartPhaseError::AlreadyStarted));
        }
        if phase.is_fed_by_registration_alone() {
            return Err(Box::new(StartPhaseError::NoPhaseSource));
        }
        for source in phase.sources() {
            if let SourceOf::Phase(from) = source.of()
                && !competition
                    .phase(&from)
                    .is_some_and(|from| from.is_finished())
            {
                return Err(Box::new(StartPhaseError::SourceNotFinished(from)));
            }
        }
        if !phase
            .plugins()
            .teams()
            .fits(competition.field_len(&self.phase_id))
        {
            return Err(Box::new(StartPhaseError::FieldDoesNotFit {
                phase: self.phase_id,
            }));
        }
        Ok(())
    }

    fn apply(&self, competition: &mut Competition) {
        competition.start_phase(self.phase_id);
    }
}
