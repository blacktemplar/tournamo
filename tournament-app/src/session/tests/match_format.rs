//! The match format a new phase starts out at: that of the phase before, where it fits.

use plugin_factory::{Plugin, compatibility::Declaration, register};
use serde::{Deserialize, Serialize};
use tournament_engine::{
    DivisionData, Match, TypeMap,
    entities::details::Part,
    plugins::{
        Phrase,
        facts::NoDraws,
        match_format::{Level, MatchFormat, Template},
    },
};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;

/// A match format of the tests for phases whose every match has a winner, as a format of the
/// tests for a bracket's stages would be.
#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_app::decisive",
    label = "decisive",
    description = "a match format of the tests",
    declare_static = Self::declare_static
)]
struct Decisive;

impl Decisive {
    fn declare_static(declaration: &mut Declaration) {
        declaration.require::<NoDraws>(Phrase::new("no-draws", "no draws"));
    }
}

#[register]
impl MatchFormat for Decisive {
    fn template(&self, _: &DivisionData, _: &Match) -> Template {
        template()
    }

    fn tally(&self, _: &DivisionData, _: &Match) -> TypeMap {
        TypeMap::new()
    }

    fn suggest(&self, _: &Level, _: &Part) -> Option<MatchResult> {
        None
    }
}

/// The first version of a match format of the tests.
#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_app::plain",
    label = "plain",
    description = "a match format of the tests"
)]
struct Plain;

#[register]
impl MatchFormat for Plain {
    fn template(&self, _: &DivisionData, _: &Match) -> Template {
        template()
    }

    fn tally(&self, _: &DivisionData, _: &Match) -> TypeMap {
        TypeMap::new()
    }

    fn suggest(&self, _: &Level, _: &Part) -> Option<MatchResult> {
        None
    }
}

/// The second version of [`Plain`].
#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_app::plain",
    version = 2,
    label = "plain",
    description = "a match format of the tests"
)]
struct PlainAgain;

#[register]
impl MatchFormat for PlainAgain {
    fn template(&self, _: &DivisionData, _: &Match) -> Template {
        template()
    }

    fn tally(&self, _: &DivisionData, _: &Match) -> TypeMap {
        TypeMap::new()
    }

    fn suggest(&self, _: &Level, _: &Part) -> Option<MatchResult> {
        None
    }
}

/// A match with nothing entered but who won.
fn template() -> Template {
    Template {
        root: Level::new(Phrase::new("match", "match")),
    }
}

/// The match format of a new phase's form, with its settings.
fn match_format_of(form: &PhaseForm) -> Option<PluginConfig> {
    PluginConfig::from_answer(form.draft().answers().get(PhasePlugins::MATCH_FORMAT)?)
}

/// The match format of the phase `phase`, with its settings.
fn match_format_of_phase(session: &Session, phase: PhaseId) -> Option<PluginConfig> {
    match_format_of(&session.phase_form_to_edit(phase).unwrap())
}

#[test]
fn a_further_phase_starts_out_at_the_match_format_of_the_phase_before() {
    let mut session = Given::new()
        .match_format(PluginConfig::of::<BestOf>().with("games", 5))
        .added();

    let form = session.phase_form().unwrap();

    let carried = match_format_of(&form).unwrap();
    assert_eq!(carried.plugin(), BestOf::ID);
    assert_eq!(
        Some(carried),
        match_format_of_phase(&session, session.first())
    );
    session.add_phase(&form).unwrap();
}

/// The phase before is a knockout, whose match format needs a winner of every match; the round
/// robin a new phase offers allows draws.
#[test]
fn a_match_format_not_fitting_a_further_phase_is_not_carried_over() {
    let session = Given::new()
        .format(SingleElimination::ID)
        .match_format(PluginConfig::of::<Decisive>())
        .added();

    let form = session.phase_form().unwrap();

    assert_eq!(
        match_format_of(&form).unwrap().plugin(),
        "tournament_engine::winner_only"
    );
    assert!(form.draft().issues().is_empty());
}

/// Only the latest version of a match format is offered for a new phase.
#[test]
fn an_earlier_version_of_a_match_format_is_not_carried_over() {
    let session = Given::new()
        .match_format(PluginConfig::of::<Plain>())
        .added();

    let form = session.phase_form().unwrap();

    assert_eq!(
        match_format_of(&form).unwrap().plugin(),
        "tournament_engine::winner_only"
    );
}
