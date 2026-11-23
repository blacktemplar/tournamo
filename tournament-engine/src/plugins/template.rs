//! Templates of a whole competition: the [`Template`] axis and the templates of this crate.
//!
//! A template is a quick start: the phases of a competition, which applying it adds as ordinary
//! phases (see [`Competition::expand`]), refined and removed as any phase is. A template is never
//! saved: the phases added are, each with where it came from (its [`TemplateOrigin`]), so that
//! what differs from the template can be told.

use std::num::NonZeroU32;

use dyn_clone::DynClone;
use plugin_factory::{
    Plugin,
    catalog::AnyAxis,
    config::{Config, ConfigError, PluginConfig},
    draft::Issue,
    register,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use vec1::{Vec1, vec1};

use crate::{
    AddPhase, Competition, PhaseId, Source, SourceOf,
    entities::entrants_per_qualifier,
    plugins::{
        PhasePlugins, Phrase,
        phase_format::{
            DoubleElimination, MonsterDyp, RandomRounds, RoundRobin, SingleElimination,
        },
        seeding::GroupWinnersApart,
    },
};

/// A template of a whole competition: the phases it stands for. It has no fields: what it sets
/// up is refined in the phases it adds, which remember it by its id (see [`TemplateOrigin`]).
#[plugin_factory::axis]
pub trait Template: Plugin + DynClone {
    /// The phases this template stands for, in the order they are added.
    fn phases(&self) -> Vec1<TemplatePhase>;
}

dyn_clone::clone_trait_object!(Template);

impl dyn Template {
    /// The template registered as `id`, or `None` if there is none.
    pub fn of(id: &str) -> Option<Box<Self>> {
        AnyAxis::of::<Self>()
            .entry(id)?
            .default_config()
            .build::<Self>()
            .ok()
    }
}

/// One phase a [`Template`] stands for.
#[derive(Debug, Clone, PartialEq)]
pub struct TemplatePhase {
    /// What the phase is called, translated by the consumer; `None` for a phase named as one
    /// added without a name is, by its format.
    pub name: Option<Phrase>,
    /// Where it takes its entrants from, in this order.
    pub sources: Vec1<TemplateSource>,
    /// Answers to the draft of the phase, see [`Competition::phase_draft`]; what they leave open
    /// follows the suggestions of the format and the defaults, see
    /// [`PhasePlugins::with_suggestions`].
    pub answers: Config,
}

impl TemplatePhase {
    /// A phase of everyone registered, unnamed, answering the draft of its phase with `answers`.
    pub fn of_registration(answers: Config) -> Self {
        Self {
            name: None,
            sources: vec1![TemplateSource::everyone(TemplateSourceOf::Registration)],
            answers,
        }
    }

    /// This phase named `name`.
    #[must_use]
    pub fn named(self, name: Phrase) -> Self {
        Self {
            name: Some(name),
            ..self
        }
    }
}

/// Where a [`TemplatePhase`] takes entrants from: the places `from..=to`, to the last place if
/// `to` is `None`, see [`Source`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TemplateSource {
    pub of: TemplateSourceOf,
    pub from: NonZeroU32,
    pub to: Option<NonZeroU32>,
}

impl TemplateSource {
    /// Every place of `of`.
    pub const fn everyone(of: TemplateSourceOf) -> Self {
        Self {
            of,
            from: NonZeroU32::MIN,
            to: None,
        }
    }

    /// The places `1..=to` of `of`.
    pub const fn best(of: TemplateSourceOf, to: NonZeroU32) -> Self {
        Self {
            of,
            from: NonZeroU32::MIN,
            to: Some(to),
        }
    }
}

/// What a [`TemplateSource`] takes places of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplateSourceOf {
    /// The registration.
    Registration,
    /// A phase before it, by its position among the phases of the template, the first being 0.
    Phase(usize),
}

/// Where a phase added by a template came from: the phase at the position `phase` among the
/// phases of the template `template`, by its id. What the template is now, not what it was when
/// the phase was added, is what the phase is compared with.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TemplateOrigin {
    pub template: String,
    pub phase: usize,
}

/// Why the phases of a template do not expand, a bug of the template.
#[derive(Error, Debug)]
#[error("phase {} of the template: {problem}", .phase + 1)]
pub struct TemplateError {
    /// The position of the phase among the phases of the template.
    pub phase: usize,
    pub problem: TemplateProblem,
}

/// What is wrong with a phase of a template, see [`TemplateError`].
#[derive(Error, Debug)]
pub enum TemplateProblem {
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
    /// The phases `phases` of the template `template`, by its id (see [`Template::phases`]), to
    /// be added to this competition in this order, each remembering its [`TemplateOrigin`]: each
    /// with the id `ids` gives it next, named `name` of its name
    /// if it has one, built from its answers to the draft of its sources (see
    /// [`Self::phase_draft`]), what they leave open answered as the format suggests (see
    /// [`PhasePlugins::with_suggestions`]), with the plugins they imply (see
    /// [`Draft::add_implied`](plugin_factory::draft::Draft::add_implied)).
    ///
    /// Adding the phases is up to the caller, and may still be refused, e.g. if this competition
    /// has a phase taking places of the registration that one of them takes too.
    ///
    /// # Errors
    /// Returns a [`TemplateError`] if a phase does not expand: a bug of the template.
    pub fn expand(
        &self,
        template: &str,
        phases: &[TemplatePhase],
        mut ids: impl FnMut() -> PhaseId,
        name: impl Fn(&Phrase) -> String,
    ) -> Result<Vec<AddPhase>, TemplateError> {
        let mut expanded: Vec<(AddPhase, Option<NonZeroU32>)> = Vec::new();
        for (index, phase) in phases.iter().enumerate() {
            let error = |problem| TemplateError {
                phase: index,
                problem,
            };
            let sources = phase.sources.try_mapped_ref(|source| {
                let of = match source.of {
                    TemplateSourceOf::Registration => SourceOf::Registration,
                    TemplateSourceOf::Phase(earlier) => SourceOf::Phase(
                        expanded
                            .get(earlier)
                            .ok_or(error(TemplateProblem::LaterPhase(earlier)))?
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
            let mut draft = PhasePlugins::draft(per_qualifier)
                .with_answers(&phase.answers)
                .and_then(PhasePlugins::with_suggestions)
                .map_err(|config| error(TemplateProblem::Answers(config)))?;
            draft.add_implied();
            let plugins = draft
                .build()
                .map_err(|issues| error(TemplateProblem::Plugins(issues)))?;
            let per_ranking = plugins.teams().entrants_per_team(per_qualifier);
            let added = AddPhase::new(ids(), plugins)
                .with_name(phase.name.as_ref().map(&name))
                .with_sources(sources)
                .with_template(Some(TemplateOrigin {
                    template: template.to_owned(),
                    phase: index,
                }));
            expanded.push((added, per_ranking));
        }
        Ok(expanded.into_iter().map(|(added, _)| added).collect())
    }
}

/// `places` as a number of places, which a template gives as a literal of at least 1.
const fn places(places: u32) -> NonZeroU32 {
    match NonZeroU32::new(places) {
        Some(places) => places,
        None => panic!("a template takes at least one place"),
    }
}

/// The phase played in the format `F` with its suggestions, of everyone registered.
fn played_in<F: plugin_factory::PluginType>() -> Vec1<TemplatePhase> {
    vec1![TemplatePhase::of_registration(
        Config::new().with(PhasePlugins::FORMAT, PluginConfig::of::<F>())
    )]
}

/// Two groups, each a round robin, then a single elimination of the best four of them.
#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::template::groups_and_knockout",
    label = "groups and knockout",
    description = "everyone plays everyone else of their group, then the best play a knockout"
)]
pub struct GroupsAndKnockout;

#[register]
impl Template for GroupsAndKnockout {
    fn phases(&self) -> Vec1<TemplatePhase> {
        vec1![
            TemplatePhase::of_registration(
                Config::new()
                    .with(PhasePlugins::FORMAT, PluginConfig::of::<RoundRobin>())
                    .with(PhasePlugins::DIVISIONS, 2)
            )
            .named(Phrase::new("group-phase", "Group phase")),
            TemplatePhase {
                name: Some(Phrase::new("knockout", "Knockout")),
                // how many go on in total, which fills a bracket whatever the number of groups
                sources: vec1![TemplateSource::best(TemplateSourceOf::Phase(0), places(4))],
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

/// Random rounds for everyone, then a single elimination of the best eight.
#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::template::rounds_and_knockout",
    label = "rounds and knockout",
    description = "some rounds against random opponents rank everyone, then the best play a \
                   knockout"
)]
pub struct RoundsAndKnockout;

#[register]
impl Template for RoundsAndKnockout {
    fn phases(&self) -> Vec1<TemplatePhase> {
        vec1![
            TemplatePhase::of_registration(
                Config::new().with(PhasePlugins::FORMAT, PluginConfig::of::<RandomRounds>())
            )
            .named(Phrase::new("qualification", "Qualification")),
            TemplatePhase {
                name: Some(Phrase::new("knockout", "Knockout")),
                sources: vec1![TemplateSource::best(TemplateSourceOf::Phase(0), places(8))],
                answers: Config::new().with(
                    PhasePlugins::FORMAT,
                    PluginConfig::of::<SingleElimination>()
                ),
            },
        ]
    }
}

/// A single elimination of everyone.
#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::template::single_elimination",
    label = "single elimination",
    description = "a knockout: whoever loses a match is out"
)]
pub struct SingleEliminationTemplate;

#[register]
impl Template for SingleEliminationTemplate {
    fn phases(&self) -> Vec1<TemplatePhase> {
        played_in::<SingleElimination>()
    }
}

/// A double elimination of everyone.
#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::template::double_elimination",
    label = "double elimination",
    description = "a knockout in which whoever loses a match gets a second chance, and is out \
                   after a second loss"
)]
pub struct DoubleEliminationTemplate;

#[register]
impl Template for DoubleEliminationTemplate {
    fn phases(&self) -> Vec1<TemplatePhase> {
        played_in::<DoubleElimination>()
    }
}

/// A Monster DYP of everyone.
#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::template::monster_dyp",
    label = "Monster DYP",
    description = "rounds with a new partner drawn for every match, everyone ranked on their own"
)]
pub struct MonsterDypTemplate;

#[register]
impl Template for MonsterDypTemplate {
    fn phases(&self) -> Vec1<TemplatePhase> {
        played_in::<MonsterDyp>()
    }
}

/// A round robin of everyone.
#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::template::round_robin",
    label = "round robin",
    description = "everyone plays everyone else"
)]
pub struct RoundRobinTemplate;

#[register]
impl Template for RoundRobinTemplate {
    fn phases(&self) -> Vec1<TemplatePhase> {
        played_in::<RoundRobin>()
    }
}

/// Random rounds of everyone.
#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::template::random_rounds",
    label = "random rounds",
    description = "some rounds, each against an opponent drawn at random"
)]
pub struct RandomRoundsTemplate;

#[register]
impl Template for RandomRoundsTemplate {
    fn phases(&self) -> Vec1<TemplatePhase> {
        played_in::<RandomRounds>()
    }
}

#[cfg(test)]
mod tests;
