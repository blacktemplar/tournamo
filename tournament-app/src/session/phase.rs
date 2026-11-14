//! Configuring, adding and editing a phase.

use plugin_factory::{catalog::AnyAxis, config::PluginConfig, draft::Draft};
use tournament_engine::{
    AddPhase, CompetitionStateMachineExt as _, CompetitionTransition, PhaseId, PhasePlugins,
    Source, SourceOf,
    plugins::{
        metrics_calculator::MetricsCalculator, phase_format::PhaseFormat, ranker::SliceRanker,
    },
    transitions::{Change, ConflictsError, ReconfigurePhaseError},
    undoable_state_machine::{StateMachineError, TransitionId},
    uuid::Uuid,
};

use super::{Session, refused, rejection};
use crate::{
    error::{
        AddPhaseError, EditPhaseError, NoPhase, PhaseDraftError, Refusal, SourcesError,
        SourcesRefusal,
    },
    form::{Context, PhaseForm},
    view::{Glossary, PhaseName, Plugin},
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
    /// recommends, following suggestions until customized, taking every entrant registered if it
    /// is the first phase, else every place of the phase before. An undone phase's settings are
    /// kept. Once filled, it is handed to [`Self::add_phase`].
    ///
    /// # Errors
    /// Returns an error if a plugin offers an answer it does not take.
    pub fn phase_form(&self) -> Result<PhaseForm, PhaseDraftError> {
        let draft = self.competition().phase_draft();
        let context = self.form_context(None);
        Ok(match &self.undone_phase {
            Some(undone) => PhaseForm::new(draft.with_answers(&undone.config)?).of_phase(
                context,
                undone.name.as_deref(),
                undone.sources.to_vec(),
            ),
            None => {
                let sources = vec![match self.competition().phases().values().last() {
                    Some(before) => Source::everyone(SourceOf::Phase(before.id())),
                    None => Source::registration(),
                }];
                let mut form = PhaseForm::new(draft).of_phase(context, None, sources);
                form.apply_suggestions()?;
                form
            }
        })
    }

    /// A form of the phase `id` as it is, see [`Self::phase_draft_to_edit`]. Once changed, it is
    /// handed to [`Self::edit_phase`].
    ///
    /// # Errors
    /// Returns [`NoPhase`] if the competition has no such phase.
    pub fn phase_form_to_edit(&self, id: PhaseId) -> Result<PhaseForm, NoPhase> {
        let draft = self.phase_draft_to_edit(id)?;
        let phase = self.phase(id).ok_or(NoPhase)?;
        Ok(PhaseForm::new(draft).of_phase(
            self.form_context(Some(id)),
            phase.name(),
            phase.sources().to_vec(),
        ))
    }

    /// What a form of the phase `id`, or of a new phase if `None`, knows of the competition.
    fn form_context(&self, id: Option<PhaseId>) -> Context {
        let competition = self.competition();
        let phases = competition.phases();
        let index = id
            .and_then(|id| phases.get_index_of(&id))
            .unwrap_or(phases.len());
        Context {
            number: index + 1,
            earlier: phases
                .values()
                .take(index)
                .map(|phase| (phase.id(), self.phase_name(phase)))
                .collect(),
            taken: phases
                .values()
                .filter(|phase| Some(phase.id()) != id)
                .flat_map(|phase| {
                    let name = self.phase_name(phase);
                    phase
                        .sources()
                        .iter()
                        .map(move |source| (*source, name.clone()))
                })
                .collect(),
            registration_closed: competition.is_started()
                && id
                    .and_then(|id| self.phase(id))
                    .is_none_or(|phase| !phase.is_fed_by_registration_alone()),
        }
    }

    /// Adds the phase `form` configures and returns its id. Its draft is a draft of this
    /// competition's phase, e.g. from [`Self::phase_form`].
    ///
    /// A calculator that has nothing to be configured is added for a metric the phase needs and
    /// nothing calculates, see [`Draft::add_implied`]; the form itself is left as it is.
    ///
    /// # Errors
    /// Returns an error if the plugins `form` configures do not fit together, or its sources are
    /// refused, see [`SourcesError`].
    pub fn add_phase(&mut self, form: &PhaseForm) -> Result<PhaseId, AddPhaseError> {
        let (name, sources) = form.entry()?;
        let plugins = completed(form.draft()).map_err(AddPhaseError::Invalid)?;
        let id = PhaseId::new(Uuid::new_v4());
        let added = AddPhase::new(id, plugins)
            .with_name(name)
            .with_sources(sources);
        self.history
            .apply_new(&mut self.machine, added.into())
            .map_err(|error| {
                refused(error, AddPhaseError::Rejected, |refusal| match refusal {
                    Refusal::Sources(refusal) => Some(self.sources_error(refusal).into()),
                    _ => None,
                })
            })?;
        Ok(id)
    }

    /// What the app says of the sources the engine refused as `refusal`.
    pub(super) fn sources_error(&self, refusal: SourcesRefusal) -> SourcesError {
        match refusal {
            SourcesRefusal::Empty => SourcesError::EmptyRange,
            SourcesRefusal::Overlap(phase) => {
                SourcesError::Overlap(phase.and_then(|phase| self.name_of_phase(phase)))
            }
            SourcesRefusal::RegistrationClosed => SourcesError::RegistrationClosed,
        }
    }

    /// A draft holding the configuration the phase `id` is played with, see [`Phase::config`], to
    /// be changed and handed to [`Self::edit_phase`] in a form.
    ///
    /// # Errors
    /// Returns [`NoPhase`] if the competition has no such phase.
    ///
    /// # Panics
    /// Panics if the draft does not take the configuration of the phase, which it was built from.
    ///
    /// [`Phase::config`]: tournament_engine::Phase::config
    pub fn phase_draft_to_edit(&self, id: PhaseId) -> Result<Draft<PhasePlugins>, NoPhase> {
        let config = self.phase(id).ok_or(NoPhase)?.config();
        Ok(self
            .competition()
            .phase_draft()
            .with_answers(&config)
            .expect("a phase's draft takes the configuration the phase was built from"))
    }

    /// Plays the phase `id` as `form` configures it, its name and sources included, as if it had
    /// been added that way, and returns whether that changed anything. Its draft is completed as
    /// by [`Self::add_phase`].
    ///
    /// # Errors
    /// Returns an error if the competition has no such phase, the plugins `form` configures do not
    /// fit together, its sources are refused, or a change made after the phase was added, e.g. a
    /// reported result ([`EditPhaseError::ResultDoesNotFit`], naming every one) or the start of
    /// a phase taking entrants from it ([`EditPhaseError::PhaseStarted`]), does not fit the edited
    /// phase.
    pub fn edit_phase(&mut self, id: PhaseId, form: &PhaseForm) -> Result<bool, EditPhaseError> {
        self.phase(id).ok_or(NoPhase)?;
        let (name, sources) = form.entry()?;
        let plugins = completed(form.draft()).map_err(EditPhaseError::Invalid)?;
        let edited = AddPhase::new(id, plugins)
            .with_name(name)
            .with_sources(sources);
        let changed = self
            .machine
            .reconfigure_phase(&mut self.history, edited.clone())
            .map_err(|error| match error {
                ReconfigurePhaseError::Rejected(error) => self.edit_refusal(edited, error),
                ReconfigurePhaseError::NotAdded => {
                    unreachable!("the phase of a session is added by an `AddPhase` in effect")
                }
            })?;
        Ok(changed.is_some())
    }

    /// Why the edit of a phase to `edited` was rejected as `rejected`: the reported results that
    /// do not fit the edited phase, if any do.
    fn edit_refusal(&self, edited: AddPhase, rejected: StateMachineError) -> EditPhaseError {
        let id = edited.id();
        let edited = edited.into();
        let conflicts = self.machine.added_by(id).map(|target| {
            self.conflicting_numbers(Change::Replace {
                target,
                with: &edited,
            })
        });
        match conflicts {
            Some(Ok(numbers)) if !numbers.is_empty() => EditPhaseError::ResultDoesNotFit(numbers),
            Some(Err(ConflictsError::Other(entry, error))) => {
                if let Some(phase) = self.started_by(entry) {
                    return EditPhaseError::PhaseStarted(phase);
                }
                match Refusal::of(&error) {
                    Some(Refusal::Sources(refusal)) => self.sources_error(refusal).into(),
                    _ => EditPhaseError::DoesNotFit(error),
                }
            }
            _ => {
                let error = rejection(rejected);
                match Refusal::of(&error) {
                    Some(Refusal::Sources(refusal)) => self.sources_error(refusal).into(),
                    _ => EditPhaseError::DoesNotFit(error),
                }
            }
        }
    }

    /// The phase the log entry `entry` starts, if it is a [`StartPhase`].
    ///
    /// [`StartPhase`]: tournament_engine::StartPhase
    pub(super) fn started_by(&self, entry: TransitionId) -> Option<PhaseName> {
        match self.machine.effective_transition(entry)? {
            CompetitionTransition::StartPhase(start) => self.name_of_phase(start.phase_id()),
            _ => None,
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
