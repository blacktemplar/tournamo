//! Templates of a whole competition: the [`Preset`] axis and the presets of this crate.
//!
//! A preset stands for one or more phases. It is never saved: applying it adds ordinary phases
//! (see [`Competition::expand`]), which are edited and removed as any phase is.

use std::num::NonZeroU32;

use dyn_clone::DynClone;
use plugin_factory::{
    Plugin, axis,
    config::{Config, ConfigError, PluginConfig},
    draft::{Draft, Issue},
    register,
    schema::Configurable,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use vec1::{Vec1, vec1};

use crate::{
    AddPhase, Competition, PhaseId, Source, SourceOf,
    entities::entrants_per_qualifier,
    plugins::{
        PhasePlugins, Phrase,
        draw::Blocks,
        match_format::{self, MatchFormat},
        phase_format::{
            DoubleElimination, PhaseSuggestions, RandomRounds, RoundRobin, SingleElimination,
        },
        seeding::GroupWinnersApart,
        teams::RandomPairs,
    },
};

/// A template of a whole competition: the phases it stands for, configured by the fields of the
/// preset. Its id names it for translations only, since a preset is never saved.
#[axis]
pub trait Preset: Plugin + DynClone {
    /// The phases this preset stands for, in the order they are added.
    fn phases(&self) -> Vec1<PresetPhase>;
}

dyn_clone::clone_trait_object!(Preset);

/// One phase a [`Preset`] stands for.
#[derive(Debug, Clone, PartialEq)]
pub struct PresetPhase {
    /// What the phase is called, translated by the consumer.
    pub name: Phrase,
    /// Where it takes its entrants from, in this order.
    pub sources: Vec1<PresetSource>,
    /// Answers to the draft of the phase, see [`Competition::phase_draft`]; what they leave open
    /// follows the match format chosen with the preset (see [`PresetChoice::match_format`]), the
    /// suggestions of the format and the defaults, see [`PhasePlugins::with_suggestions`].
    pub answers: Config,
}

/// Where a [`PresetPhase`] takes entrants from: the places `from..=to`, to the last place if
/// `to` is `None`, see [`Source`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresetSource {
    pub of: PresetSourceOf,
    pub from: NonZeroU32,
    pub to: Option<NonZeroU32>,
}

impl PresetSource {
    /// Every place of `of`.
    pub const fn everyone(of: PresetSourceOf) -> Self {
        Self {
            of,
            from: NonZeroU32::MIN,
            to: None,
        }
    }
}

/// What a [`PresetSource`] takes places of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetSourceOf {
    /// The registration.
    Registration,
    /// A phase before it, by its position among the phases of the preset, the first being 0.
    Phase(usize),
}

/// A preset as a composite to configure: a [`Draft`] of it answers which preset and its fields,
/// and the match format of its phases, as a [`PhasePlugins`] draft answers the plugins of a phase.
#[derive(Debug, Clone, Serialize, Deserialize, Configurable)]
pub struct PresetChoice {
    #[plugin(
        label = "preset",
        description = "the phases the competition is played in"
    )]
    preset: Box<dyn Preset>,
    // the label and description of the field of a phase, which a consumer translates once
    #[plugin(
        label = "match format",
        description = "what is entered for a match",
        default = match_format::default()
    )]
    match_format: Box<dyn MatchFormat>,
}

impl PresetChoice {
    /// A draft with nothing answered.
    pub fn draft() -> Draft<Self> {
        Draft::new(|_| {})
    }

    /// The draft of the preset `id`, configured as it offers.
    ///
    /// # Errors
    /// Returns [`ConfigError::UnknownPlugin`] if no preset of this id is registered.
    pub fn draft_of(id: &str) -> Result<Draft<Self>, ConfigError> {
        Self::draft()
            .with(Self::PRESET, PluginConfig::new(id))?
            .with_defaults()
    }

    /// The preset `preset`, its phases played in the default match format where they leave it
    /// open, see [`Competition::expand`].
    pub fn new(preset: Box<dyn Preset>) -> Self {
        Self {
            preset,
            match_format: match_format::default(),
        }
    }

    /// This choice with its phases played in `match_format` where they leave it open and fit it.
    #[must_use]
    pub fn with_match_format(self, match_format: Box<dyn MatchFormat>) -> Self {
        Self {
            match_format,
            ..self
        }
    }

    pub fn preset(&self) -> &dyn Preset {
        &*self.preset
    }

    /// The match format of the phases that leave it open, see [`Competition::expand`].
    pub fn match_format(&self) -> &dyn MatchFormat {
        &*self.match_format
    }
}

/// Why a [`Preset`] does not expand, a bug of the preset.
#[derive(Error, Debug)]
#[error("phase {} of the preset: {problem}", .phase + 1)]
pub struct PresetError {
    /// The position of the phase among the phases of the preset.
    pub phase: usize,
    pub problem: PresetProblem,
}

/// What is wrong with a phase of a [`Preset`], see [`PresetError`].
#[derive(Error, Debug)]
pub enum PresetProblem {
    /// A source takes places of the phase at this position, which is not before it.
    #[error("a source takes places of a phase that is not before it")]
    LaterPhase(usize),
    /// The answers do not fit the draft of the phase.
    #[error("the answers do not fit: {0}")]
    Answers(ConfigError),
    /// The plugins are not complete or do not fit together.
    #[error("the plugins do not fit together")]
    Plugins(Vec<Issue>),
}

impl Competition {
    /// The phases the preset of `choice` stands for, to be added to this competition in this
    /// order: each with the id `ids` gives it next, named `name` of its name, built from its
    /// answers to the draft of its sources (see [`Self::phase_draft`]), the match format of
    /// `choice` where they leave it open and the phase fits it, what they leave open besides
    /// answered as the format suggests (see [`PhasePlugins::with_suggestions`]), with the plugins
    /// they imply (see [`Draft::add_implied`]).
    ///
    /// Adding the phases is up to the caller, and may still be refused, e.g. if this competition
    /// has a phase taking places of the registration that one of them takes too.
    ///
    /// # Errors
    /// Returns a [`PresetError`] if a phase does not expand: a bug of the preset.
    pub fn expand(
        &self,
        choice: &PresetChoice,
        mut ids: impl FnMut() -> PhaseId,
        name: impl Fn(&Phrase) -> String,
    ) -> Result<Vec<AddPhase>, PresetError> {
        let match_format = Config::of(choice)
            .expect("every plugin serializes as it is configured")
            .get(PresetChoice::MATCH_FORMAT)
            .cloned()
            .expect("a preset choice has a match format");
        let mut expanded: Vec<(AddPhase, Option<NonZeroU32>)> = Vec::new();
        for (index, phase) in choice.preset().phases().into_iter().enumerate() {
            let error = |problem| PresetError {
                phase: index,
                problem,
            };
            let sources = phase.sources.try_mapped_ref(|source| {
                let of = match source.of {
                    PresetSourceOf::Registration => SourceOf::Registration,
                    PresetSourceOf::Phase(earlier) => SourceOf::Phase(
                        expanded
                            .get(earlier)
                            .ok_or(error(PresetProblem::LaterPhase(earlier)))?
                            .0
                            .id(),
                    ),
                };
                Ok(Source::new(of, source.from, source.to))
            })?;
            let per_qualifier = entrants_per_qualifier(&sources, |id| {
                expanded
                    .iter()
                    .find(|(added, _)| added.id() == *id)
                    .map_or_else(
                        || self.entrants_per_ranking(id),
                        |&(_, per_ranking)| per_ranking,
                    )
            });
            let built = |answers: &Config| {
                let mut draft = PhasePlugins::draft(per_qualifier)
                    .with_answers(answers)
                    .and_then(PhasePlugins::with_suggestions)
                    .map_err(|config| error(PresetProblem::Answers(config)))?;
                draft.add_implied();
                draft
                    .build()
                    .map_err(|issues| error(PresetProblem::Plugins(issues)))
            };
            let chosen = phase
                .answers
                .get(PhasePlugins::MATCH_FORMAT)
                .is_none()
                .then(|| {
                    built(
                        &phase
                            .answers
                            .clone()
                            .with(PhasePlugins::MATCH_FORMAT, match_format.clone()),
                    )
                    .ok()
                })
                .flatten();
            let plugins = match chosen {
                Some(plugins) => plugins,
                None => built(&phase.answers)?,
            };
            let per_ranking = plugins.teams().entrants_per_team(per_qualifier);
            let added = AddPhase::new(ids(), plugins)
                .with_name(Some(name(&phase.name)))
                .with_sources(sources);
            expanded.push((added, per_ranking));
        }
        Ok(expanded.into_iter().map(|(added, _)| added).collect())
    }
}

/// The places `1..=to` of the phase at the position `phase` of the preset.
fn best_of(phase: usize, to: NonZeroU32) -> PresetSource {
    PresetSource {
        to: Some(to),
        ..PresetSource::everyone(PresetSourceOf::Phase(phase))
    }
}

/// `value` as a positive number, which a field of at least 1 holds.
fn positive(value: u32) -> NonZeroU32 {
    NonZeroU32::new(value).expect("the field takes no 0")
}

/// Groups, each a round robin, then a single elimination of the best of each group.
#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::groups_then_knockout",
    label = "groups then knockout",
    description = "everyone plays everyone else of their group, then the best of each group play \
                   a knockout"
)]
pub struct GroupsThenKnockout {
    #[plugin(label = "groups", min = 1, default = 2)]
    groups: u32,
    #[plugin(label = "advancing per group", min = 1, default = 2)]
    advancing: u32,
}

#[register]
impl Preset for GroupsThenKnockout {
    fn phases(&self) -> Vec1<PresetPhase> {
        let groups = positive(self.groups);
        vec1![
            PresetPhase {
                name: Phrase::new("group-phase", "Group phase"),
                sources: vec1![PresetSource::everyone(PresetSourceOf::Registration)],
                answers: Config::new()
                    .with(PhasePlugins::FORMAT, PluginConfig::of::<RoundRobin>())
                    .with(PhasePlugins::DIVISIONS, groups.get())
                    .with(PhasePlugins::ACROSS, PhaseSuggestions::across_by_points()),
            },
            PresetPhase {
                name: Phrase::new("knockout", "Knockout"),
                sources: vec1![best_of(0, groups.saturating_mul(positive(self.advancing)))],
                answers: Config::new()
                    .with(
                        PhasePlugins::FORMAT,
                        PluginConfig::of::<SingleElimination>()
                    )
                    .with(
                        PhasePlugins::SEEDING,
                        PluginConfig::of::<GroupWinnersApart>()
                    ),
            },
        ]
    }
}

/// Random rounds for everyone, then everyone plays on in single eliminations by their places.
#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::qualification_then_knockouts",
    label = "qualification then knockouts",
    description = "some rounds against random opponents rank everyone, who then play knockouts \
                   by their places: the best in the first, the next in the second, and so on"
)]
pub struct QualificationThenKnockouts {
    #[plugin(label = "rounds", min = 1, default = 5)]
    rounds: u32,
    #[plugin(label = "knockouts", min = 1, default = 2)]
    knockouts: u32,
}

#[register]
impl Preset for QualificationThenKnockouts {
    fn phases(&self) -> Vec1<PresetPhase> {
        vec1![
            PresetPhase {
                name: Phrase::new("qualification", "Qualification"),
                sources: vec1![PresetSource::everyone(PresetSourceOf::Registration)],
                answers: Config::new().with(
                    PhasePlugins::FORMAT,
                    PluginConfig::of::<RandomRounds>().with("rounds", self.rounds),
                ),
            },
            PresetPhase {
                name: Phrase::new("knockouts", "Knockouts"),
                sources: vec1![PresetSource::everyone(PresetSourceOf::Phase(0))],
                answers: Config::new()
                    .with(
                        PhasePlugins::FORMAT,
                        PluginConfig::of::<SingleElimination>()
                    )
                    .with(PhasePlugins::DIVISIONS, positive(self.knockouts).get())
                    .with(PhasePlugins::DRAW, PluginConfig::of::<Blocks>()),
            },
        ]
    }
}

/// Drawn pairs playing a double elimination.
#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::dyp",
    label = "DYP",
    description = "draw your partner: the entrants are drawn into pairs, which play a double \
                   elimination"
)]
pub struct Dyp;

#[register]
impl Preset for Dyp {
    fn phases(&self) -> Vec1<PresetPhase> {
        vec1![PresetPhase {
            name: Phrase::new("dyp", "DYP"),
            sources: vec1![PresetSource::everyone(PresetSourceOf::Registration)],
            answers: Config::new()
                .with(
                    PhasePlugins::FORMAT,
                    PluginConfig::of::<DoubleElimination>()
                )
                .with(PhasePlugins::TEAMS, PluginConfig::of::<RandomPairs>()),
        }]
    }
}

#[cfg(test)]
mod tests;
