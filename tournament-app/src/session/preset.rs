//! Setting up the phases of a competition by a preset.

use plugin_factory::{catalog::AnyAxis, config::ConfigError};
use tournament_engine::{
    AddPhase, PhaseId,
    plugins::preset::Preset,
    undoable_state_machine::{LogEntry, LogEntryKind, TransitionId},
    uuid::Uuid,
};

use super::{Session, phase::plugins, refused};
use crate::{
    error::{AddPresetError, PresetFormError, Refusal},
    form::PresetForm,
    i18n::Translator,
    view::Plugin,
};

impl Session {
    /// Every preset a competition can be set up by, in the order of their ids.
    pub fn presets() -> Vec<Plugin> {
        plugins(AnyAxis::of::<dyn Preset>())
    }

    /// A form of the preset `id`, a [`Plugin::id`], every field answered as it offers. Once
    /// complete, it is handed to [`Self::add_preset`].
    ///
    /// # Errors
    /// Returns an error if there is no such preset, or it offers an answer it does not take.
    pub fn preset_form(id: &str) -> Result<PresetForm, PresetFormError> {
        PresetForm::new(id).map_err(|error| match error {
            ConfigError::UnknownPlugin { .. } => PresetFormError::UnknownPreset(id.to_owned()),
            error => PresetFormError::Configuration(error),
        })
    }

    /// Adds the phases of the preset `form` configures, named in the language of `tr`, as one
    /// action, which one [`Self::undo`] takes back, and returns their ids in order. They are
    /// ordinary phases, each edited and removed as any phase.
    ///
    /// # Errors
    /// Returns an error if the competition has a phase, the form is not complete, the preset does
    /// not expand, or a phase is refused, as [`Self::add_phase`] would refuse it; then none is
    /// added.
    pub fn add_preset(
        &mut self,
        form: &PresetForm,
        tr: &Translator,
    ) -> Result<Vec<PhaseId>, AddPresetError> {
        if !self.competition().phases().is_empty() {
            return Err(AddPresetError::HasPhases);
        }
        let choice = form.preset().ok_or(AddPresetError::Incomplete)?;
        let phases = self
            .competition()
            .expand(
                &choice,
                || PhaseId::new(Uuid::new_v4()),
                |phrase| tr.preset_phase(form.id(), phrase),
            )
            .map_err(AddPresetError::DoesNotExpand)?;
        let ids = phases.iter().map(AddPhase::id).collect();
        let entries = phases
            .into_iter()
            .map(|added| LogEntry::new(TransitionId::new(), LogEntryKind::Apply(added.into())));
        self.history
            .apply_all(&mut self.machine, entries)
            .map_err(|error| {
                refused(error, AddPresetError::Rejected, |refusal| match refusal {
                    Refusal::Sources(refusal) => Some(self.sources_error(refusal).into()),
                    _ => None,
                })
            })?;
        Ok(ids)
    }
}
