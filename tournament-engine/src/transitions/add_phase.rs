use plugin_factory::{compatibility, schema::Configurable};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use undoable_state_machine::{Transition, TransitionError};
use vec1::{Vec1, vec1};

use crate::{
    Columns, Source, SourceOf,
    entities::{Competition, PhaseId},
    plugins::PhasePlugins,
};

/// Adds a phase to a competition: its name, where its entrants come from, and its plugins.
///
/// Applying it fails with an [`AddPhaseError`] if a phase with its id exists, its sources are not
/// as that error describes, or if the plugins of the phase do not fit together in the competition,
/// see [`compatibility::check`], with the context that [`Competition::phase_draft`] carries.
///
/// The phase is changed after it was added by replacing this transition with one adding it
/// otherwise, see [`reconfigure_phase`].
///
/// [`reconfigure_phase`]: crate::CompetitionStateMachineExt::reconfigure_phase
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddPhase {
    id: PhaseId,
    name: Option<String>,
    sources: Vec1<Source>,
    plugins: PhasePlugins,
    columns: Columns,
}

impl AddPhase {
    /// Adds the phase `id` with `plugins`, without a name, of the whole registration.
    ///
    /// Nothing is added to `plugins`: the plugins implied by a fact they require that none of them
    /// provides are added beforehand, to the draft they are built from, see
    /// [`Draft::add_implied`](plugin_factory::draft::Draft::add_implied).
    pub fn new(id: PhaseId, plugins: PhasePlugins) -> Self {
        Self {
            id,
            name: None,
            sources: vec1![Source::registration()],
            plugins,
            columns: Columns::new(),
        }
    }

    /// This phase named `name`, or without a name.
    #[must_use]
    pub fn with_name(mut self, name: Option<String>) -> Self {
        self.name = name;
        self
    }

    /// This phase taking its entrants from `sources`, in this order.
    #[must_use]
    pub fn with_sources(mut self, sources: Vec1<Source>) -> Self {
        self.sources = sources;
        self
    }

    /// This phase showing in its tables of standings what `columns` say, without what they say
    /// about a calculator that is not among the plugins of the phase.
    #[must_use]
    pub fn with_columns(mut self, mut columns: Columns) -> Self {
        let metrics: Vec<&str> = self
            .plugins
            .metrics()
            .iter()
            .map(|calculator| plugin_factory::id_of(&**calculator))
            .collect();
        columns.retain(|id| metrics.contains(&id));
        self.columns = columns;
        self
    }

    pub fn id(&self) -> PhaseId {
        self.id
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn sources(&self) -> &Vec1<Source> {
        &self.sources
    }

    pub fn plugins(&self) -> &PhasePlugins {
        &self.plugins
    }

    /// What the operator said about the columns of the tables of standings of the phase.
    pub fn columns(&self) -> &Columns {
        &self.columns
    }
}

/// Why an [`AddPhase`] does not apply, besides an
/// [`Incompatibility`](compatibility::Incompatibility) of its plugins.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum AddPhaseError {
    #[error("a phase with this id exists")]
    PhaseExists,
    /// A source takes places of a phase that was not added before this one.
    #[error("a source takes places of a phase that was not added before")]
    UnknownPhase(PhaseId),
    /// A source ends before it starts.
    #[error("a source ends before it starts")]
    EmptyRange(Source),
    /// The source `range` takes a place that the source `other` takes, of this phase if `phase`
    /// is `None`, else of the phase `phase`.
    #[error("two sources take the same place")]
    Overlap {
        range: Source,
        other: Source,
        phase: Option<PhaseId>,
    },
    /// The competition has started, and every source of the phase is the registration, so that
    /// it would never start.
    #[error("the registration is closed")]
    RegistrationClosed,
}

impl Transition<Competition> for AddPhase {
    fn check_applicable(&self, competition: &Competition) -> Result<(), TransitionError> {
        if competition.phase(&self.id).is_some() {
            return Err(Box::new(AddPhaseError::PhaseExists));
        }
        for (index, source) in self.sources.iter().enumerate() {
            if let SourceOf::Phase(phase) = source.of()
                && competition.phase(&phase).is_none()
            {
                return Err(Box::new(AddPhaseError::UnknownPhase(phase)));
            }
            if source.is_empty() {
                return Err(Box::new(AddPhaseError::EmptyRange(*source)));
            }
            let earlier = self.sources[..index].iter().map(|other| (other, None));
            let of_phases = competition.phases().values().flat_map(|phase| {
                phase
                    .sources()
                    .iter()
                    .map(move |other| (other, Some(phase.id())))
            });
            if let Some((other, phase)) = earlier
                .chain(of_phases)
                .find(|(other, _)| source.overlaps(other))
            {
                return Err(Box::new(AddPhaseError::Overlap {
                    range: *source,
                    other: *other,
                    phase,
                }));
            }
        }
        if competition.is_started()
            && self
                .sources
                .iter()
                .all(|source| source.of() == SourceOf::Registration)
        {
            return Err(Box::new(AddPhaseError::RegistrationClosed));
        }
        compatibility::check(
            |declaration| competition.declare_phase_context(declaration),
            &self.plugins.plugins(),
        )?;
        Ok(())
    }

    fn apply(&self, state: &mut Competition) {
        state.add_phase(
            self.id,
            self.name.clone(),
            self.sources.clone(),
            self.plugins.clone(),
            self.columns.clone(),
        );
    }
}
