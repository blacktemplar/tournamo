//! Setting up the phases of a competition by a template.

use plugin_factory::{catalog::AnyAxis, config::Config};
use tournament_engine::{
    AddPhase, Phase, PhaseId,
    plugins::template::Template,
    undoable_state_machine::{LogEntry, LogEntryKind, TransitionId},
    uuid::Uuid,
};

use super::{Session, phase::plugins, refused};
use crate::{
    error::{AddTemplateError, Refusal},
    form::{self, PreviewPhase},
    i18n::Translator,
    view::{PhaseName, Plugin},
};

/// The template the phases of a competition were added by, and how they differ from it, see
/// [`Session::based_on`].
#[derive(Debug, Clone, PartialEq)]
pub struct BasedOn {
    /// The template, as [`Session::templates`] lists it.
    pub template: Plugin,
    /// The phases added besides those of the template, in order.
    pub added: Vec<PhaseName>,
    /// The phases of the template that were removed, in the order of the template.
    pub removed: Vec<RemovedPhase>,
}

/// A phase of a template that was removed, see [`BasedOn::removed`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemovedPhase {
    /// Its position among the phases of the template, the first being 1.
    pub number: usize,
    /// Its name, in the language asked for; `None` for a phase the template leaves unnamed.
    pub name: Option<String>,
}

/// The template `id`.
fn template(id: &str) -> Result<Box<dyn Template>, AddTemplateError> {
    <dyn Template>::of(id).ok_or_else(|| AddTemplateError::UnknownTemplate(id.to_owned()))
}

impl Session {
    /// The template the phases of the competition were added by, with the phases added besides
    /// and those of it removed, the names of those in the language of `tr`; `None` if no phase was
    /// added by a template this version knows.
    ///
    /// The template is compared as it is now, not as it was when it added the phases.
    pub fn based_on(&self, tr: &Translator) -> Option<BasedOn> {
        let phases = self.competition().phases();
        let id = phases
            .values()
            .find_map(|phase| phase.template())
            .map(|origin| origin.template.clone())?;
        let template = Self::templates()
            .into_iter()
            .find(|template| template.id == id)?;
        let of_template = <dyn Template>::of(&id)?.phases();
        let position = |phase: &Phase| {
            phase
                .template()
                .filter(|origin| origin.template == id && origin.phase < of_template.len())
                .map(|origin| origin.phase)
        };
        let added = phases
            .values()
            .filter(|phase| position(phase).is_none())
            .map(|phase| self.phase_name(phase))
            .collect();
        let removed = of_template
            .iter()
            .enumerate()
            .filter(|(index, _)| !phases.values().any(|phase| position(phase) == Some(*index)))
            .map(|(index, phase)| RemovedPhase {
                number: index + 1,
                name: phase.name.as_ref().map(|name| tr.template_phase(&id, name)),
            })
            .collect();
        Some(BasedOn {
            template,
            added,
            removed,
        })
    }

    /// The answers of the phase of a template the phase `phase` was added as, as the template
    /// gives them now; `None` if it was not added by a template, or by none this version knows.
    pub(super) fn template_answers(phase: &Phase) -> Option<Config> {
        let origin = phase.template()?;
        <dyn Template>::of(&origin.template)?
            .phases()
            .into_iter()
            .nth(origin.phase)
            .map(|phase| phase.answers)
    }

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
                id,
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
