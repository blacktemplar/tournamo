//! What the basic view sets for the whole competition: the match format of every phase, and
//! whether the phases taking the registration are played by drawn pairs. Neither is saved: each
//! is a view of the phases not started, and setting it sets it on each of them, as one action.

use plugin_factory::{
    config::{Path, PluginConfig, Value},
    draft::Draft,
};
use tournament_engine::{
    AddPhase, CompetitionStateMachineExt as _, PhaseId, PhasePlugins, SourceOf,
    plugins::teams::{Keep, RandomPairs},
    undoable_state_machine::{LogEntry, LogEntryKind, TransitionId},
};

use super::{Session, phase::completed, rejection};
use crate::{error::EditPhasesError, form::PhaseForm};

/// The match format of the phases not started, see [`Session::match_formats`].
#[derive(Clone)]
pub enum MatchFormats {
    /// They have the same match format with the same settings: a form of the first of them,
    /// whose match format a screen shows, see [`PhaseForm::match_format_field`], and hands to
    /// [`Session::set_match_format`] once changed.
    Same(Box<PhaseForm>),
    /// They differ.
    Mixed,
}

/// Whether the phases taking the registration are played by drawn pairs, see [`Session::dyp`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dyp {
    /// Each forms drawn pairs.
    On,
    /// Each lets everyone play on their own.
    Off,
    /// They form other teams, or not all the same.
    Other,
}

impl Session {
    /// The phases not started, in order.
    fn pending(&self) -> Vec<PhaseId> {
        self.competition()
            .phases()
            .values()
            .filter(|phase| !phase.is_started())
            .map(|phase| phase.id())
            .collect()
    }

    /// The phases not started that take places of the registration, in order.
    fn of_registration(&self) -> Vec<PhaseId> {
        self.competition()
            .phases()
            .values()
            .filter(|phase| !phase.is_started())
            .filter(|phase| {
                phase
                    .sources()
                    .iter()
                    .any(|source| source.of() == SourceOf::Registration)
            })
            .map(|phase| phase.id())
            .collect()
    }

    /// The answer of the field `field` of the phase `id`.
    fn answer_of(&self, id: PhaseId, field: &str) -> Option<Value> {
        self.competition().phase(&id)?.config().get(field).cloned()
    }

    /// The match format of the phases not started; `None` if every phase has started, or there
    /// is none.
    pub fn match_formats(&self) -> Option<MatchFormats> {
        let pending = self.pending();
        let first = *pending.first()?;
        let format = self.answer_of(first, PhasePlugins::MATCH_FORMAT);
        if pending
            .iter()
            .any(|id| self.answer_of(*id, PhasePlugins::MATCH_FORMAT) != format)
        {
            return Some(MatchFormats::Mixed);
        }
        let form = self.phase_form_to_edit(first).ok()?;
        Some(MatchFormats::Same(Box::new(form)))
    }

    /// Plays every phase not started in the match format `form` answers, as one action, and
    /// returns whether that changed anything.
    ///
    /// # Errors
    /// Returns an error naming the first phase the match format does not fit, e.g. one allowing
    /// draws for a knockout, or if the engine refuses a phase; then no phase changes.
    pub fn set_match_format(&mut self, form: &PhaseForm) -> Result<bool, EditPhasesError> {
        let Some(answer) = form
            .draft()
            .answers()
            .get(PhasePlugins::MATCH_FORMAT)
            .cloned()
        else {
            return Ok(false);
        };
        let pending = self.pending();
        self.answer_phases(&pending, PhasePlugins::MATCH_FORMAT, &answer)
    }

    /// Whether the teams of a phase [`Self::dyp`] is about differ from what is expected of them,
    /// see [`PhaseForm::differs`].
    pub fn dyp_differs(&self) -> bool {
        self.of_registration().into_iter().any(|id| {
            self.phase_form_to_edit(id)
                .is_ok_and(|form| form.differs(PhasePlugins::TEAMS))
        })
    }

    /// Whether the phases not started that take places of the registration are played by drawn
    /// pairs; `None` where that is not offered: there is no such phase, or one of them cannot
    /// be played by pairs, e.g. a Monster DYP, which draws pairs of its own.
    pub fn dyp(&self) -> Option<Dyp> {
        let phases = self.of_registration();
        if phases.is_empty() {
            return None;
        }
        let pairs = Value::from(PluginConfig::of::<RandomPairs>());
        let fits = phases.iter().all(|id| {
            self.answered(*id, PhasePlugins::TEAMS, &pairs)
                .is_some_and(|draft| completed(&draft).is_ok())
        });
        if !fits {
            return None;
        }
        let teams: Vec<_> = phases
            .iter()
            .map(|id| {
                self.answer_of(*id, PhasePlugins::TEAMS)
                    .as_ref()
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

    /// Plays the phases not started that take places of the registration by drawn pairs if `on`,
    /// else everyone on their own, as one action, and returns whether that changed anything. The
    /// phases after them keep the teams they are sent.
    ///
    /// # Errors
    /// Returns an error naming the first phase that cannot be played so, or if the engine refuses
    /// a phase; then no phase changes.
    pub fn set_dyp(&mut self, on: bool) -> Result<bool, EditPhasesError> {
        let teams = if on {
            PluginConfig::of::<RandomPairs>()
        } else {
            PluginConfig::of::<Keep>()
        };
        let phases = self.of_registration();
        self.answer_phases(&phases, PhasePlugins::TEAMS, &teams.into())
    }

    /// The draft of the phase `id` with its field `field` answered `answer`, the fields of a
    /// plugin answering it as they offer; `None` if there is no such phase or the field does not
    /// take the answer.
    fn answered(&self, id: PhaseId, field: &str, answer: &Value) -> Option<Draft<PhasePlugins>> {
        let mut draft = self.phase_draft_to_edit(id).ok()?;
        draft.set(&Path::ROOT.field(field), answer.clone()).ok()?;
        draft.with_defaults().ok()
    }

    /// Answers the field `field` of each phase of `phases` with `answer`, as one action of
    /// those it changes, and returns whether it changed any.
    fn answer_phases(
        &mut self,
        phases: &[PhaseId],
        field: &str,
        answer: &Value,
    ) -> Result<bool, EditPhasesError> {
        let mut entries = Vec::new();
        for &id in phases {
            if self.answer_of(id, field).as_ref() == Some(answer) {
                continue;
            }
            let name = || {
                self.name_of_phase(id)
                    .expect("the phase is of the competition")
            };
            let plugins = self
                .answered(id, field, answer)
                .ok_or_else(|| EditPhasesError::DoesNotFit(name()))
                .and_then(|draft| {
                    completed(&draft).map_err(|_| EditPhasesError::DoesNotFit(name()))
                })?;
            let phase = self.competition().phase(&id).expect("a phase not started");
            let edited = AddPhase::new(id, plugins)
                .with_name(phase.name().map(str::to_owned))
                .with_sources(phase.sources().clone())
                .with_columns(phase.columns().clone());
            let target = self
                .machine
                .added_by(id)
                .expect("the phase of a session is added by an `AddPhase` in effect");
            entries.push(LogEntry::new(
                TransitionId::new(),
                LogEntryKind::Replace {
                    target,
                    with: edited.into(),
                },
            ));
        }
        if entries.is_empty() {
            return Ok(false);
        }
        self.history
            .apply_all(&mut self.machine, entries)
            .map_err(|error| EditPhasesError::Rejected(rejection(error)))?;
        Ok(true)
    }
}
