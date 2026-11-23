//! The phases a template would add, shown before it is applied.

use tournament_engine::{
    Competition, PhaseId,
    plugins::template::{Template, TemplateError},
    uuid::Uuid,
};

use super::{FormField, PhaseForm};
use crate::{
    i18n::Translator,
    session::named,
    view::{PhaseName, SourceRow},
};

/// A phase a template would add, see
/// [`Session::template_preview`](crate::session::Session::template_preview).
#[derive(Debug, Clone, PartialEq)]
pub struct PreviewPhase {
    /// An id standing for the phase in the preview, which its sources name; the phase added gets
    /// another.
    pub id: PhaseId,
    pub name: PhaseName,
    /// Where its entrants come from, in order.
    pub sources: Vec<SourceRow>,
    /// How it is played, as [`FormView::summary`](super::FormView::summary) shows it.
    pub summary: Vec<FormField>,
}

/// The phases `template`, of the id `id`, would add to a competition without phases, in order,
/// named in the language of `tr`.
pub(crate) fn preview(
    id: &str,
    template: &dyn Template,
    tr: &Translator,
) -> Result<Vec<PreviewPhase>, TemplateError> {
    let mut next = 0;
    let competition = Competition::default();
    let phases = competition.expand(
        id,
        &template.phases(),
        || {
            next += 1;
            PhaseId::new(Uuid::from_u128(next))
        },
        |phrase| tr.template_phase(id, phrase),
    )?;
    Ok(phases
        .iter()
        .enumerate()
        .map(|(index, added)| {
            let draft = competition
                .phase_draft(added.sources())
                .with_answers(&added.plugins().config())
                .expect("a phase's draft takes the configuration it was built from");
            PreviewPhase {
                id: added.id(),
                name: named(index + 1, added.name(), added.plugins().format()),
                sources: added.sources().iter().map(SourceRow::of).collect(),
                summary: PhaseForm::new(draft).view().summary(),
            }
        })
        .collect())
}
