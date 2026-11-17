//! Presets expand into ordinary phases, which play through.

use std::num::NonZeroU32;

use plugin_factory::{
    Plugin, PluginType,
    config::{Config, ConfigError, PluginConfig},
    register,
};
use serde::{Deserialize, Serialize};
use tournament_engine::{
    AddEntrant, AddPhase, Competition, CompetitionStateMachine, Entrant, EntrantId, MatchResult,
    PhaseId, PhasePlugins, ReportMatchResult, Seed, SetCompetitionSeed, Source, SourceOf,
    StartCompetition, StartPhase,
    plugins::{
        phase_format::{DoubleElimination, RoundRobin},
        preset::{
            Dyp, GroupsThenKnockout, Preset, PresetChoice, PresetError, PresetPhase, PresetProblem,
            PresetSource, PresetSourceOf, QualificationThenKnockouts,
        },
        teams::RandomPairs,
    },
    uuid::Uuid,
};
use vec1::{Vec1, vec1};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

/// The preset `P` as it is configured by `answers`, the rest as it offers.
fn preset<P: PluginType>(answers: &[(&str, u32)]) -> Box<dyn Preset> {
    let config = answers
        .iter()
        .fold(PluginConfig::of::<P>(), |config, (field, answer)| {
            config.with(*field, *answer)
        });
    let choice = PresetChoice::draft()
        .with(PresetChoice::PRESET, config)
        .and_then(|draft| draft.with_defaults())
        .unwrap()
        .build()
        .unwrap();
    dyn_clone::clone_box(choice.preset())
}

/// The phases `preset` expands into in `competition`, with ids counting up from 1.
fn expand(competition: &Competition, preset: &dyn Preset) -> Result<Vec<AddPhase>, PresetError> {
    let mut next = 0;
    competition.expand(
        preset,
        || {
            next += 1;
            PhaseId::new(Uuid::from_u128(next))
        },
        |name| format!("{name}!"),
    )
}

/// A competition of `entrants` entrants with the phases of `preset`.
fn set_up(entrants: u128, preset: &dyn Preset) -> CompetitionStateMachine {
    let mut machine = CompetitionStateMachine::default();
    machine
        .apply_new(SetCompetitionSeed::new(Seed::new(*b"presets!....")).into())
        .unwrap();
    for entrant in 0..entrants {
        let entrant = Entrant::new(EntrantId::new(Uuid::from_u128(entrant)));
        machine.apply_new(AddEntrant::new(entrant).into()).unwrap();
    }
    for phase in expand(machine.current(), preset).unwrap() {
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
fn groups_then_a_knockout_of_the_best_of_each_group() {
    let mut machine = set_up(12, &*preset::<GroupsThenKnockout>(&[("groups", 3)]));
    let competition = machine.current();
    let [groups, knockout] = competition.phases().values().collect::<Vec<_>>()[..] else {
        panic!("two phases");
    };
    assert_eq!(groups.name(), Some("Group phase!"));
    assert_eq!(groups.plugins().divisions().get(), 3);
    assert_eq!(knockout.name(), Some("Knockout!"));
    assert_eq!(
        knockout.sources().as_slice(),
        [Source::new(
            SourceOf::Phase(groups.id()),
            place(1),
            Some(place(6))
        )]
    );
    play_through(&mut machine);
    assert_eq!(places(&machine).len(), 12);
    assert_eq!(places(&machine)[..2], [1, 2]);
}

#[test]
fn a_qualification_then_knockouts_in_blocks() {
    let mut machine = set_up(12, &*preset::<QualificationThenKnockouts>(&[]));
    let knockouts = machine.current().phases().values().nth(1).unwrap();
    assert_eq!(knockouts.plugins().divisions().get(), 2);
    play_through(&mut machine);
    assert_eq!(places(&machine).len(), 12);
    // the best of the second knockout is placed after everyone of the first
    assert_eq!(places(&machine)[6], 7);
}

#[test]
fn a_dyp_of_drawn_pairs() {
    let mut machine = set_up(12, &*preset::<Dyp>(&[]));
    play_through(&mut machine);
    let ranking = machine.current().ranking();
    assert_eq!(ranking.len(), 12);
    // partners share their place
    assert_eq!(ranking[0].place, ranking[1].place);
}

/// A preset of the phases `phases`, for the refusals of expanding it.
#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(id = "presets::broken", label = "broken")]
struct Broken {
    #[serde(skip)]
    #[plugin(skip)]
    phases: Vec<(Vec1<PresetSource>, Config)>,
}

#[register]
impl Preset for Broken {
    fn phases(&self) -> Vec1<PresetPhase> {
        Vec1::try_from_vec(
            self.phases
                .iter()
                .map(|(sources, answers)| PresetPhase {
                    name: tournament_engine::plugins::Phrase::new("broken", "Broken"),
                    sources: sources.clone(),
                    answers: answers.clone(),
                })
                .collect(),
        )
        .unwrap()
    }
}

fn of_registration() -> Vec1<PresetSource> {
    vec1![PresetSource::everyone(PresetSourceOf::Registration)]
}

#[test]
fn a_source_taking_places_of_a_phase_not_before_it_does_not_expand() {
    let broken = Broken {
        phases: vec![
            (of_registration(), Config::new()),
            (
                vec1![PresetSource::everyone(PresetSourceOf::Phase(1))],
                Config::new(),
            ),
        ],
    };
    let error = expand(&Competition::default(), &broken).unwrap_err();
    assert_eq!(error.phase, 1);
    assert!(matches!(error.problem, PresetProblem::LaterPhase(1)));
}

#[test]
fn answers_not_fitting_the_draft_do_not_expand() {
    let broken = Broken {
        phases: vec![(
            of_registration(),
            Config::new().with(PhasePlugins::DIVISIONS, "two"),
        )],
    };
    let error = expand(&Competition::default(), &broken).unwrap_err();
    assert_eq!(error.phase, 0);
    assert!(matches!(
        error.problem,
        PresetProblem::Answers(ConfigError::WrongKind { .. })
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
    let broken = Broken {
        phases: vec![
            (of_registration(), pairs.clone()),
            (
                vec1![PresetSource::everyone(PresetSourceOf::Phase(0))],
                pairs.with(PhasePlugins::FORMAT, PluginConfig::of::<RoundRobin>()),
            ),
        ],
    };
    let error = expand(&Competition::default(), &broken).unwrap_err();
    assert_eq!(error.phase, 1);
    assert!(matches!(error.problem, PresetProblem::Plugins(_)));
}
