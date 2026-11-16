use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use undoable_state_machine::Transition;
use vec1::Vec1;

#[cfg(doc)]
use crate::plugins::teams::Teams;
use crate::{EntrantId, PhaseId, entities::Competition, transitions::ensure_not_started};

/// Closes the registration of a competition: draws the lots of its entrants (see
/// [`Competition::lots`]), orders the registration (see [`Competition::registration`]) and starts
/// every phase fed by the registration alone.
///
/// The default is the start by lot, with no order given.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartCompetition {
    /// Tiers of entrants, best first; the entrants of a tier are ordered by lot. Entrants in no
    /// tier follow, by lot.
    #[serde(default)]
    order: Vec<Vec1<EntrantId>>,
}

impl StartCompetition {
    /// The start ordering the registration by `order`: tiers of entrants, best first, the entrants
    /// of a tier by lot, then the entrants in no tier by lot. Each entrant may be named once.
    pub fn new(order: Vec<Vec1<EntrantId>>) -> Self {
        Self { order }
    }

    /// The tiers the registration is ordered by, best first.
    pub fn order(&self) -> &[Vec1<EntrantId>] {
        &self.order
    }
}

/// Why a [`StartCompetition`] does not apply, besides
/// [`CompetitionAlreadyStarted`](crate::transitions::CompetitionAlreadyStarted).
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum StartCompetitionError {
    #[error("too few entrants")]
    TooFewEntrants,
    /// No phase takes entrants from the registration alone, so none would start.
    #[error("no phases")]
    NoPhases,
    /// The order names an entrant the competition does not have.
    #[error("unknown entrant {0:?}")]
    UnknownEntrant(EntrantId),
    /// The order names an entrant more than once.
    #[error("entrant {0:?} named twice")]
    EntrantTwice(EntrantId),
    /// The field of the phase `phase`, which would start, cannot be formed into its teams, e.g. an
    /// odd number of entrants into pairs, see [`Teams::fits`].
    #[error("the field of a phase cannot be formed into its teams")]
    FieldDoesNotFit { phase: PhaseId },
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
        if let Some(phase) = competition.phases().values().find(|phase| {
            phase.is_fed_by_registration_alone()
                && !phase
                    .plugins()
                    .teams()
                    .fits(competition.field_len(&phase.id()))
        }) {
            return Err(Box::new(StartCompetitionError::FieldDoesNotFit {
                phase: phase.id(),
            }));
        }
        let mut named = HashSet::new();
        for entrant in self.order.iter().flatten() {
            if !competition.entrants().contains_key(entrant) {
                return Err(Box::new(StartCompetitionError::UnknownEntrant(*entrant)));
            }
            if !named.insert(*entrant) {
                return Err(Box::new(StartCompetitionError::EntrantTwice(*entrant)));
            }
        }
        Ok(())
    }

    fn apply(&self, competition: &mut Competition) {
        competition.start(&self.order);
    }
}
