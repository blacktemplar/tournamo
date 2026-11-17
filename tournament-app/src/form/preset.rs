//! A preset being configured: its fields as a form, and the phases it would add.

use plugin_factory::{config::ConfigError, draft::Draft};
use tournament_engine::{Competition, PhaseId, plugins::preset::PresetChoice, uuid::Uuid};

use super::{
    Answering, FormField, Generator, Input, Path, PhaseForm, PluginInput, Whole, issue_text, place,
};
use crate::{
    i18n::{Text, Translator},
    rejected::Rejections,
    session::named,
    view::{PhaseName, SourceRow},
};

/// A preset being configured, see [`Session::preset_form`]: its fields, edited as those of a
/// phase (see [`Editable`](super::Editable)), and the phases it would add. Once complete, it is
/// handed to [`Session::add_preset`].
///
/// [`Session::preset_form`]: crate::session::Session::preset_form
/// [`Session::add_preset`]: crate::session::Session::add_preset
#[derive(Clone)]
pub struct PresetForm {
    draft: Draft<PresetChoice>,
    rejected: Rejections<Path>,
}

/// What a [`PresetForm`] shows: plain data for a screen to lay out.
#[derive(Debug, Clone, PartialEq)]
pub struct PresetView {
    /// The fields of the preset, in schema order.
    pub fields: Vec<FormField>,
    /// What is wrong with the answers that no field in particular fixes.
    pub issues: Vec<Text>,
    /// Whether nothing is wrong with the answers and every number entered was taken, so that the
    /// preset can be added.
    pub complete: bool,
}

/// A phase a preset would add, see [`PresetForm::preview`].
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

impl PresetForm {
    /// A form of the preset `id`, every field answered as it offers.
    ///
    /// # Errors
    /// Returns [`ConfigError::UnknownPlugin`] if no preset has this id, and an error if it offers
    /// an answer it does not take.
    pub(crate) fn new(id: &str) -> Result<Self, ConfigError> {
        Ok(Self {
            draft: PresetChoice::draft_of(id)?,
            rejected: Rejections::default(),
        })
    }

    /// The id of the preset.
    pub fn id(&self) -> &'static str {
        self.draft
            .plugin_at(&Path::ROOT.field(PresetChoice::PRESET))
            .map_or("", |plugin| plugin.entry.id())
    }

    /// What the form shows now.
    pub fn view(&self) -> PresetView {
        let issues = self.draft.build().err().unwrap_or_default();
        let nodes = self.draft.fields();
        let irrelevant = self.draft.irrelevant();
        let whole = Whole {
            completed: &nodes,
            irrelevant: &irrelevant,
        };
        let mut fields = self.generator().fields(&nodes, None, &nodes, &whole);
        let mut unplaced = Vec::new();
        for issue in &issues {
            let text = issue_text(issue, &self.draft);
            if !place(&mut fields, issue.path(), &text) {
                unplaced.push(text);
            }
        }
        for (path, rejected) in self.rejected.iter() {
            place(&mut fields, path, &rejected.problem);
        }
        // the composite has the preset as its only field, which the form shows the fields of
        let (fields, mut issues_of_preset) = match fields.pop() {
            Some(FormField {
                input:
                    Input::Plugin(PluginInput {
                        chosen: Some(chosen),
                        ..
                    }),
                issues,
                ..
            }) => (chosen.fields, issues),
            _ => (Vec::new(), Vec::new()),
        };
        issues_of_preset.append(&mut unplaced);
        PresetView {
            fields,
            complete: issues.is_empty() && self.rejected.is_empty(),
            issues: issues_of_preset,
        }
    }

    /// The preset as configured; `None` while the form is not complete.
    pub(crate) fn preset(&self) -> Option<PresetChoice> {
        if !self.rejected.is_empty() {
            return None;
        }
        self.draft.build().ok()
    }

    /// The phases the preset would add to a competition without phases, in order, named in the
    /// language of `tr`; or why it adds none: the form is not complete, or the preset does not
    /// expand.
    ///
    /// # Errors
    /// Returns what keeps the preset from expanding.
    pub fn preview(&self, tr: &Translator) -> Result<Vec<PreviewPhase>, Text> {
        let preset = self
            .preset()
            .ok_or_else(|| Text::new("issue-preset-incomplete"))?;
        let mut next = 0;
        let competition = Competition::default();
        let phases = competition
            .expand(
                preset.preset(),
                || {
                    next += 1;
                    PhaseId::new(Uuid::from_u128(next))
                },
                |phrase| tr.preset_phase(self.id(), phrase),
            )
            .map_err(|error| {
                Text::new("error-preset-does-not-expand").with("reason", error.to_string())
            })?;
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

    /// What the fields of the draft are shown from.
    fn generator(&self) -> Generator<'_, PresetChoice> {
        Generator {
            draft: &self.draft,
            rejected: &self.rejected,
            offered: Draft::default_value,
        }
    }
}

impl Answering for PresetForm {
    type Of = PresetChoice;

    fn draft(&self) -> &Draft<PresetChoice> {
        &self.draft
    }

    fn rejected(&self) -> &Rejections<Path> {
        &self.rejected
    }

    fn rejected_mut(&mut self) -> &mut Rejections<Path> {
        &mut self.rejected
    }

    fn change(
        &mut self,
        _: &Path,
        change: impl FnOnce(&mut Draft<PresetChoice>) -> Result<(), ConfigError>,
    ) -> Result<(), ConfigError> {
        let mut draft = self.draft.clone();
        change(&mut draft)?;
        self.draft = draft.with_defaults()?;
        self.rejected.clear();
        Ok(())
    }
}
