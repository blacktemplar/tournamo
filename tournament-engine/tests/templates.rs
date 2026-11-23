//! Templates expand into ordinary phases, which play through.

use std::num::NonZeroU32;

use plugin_factory::{
    PluginType,
    config::{Config, ConfigError, PluginConfig},
};
use tournament_engine::{
    AddEntrant, AddPhase, Competition, CompetitionStateMachine, Entrant, EntrantId, MatchResult,
    PhaseId, PhasePlugins, ReportMatchResult, Seed, SetCompetitionSeed, Source, SourceOf,
    StartCompetition, StartPhase,
    plugins::{
        phase_format::{DoubleElimination, RoundRobin},
        seeding::GroupWinnersApart,
        teams::RandomPairs,
        template::{
            GroupsAndKnockout, RoundRobinTemplate, RoundsAndKnockout, Template, TemplateError,
            TemplatePhase, TemplateProblem, TemplateSource, TemplateSourceOf,
        },
    },
    uuid::Uuid,
};
use vec1::{Vec1, vec1};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

/// The phases of the template `T`.
fn phases_of<T: PluginType>() -> Vec<TemplatePhase> {
    <dyn Template>::of(T::ID).unwrap().phases().into_vec()
}

/// The phases `phases` expand into in `competition`, with ids counting up from 1.
fn expand(
    competition: &Competition,
    phases: &[TemplatePhase],
) -> Result<Vec<AddPhase>, TemplateError> {
    let mut next = 0;
    competition.expand(
        phases,
        || {
            next += 1;
            PhaseId::new(Uuid::from_u128(next))
        },
        |name| format!("{name}!"),
    )
}

/// A competition of `entrants` entrants with the phases `phases`.
fn set_up(entrants: u128, phases: &[TemplatePhase]) -> CompetitionStateMachine {
    let mut machine = CompetitionStateMachine::default();
    machine
        .apply_new(SetCompetitionSeed::new(Seed::new(*b"templates!..")).into())
        .unwrap();
    for entrant in 0..entrants {
        let entrant = Entrant::new(EntrantId::new(Uuid::from_u128(entrant)));
        machine.apply_new(AddEntrant::new(entrant).into()).unwrap();
    }
    for phase in expand(machine.current(), phases).unwrap() {
        machine.apply_new(phase.into()).unwrap();
    }
    machine
}

/// Starts the competition and plays every phase to its end in order, side A winning every match.
fn play_through(machine: &mut CompetitionStateMachine) {
    machine
        .apply_new(StartCompetition::default().into())
        .unwrap();
    let phases: Vec<PhaseId> = machine.current().phases().keys().copied().collect();
    for phase_id in phases {
        if !machine.current().phases()[&phase_id].is_started() {
            machine.apply_new(StartPhase::new(phase_id).into()).unwrap();
        }
        while let Some(open) = machine.current().phases()[&phase_id]
            .divisions()
            .iter()
            .flat_map(|division| division.data().matches().values())
            .find(|r#match| !r#match.is_finished())
            .map(|r#match| r#match.id())
        {
            machine
                .apply_new(ReportMatchResult::new(phase_id, open, MatchResult::AWins).into())
                .unwrap();
        }
        assert!(machine.current().phases()[&phase_id].is_finished());
    }
}

/// The places of the competition ranking, in order.
fn places(machine: &CompetitionStateMachine) -> Vec<u32> {
    machine
        .current()
        .ranking()
        .iter()
        .map(|placed| placed.place.get())
        .collect()
}

fn place(place: u32) -> NonZeroU32 {
    NonZeroU32::new(place).unwrap()
}

#[test]
fn groups_then_a_knockout_of_the_best_four_in_total() {
    let mut machine = set_up(12, &phases_of::<GroupsAndKnockout>());
    let competition = machine.current();
    let [groups, knockout] = competition.phases().values().collect::<Vec<_>>()[..] else {
        panic!("two phases");
    };
    assert_eq!(groups.name(), Some("Group phase!"));
    assert_eq!(groups.plugins().divisions().get(), 2);
    assert!(
        !groups.plugins().across().is_empty(),
        "the groups are compared as the format suggests"
    );
    assert_eq!(knockout.name(), Some("Knockout!"));
    assert_eq!(
        knockout.sources().as_slice(),
        [Source::new(
            SourceOf::Phase(groups.id()),
            place(1),
            Some(place(4))
        )]
    );
    assert_eq!(
        knockout.plugins().seeding().id(),
        <GroupWinnersApart as PluginType>::ID
    );
    play_through(&mut machine);
    assert_eq!(places(&machine).len(), 12);
    assert_eq!(places(&machine)[..2], [1, 2]);
}

#[test]
fn rounds_then_a_knockout_of_the_best_eight() {
    let mut machine = set_up(12, &phases_of::<RoundsAndKnockout>());
    let knockout = machine.current().phases().values().nth(1).unwrap();
    assert_eq!(knockout.sources()[0].to(), Some(place(8)));
    play_through(&mut machine);
    assert_eq!(places(&machine).len(), 12);
}

/// A template of one phase leaves it unnamed, as a phase added by hand.
#[test]
fn the_phase_of_a_template_of_one_format_is_unnamed() {
    let machine = set_up(4, &phases_of::<RoundRobinTemplate>());
    let [phase] = machine.current().phases().values().collect::<Vec<_>>()[..] else {
        panic!("one phase");
    };
    assert_eq!(phase.name(), None);
}

/// The phases of a broken template, each taking its entrants from its sources and answering the
/// draft of its phase with its answers.
fn broken(phases: Vec<(Vec1<TemplateSource>, Config)>) -> Vec<TemplatePhase> {
    phases
        .into_iter()
        .map(|(sources, answers)| TemplatePhase {
            name: None,
            sources,
            answers,
        })
        .collect()
}

fn of_registration() -> Vec1<TemplateSource> {
    vec1![TemplateSource::everyone(TemplateSourceOf::Registration)]
}

#[test]
fn a_source_taking_places_of_a_phase_not_before_it_does_not_expand() {
    let phases = broken(vec![
        (of_registration(), Config::new()),
        (
            vec1![TemplateSource::everyone(TemplateSourceOf::Phase(1))],
            Config::new(),
        ),
    ]);
    let error = expand(&Competition::default(), &phases).unwrap_err();
    assert_eq!(error.phase, 1);
    assert!(matches!(error.problem, TemplateProblem::LaterPhase(1)));
}

#[test]
fn answers_not_fitting_the_draft_do_not_expand() {
    let phases = broken(vec![(
        of_registration(),
        Config::new().with(PhasePlugins::DIVISIONS, "two"),
    )]);
    let error = expand(&Competition::default(), &phases).unwrap_err();
    assert_eq!(error.phase, 0);
    assert!(matches!(
        error.problem,
        TemplateProblem::Answers(ConfigError::WrongKind { .. })
    ));
}

#[test]
fn plugins_not_fitting_together_do_not_expand() {
    // drawn pairs need single entrants, and the phase before sends pairs
    let pairs = Config::new()
        .with(
            PhasePlugins::FORMAT,
            PluginConfig::of::<DoubleElimination>(),
        )
        .with(PhasePlugins::TEAMS, PluginConfig::of::<RandomPairs>());
    let phases = broken(vec![
        (of_registration(), pairs.clone()),
        (
            vec1![TemplateSource::everyone(TemplateSourceOf::Phase(0))],
            pairs.with(PhasePlugins::FORMAT, PluginConfig::of::<RoundRobin>()),
        ),
    ]);
    let error = expand(&Competition::default(), &phases).unwrap_err();
    assert_eq!(error.phase, 1);
    assert!(matches!(error.problem, TemplateProblem::Plugins(_)));
}
