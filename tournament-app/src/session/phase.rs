//! Configuring, adding and editing the phase.

use plugin_factory::{catalog::AnyAxis, config::PluginConfig, draft::Draft};
use tournament_engine::{
    AddPhase, CompetitionStateMachineExt as _, PhaseId, PhasePlugins,
    plugins::{
        metrics_calculator::MetricsCalculator, phase_format::PhaseFormat, ranker::SliceRanker,
    },
    transitions::{
        self as engine, Change, CompetitionAlreadyStarted, ConflictsError, ReconfigurePhaseError,
    },
    undoable_state_machine::StateMachineError,
    uuid::Uuid,
};

use super::{Session, rejection};
use crate::{
    error::{AddPhaseError, EditPhaseError, NoPhase, PhaseDraftError},
    form::PhaseForm,
    view::{Glossary, Plugin},
};

impl Session {
    /// Every format a phase can be played in.
    pub fn formats() -> Vec<Plugin> {
        plugins(AnyAxis::of::<dyn PhaseFormat>())
    }

    /// Every format, criterion of the standings and metric a phase can be played with.
    pub fn glossary() -> Glossary {
        Glossary {
            formats: Self::formats(),
            criteria: plugins(AnyAxis::of::<dyn SliceRanker>()),
            metrics: plugins(AnyAxis::of::<dyn MetricsCalculator>()),
        }
    }

    /// A draft of a phase of this competition played in the format `format`, a [`Plugin::id`],
    /// with schema defaults and format suggestions, see [`PhasePlugins::with_suggestions`].
    /// It can be changed before it is handed to [`Self::add_phase`].
    ///
    /// # Errors
    /// Returns an error if there is no such format, or it offers an answer it does not take.
    pub fn phase_draft(&self, format: &str) -> Result<Draft<PhasePlugins>, PhaseDraftError> {
        let entry = AnyAxis::of::<dyn PhaseFormat>()
            .entries()
            .find(|entry| entry.id() == format)
            .ok_or_else(|| PhaseDraftError::UnknownFormat(format.to_owned()))?;
        Ok(PhasePlugins::with_suggestions(
            self.competition()
                .phase_draft()
                .with(PhasePlugins::FORMAT, PluginConfig::new(entry.id()))?,
        )?)
    }

    /// A form of a new phase of this competition, answered as the phase was played whose adding
    /// this session undid last, so that adding it again adds the same phase; if it undid none,
    /// with every field answered as offered: in the format a phase offers, with what that
    /// recommends, following suggestions until customized. An undone phase's settings are kept.
    /// Once filled, its [draft](PhaseForm::draft) is handed to [`Self::add_phase`].
    ///
    /// # Errors
    /// Returns an error if a plugin offers an answer it does not take.
    pub fn phase_form(&self) -> Result<PhaseForm, PhaseDraftError> {
        let draft = self.competition().phase_draft();
        match &self.undone_phase {
            Some(config) => Ok(PhaseForm::new(draft.with_answers(config)?)),
            None => {
                let mut form = PhaseForm::new(draft);
                form.apply_suggestions()?;
                Ok(form)
            }
        }
    }

    /// A form of the phase as it is played, see [`Self::phase_draft_to_edit`]. Once changed, its
    /// [draft](PhaseForm::draft) is handed to [`Self::edit_phase`].
    ///
    /// # Errors
    /// Returns [`NoPhase`] if the competition has no phase.
    pub fn phase_form_to_edit(&self) -> Result<PhaseForm, NoPhase> {
        self.phase_draft_to_edit().map(PhaseForm::new)
    }

    /// Adds the phase `draft` configures, which is the only phase the competition is played in.
    /// `draft` is a draft of this competition's phase, e.g. from [`Self::phase_draft`].
    ///
    /// A calculator that has nothing to be configured is added for a metric the phase needs and
    /// nothing calculates, see [`Draft::add_implied`]; `draft` itself is left as it is.
    ///
    /// # Errors
    /// Returns an error if the plugins `draft` configures do not fit together, the competition has
    /// a phase already, or it has started.
    pub fn add_phase(&mut self, draft: &Draft<PhasePlugins>) -> Result<(), AddPhaseError> {
        let plugins = completed(draft).map_err(AddPhaseError::Invalid)?;
        self.machine
            .apply_in(
                &mut self.history,
                AddPhase::new(PhaseId::new(Uuid::new_v4()), plugins),
            )
            .map_err(|error| {
                let error = rejection(error);
                if error.is::<CompetitionAlreadyStarted>() {
                    AddPhaseError::Started
                } else if error.is::<engine::AddPhaseError>() {
                    AddPhaseError::PhaseAlreadyAdded
                } else {
                    AddPhaseError::Rejected(error)
                }
            })?;
        Ok(())
    }

    /// A draft holding the configuration the phase is played with, see [`Phase::config`], to be
    /// changed and handed to [`Self::edit_phase`].
    ///
    /// # Errors
    /// Returns [`NoPhase`] if the competition has no phase.
    ///
    /// # Panics
    /// Panics if the draft does not take the configuration of the phase, which it was built from.
    ///
    /// [`Phase::config`]: tournament_engine::Phase::config
    pub fn phase_draft_to_edit(&self) -> Result<Draft<PhasePlugins>, NoPhase> {
        let config = self.phase().ok_or(NoPhase)?.config();
        Ok(self
            .competition()
            .phase_draft()
            .with_answers(&config)
            .expect("a phase's draft takes the configuration the phase was built from"))
    }

    /// Plays the phase as `draft` configures it, as if it had been added that way, and returns
    /// whether that changed anything. `draft` is completed as by [`Self::add_phase`].
    ///
    /// # Errors
    /// Returns an error if the competition has no phase, the plugins `draft` configures do not fit
    /// together, or a change made after the phase was added, e.g. a reported result
    /// ([`EditPhaseError::ResultDoesNotFit`], naming every one), does not fit the edited phase.
    pub fn edit_phase(&mut self, draft: &Draft<PhasePlugins>) -> Result<bool, EditPhaseError> {
        let phase_id = self.phase().ok_or(NoPhase)?.id();
        let plugins = completed(draft).map_err(EditPhaseError::Invalid)?;
        let edited = self
            .machine
            .reconfigure_phase(&mut self.history, phase_id, plugins)
            .map_err(|error| match error {
                ReconfigurePhaseError::Rejected(error) => self.edit_refusal(phase_id, draft, error),
                ReconfigurePhaseError::NotAdded => {
                    unreachable!("the phase of a session is added by an `AddPhase` in effect")
                }
            })?;
        Ok(edited.is_some())
    }

    /// Why the edit of the phase `phase_id` to what `draft` configures was rejected as `rejected`:
    /// the reported results that do not fit the edited phase, if any do.
    fn edit_refusal(
        &self,
        phase_id: PhaseId,
        draft: &Draft<PhasePlugins>,
        rejected: StateMachineError,
    ) -> EditPhaseError {
        // the rejected edit took the plugins, which the draft builds again
        let plugins = completed(draft).expect("the draft built the plugins once");
        let edited = AddPhase::new(phase_id, plugins).into();
        let conflicts = self.machine.added_by(phase_id).map(|target| {
            self.conflicting_numbers(Change::Replace {
                target,
                with: &edited,
            })
        });
        match conflicts {
            Some(Ok(numbers)) if !numbers.is_empty() => EditPhaseError::ResultDoesNotFit(numbers),
            Some(Err(ConflictsError::Other(_, error))) => EditPhaseError::DoesNotFit(error),
            _ => EditPhaseError::DoesNotFit(rejection(rejected)),
        }
    }
}

/// The plugins `draft`, a phase draft, configures, with the plugins its answers imply added, see
/// [`Draft::add_implied`].
pub(super) fn completed(
    draft: &Draft<PhasePlugins>,
) -> Result<PhasePlugins, Vec<plugin_factory::draft::Issue>> {
    let mut draft = draft.clone();
    draft.add_implied();
    draft.build()
}

/// Every plugin of `axis`, in the order of their ids.
fn plugins(axis: AnyAxis) -> Vec<Plugin> {
    axis.entries()
        .map(|entry| Plugin {
            id: entry.id(),
            label: entry.label(),
            description: entry.description(),
        })
        .collect()
}
