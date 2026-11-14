use serde::{Deserialize, Serialize};
use thiserror::Error;
use undoable_state_machine::Transition;

use crate::{entities::Competition, transitions::ensure_not_started};

/// Closes the registration of a competition: draws the lots of its entrants (see
/// [`Competition::lots`]) and starts every phase fed by the registration alone.
#[derive(Debug, Serialize, Deserialize)]
pub struct StartCompetition;

/// Why a [`StartCompetition`] does not apply, besides
/// [`CompetitionAlreadyStarted`](crate::transitions::CompetitionAlreadyStarted).
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum StartCompetitionError {
    #[error("too few entrants")]
    TooFewEntrants,
    /// No phase takes entrants from the registration alone, so none would start.
    #[error("no phases")]
    NoPhases,
}

impl Transition<Competition> for StartCompetition {
    fn check_applicable(
        &self,
        competition: &Competition,
    ) -> Result<(), undoable_state_machine::TransitionError> {
        ensure_not_started(competition)?;
        if competition.entrants().len() < 2 {
            return Err(Box::new(StartCompetitionError::TooFewEntrants));
        }
        if !competition
            .phases()
            .values()
            .any(|phase| phase.is_fed_by_registration_alone())
        {
            return Err(Box::new(StartCompetitionError::NoPhases));
        }
        Ok(())
    }

    fn apply(&self, competition: &mut Competition) {
        competition.start();
    }
}
