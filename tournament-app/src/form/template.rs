//! The phases of a template as forms, refined before they are added, as the form of a phase is.

use plugin_factory::{
    catalog::AnyAxis,
    config::{Path, PluginConfig, Value},
};
use tournament_engine::{
    PhaseId, PhasePlugins, SourceOf,
    plugins::{
        draw::{DivisionOrder, Draw},
        phase_format::PhaseFormat,
        teams::{Keep, RandomPairs},
    },
};

use super::{Answering as _, Context, Groups, PhaseForm};
use crate::{
    error::EditPhasesError,
    session::{Dyp, MatchFormats, completed},
    view::{PhaseName, Plugin},
};

/// The phases of a template, each a form, refined before they are added, see
/// [`Session::template_form`](crate::session::Session::template_form). Each knows the phases
/// before it from the forms before it, as the form of a new phase knows those added, so a source
/// takes places of a phase not added yet.
#[derive(Clone)]
pub struct TemplateForm {
    template: String,
    ids: Vec<PhaseId>,
    forms: Vec<PhaseForm>,
    /// How often each form changed, see [`Self::revision`].
    revisions: Vec<u64>,
}

impl TemplateForm {
    /// The forms `forms` of the phases of the template `template`, each to be added with the id
    /// of `ids` at its position.
    pub(crate) fn new(template: &str, ids: Vec<PhaseId>, forms: Vec<PhaseForm>) -> Self {
        let mut form = Self {
            template: template.to_owned(),
            revisions: vec![0; forms.len()],
            ids,
            forms,
        };
        form.settle();
        form
    }

    /// The id of the template.
    pub fn template(&self) -> &str {
        &self.template
    }

    /// The ids the phases are added with, in order, which their sources name.
    pub fn ids(&self) -> &[PhaseId] {
        &self.ids
    }

    /// The forms of the phases, in order.
    pub fn forms(&self) -> &[PhaseForm] {
        &self.forms
    }

    /// How often the form at `index` was changed other than by [`Self::replace`]: by a change of
    /// another form it knows of, e.g. the name of a phase it takes places of, or by a setting of
    /// all forms. A screen editing a copy of it, which it hands to [`Self::replace`], takes it
    /// anew once this changes.
    ///
    /// # Panics
    /// Panics if there is no form at `index`.
    pub fn revision(&self, index: usize) -> u64 {
        self.revisions[index]
    }

    /// Whether every phase can be added as its form stands.
    pub fn complete(&self) -> bool {
        self.forms.iter().all(|form| form.view().complete)
    }

    /// Replaces the form at `index` with `form`, a changed copy of it, and lets the other forms
    /// know.
    ///
    /// # Panics
    /// Panics if there is no form at `index`.
    pub fn replace(&mut self, index: usize, form: PhaseForm) {
        self.forms[index] = form;
        self.settle();
    }

    /// What the phase at `index` is called, see [`PhaseName`].
    ///
    /// # Panics
    /// Panics if there is no form at `index`.
    pub fn name(&self, index: usize) -> PhaseName {
        let form = &self.forms[index];
        let id = form
            .draft()
            .answers()
            .get(PhasePlugins::FORMAT)
            .and_then(PluginConfig::from_answer)
            .map(|config| config.plugin().to_owned())
            .unwrap_or_default();
        let entry = AnyAxis::of::<dyn PhaseFormat>().entry(&id);
        let name = form.name().trim();
        PhaseName {
            number: index + 1,
            name: (!name.is_empty()).then(|| name.to_owned()),
            format: Plugin {
                id: entry.map_or("", |entry| entry.id()),
                label: entry.map_or("", |entry| entry.label()),
                description: entry.and_then(|entry| entry.description()),
            },
        }
    }

    /// Gives each form what it knows of the phases before it and of the places the others take,
    /// in order, so that each takes the teams the forms before it send as they stand.
    fn settle(&mut self) {
        for index in 0..self.forms.len() {
            let context = self.context(index);
            if self.forms[index].set_context(context) {
                self.revisions[index] += 1;
            }
        }
    }

    /// What the form at `index` knows of the competition: the forms before it, as phases.
    fn context(&self, index: usize) -> Context {
        let before = 0..index;
        Context {
            number: index + 1,
            earlier: before
                .clone()
                .map(|earlier| (self.ids[earlier], self.name(earlier)))
                .collect(),
            entrants_per_ranking: before
                .clone()
                .filter_map(|earlier| {
                    let size = self.forms[earlier].teams_of_one_size()?;
                    Some((self.ids[earlier], size))
                })
                .collect(),
            taken: (0..self.forms.len())
                .filter(|other| *other != index)
                .flat_map(|other| {
                    let name = self.name(other);
                    self.forms[other]
                        .sources()
                        .iter()
                        .map(move |source| (*source, name.clone()))
                })
                .collect(),
            registration_closed: false,
            groups: before
                .filter_map(|earlier| Some((self.ids[earlier], groups(&self.forms[earlier])?)))
                .collect(),
        }
    }

    /// The match format of the phases: [`MatchFormats::Same`] with the first form if all have
    /// the same with the same settings.
    pub fn match_formats(&self) -> MatchFormats {
        let answer = |form: &PhaseForm| {
            form.draft()
                .answers()
                .get(PhasePlugins::MATCH_FORMAT)
                .cloned()
        };
        let first = answer(&self.forms[0]);
        if self.forms.iter().any(|form| answer(form) != first) {
            return MatchFormats::Mixed;
        }
        MatchFormats::Same(Box::new(self.forms[0].clone()))
    }

    /// Plays every phase in the match format `form` answers, and returns whether that changed
    /// anything.
    ///
    /// # Errors
    /// Returns an error naming the first phase the match format does not fit, e.g. one allowing
    /// draws for a knockout; then no form changes.
    pub fn set_match_format(&mut self, form: &PhaseForm) -> Result<bool, EditPhasesError> {
        let Some(answer) = form
            .draft()
            .answers()
            .get(PhasePlugins::MATCH_FORMAT)
            .cloned()
        else {
            return Ok(false);
        };
        let all: Vec<_> = (0..self.forms.len()).collect();
        self.answer_all(&all, PhasePlugins::MATCH_FORMAT, &answer)
    }

    /// The forms of phases taking places of the registration, by their positions.
    fn of_registration(&self) -> Vec<usize> {
        (0..self.forms.len())
            .filter(|index| {
                self.forms[*index]
                    .sources()
                    .iter()
                    .any(|source| source.of() == SourceOf::Registration)
            })
            .collect()
    }

    /// Whether the phases taking places of the registration are played by drawn pairs; `None`
    /// where that is not offered: there is no such phase, or one of them cannot be played by
    /// pairs, e.g. a Monster DYP, which draws pairs of its own.
    pub fn dyp(&self) -> Option<Dyp> {
        let phases = self.of_registration();
        if phases.is_empty() {
            return None;
        }
        let pairs = Value::from(PluginConfig::of::<RandomPairs>());
        let fits = phases.iter().all(|index| {
            let mut draft = self.forms[*index].draft().clone();
            draft
                .set(&Path::ROOT.field(PhasePlugins::TEAMS), pairs.clone())
                .ok()
                .and_then(|()| draft.with_defaults().ok())
                .is_some_and(|draft| completed(&draft).is_ok())
        });
        if !fits {
            return None;
        }
        let teams: Vec<_> = phases
            .iter()
            .map(|index| {
                self.forms[*index]
                    .draft()
                    .answers()
                    .get(PhasePlugins::TEAMS)
                    .and_then(PluginConfig::from_answer)
                    .map(|config| config.plugin().to_owned())
            })
            .collect();
        let all = |id: &str| teams.iter().all(|team| team.as_deref() == Some(id));
        Some(if all(<RandomPairs as plugin_factory::PluginType>::ID) {
            Dyp::On
        } else if all(<Keep as plugin_factory::PluginType>::ID) {
            Dyp::Off
        } else {
            Dyp::Other
        })
    }

    /// Plays the phases taking places of the registration by drawn pairs if `on`, else everyone
    /// on their own, and returns whether that changed anything. The phases after them keep the
    /// teams they are sent.
    ///
    /// # Errors
    /// Returns an error naming the first phase that cannot be played so; then no form changes.
    pub fn set_dyp(&mut self, on: bool) -> Result<bool, EditPhasesError> {
        let teams = if on {
            PluginConfig::of::<RandomPairs>()
        } else {
            PluginConfig::of::<Keep>()
        };
        let phases = self.of_registration();
        self.answer_all(&phases, PhasePlugins::TEAMS, &teams.into())
    }

    /// Answers the field `field` of the forms at `indices` with `answer`, and returns whether
    /// that changed any. A form that could be added before must still be addable after.
    fn answer_all(
        &mut self,
        indices: &[usize],
        field: &str,
        answer: &Value,
    ) -> Result<bool, EditPhasesError> {
        let path = Path::ROOT.field(field);
        let mut changed = Vec::new();
        for &index in indices {
            let form = &self.forms[index];
            if form.draft().answers().get(field) == Some(answer) {
                continue;
            }
            let fitted = completed(form.draft()).is_ok();
            let mut edited = form.clone();
            let fits = edited
                .change(&path, |draft| draft.set(&path, answer.clone()))
                .is_ok()
                && (!fitted || completed(edited.draft()).is_ok());
            if !fits {
                return Err(EditPhasesError::DoesNotFit(self.name(index)));
            }
            changed.push((index, edited));
        }
        let any = !changed.is_empty();
        for (index, edited) in changed {
            self.forms[index] = edited;
            self.revisions[index] += 1;
        }
        self.settle();
        Ok(any)
    }
}

/// The groups side by side the phase `form` configures is played in, if it is played in several.
fn groups(form: &PhaseForm) -> Option<Groups> {
    let answers = form.draft().answers();
    let count = answers.get(PhasePlugins::DIVISIONS)?.as_u64()? as usize;
    let parallel = answers
        .get(PhasePlugins::DRAW)
        .and_then(PluginConfig::from_answer)
        .and_then(|config| config.build::<dyn Draw>().ok())
        .is_none_or(|draw| draw.order() == DivisionOrder::Parallel);
    (count > 1 && parallel).then_some(Groups { count, sizes: None })
}
