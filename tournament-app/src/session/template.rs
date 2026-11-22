//! Setting up the phases of a competition by a template.

use plugin_factory::catalog::AnyAxis;
use tournament_engine::{
    AddPhase, PhaseId,
    plugins::template::Template,
    undoable_state_machine::{LogEntry, LogEntryKind, TransitionId},
    uuid::Uuid,
};

use super::{Session, phase::plugins, refused};
use crate::{
    error::{AddTemplateError, Refusal},
    form::{self, PreviewPhase},
    i18n::Translator,
    view::Plugin,
};

/// The template `id`.
fn template(id: &str) -> Result<Box<dyn Template>, AddTemplateError> {
    <dyn Template>::of(id).ok_or_else(|| AddTemplateError::UnknownTemplate(id.to_owned()))
}

impl Session {
    /// Every template a competition can be set up by, in the order of their ids.
    pub fn templates() -> Vec<Plugin> {
        plugins(AnyAxis::of::<dyn Template>())
    }

    /// The phases the template `id`, a [`Plugin::id`], would add to a competition without phases,
    /// in order, named in the language of `tr`, each with how it is played.
    ///
    /// # Errors
    /// Returns an error if there is no such template, or it does not expand.
    pub fn template_preview(
        id: &str,
        tr: &Translator,
    ) -> Result<Vec<PreviewPhase>, AddTemplateError> {
        form::template::preview(id, &*template(id)?, tr).map_err(AddTemplateError::DoesNotExpand)
    }

    /// Adds the phases of the template `id`, a [`Plugin::id`], named in the language of `tr`, as
    /// one action, which one [`Self::undo`] takes back, and returns their ids in order. They are
    /// ordinary phases, each edited and removed as any phase.
    ///
    /// # Errors
    /// Returns an error if the competition has a phase, there is no such template, it does not
    /// expand, or a phase is refused, as [`Self::add_phase`] would refuse it; then none is added.
    pub fn add_template(
        &mut self,
        id: &str,
        tr: &Translator,
    ) -> Result<Vec<PhaseId>, AddTemplateError> {
        if !self.competition().phases().is_empty() {
            return Err(AddTemplateError::HasPhases);
        }
        let phases = self
            .competition()
            .expand(
                &template(id)?.phases(),
                || PhaseId::new(Uuid::new_v4()),
                |phrase| tr.template_phase(id, phrase),
            )
            .map_err(AddTemplateError::DoesNotExpand)?;
        let ids = phases.iter().map(AddPhase::id).collect();
        let entries = phases
            .into_iter()
            .map(|added| LogEntry::new(TransitionId::new(), LogEntryKind::Apply(added.into())));
        self.history
            .apply_all(&mut self.machine, entries)
            .map_err(|error| {
                refused(error, AddTemplateError::Rejected, |refusal| match refusal {
                    Refusal::Sources(refusal) => Some(self.sources_error(refusal).into()),
                    _ => None,
                })
            })?;
        Ok(ids)
    }
}
