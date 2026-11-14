//! Competitions of several phases: each takes its entrants from the registration or from places of
//! the overall standings of earlier phases, is started once those are finished, and the
//! competition places every entrant by the latest phase they reached.

use std::num::NonZeroU32;

use itertools::Itertools;
use tournament_engine::{
    AddEntrant, AddPhase, Competition, CompetitionStateMachine, CompetitionStateMachineExt,
    CompetitionTransition, Entrant, EntrantId, MatchId, MatchResult, Origin, PhaseId, PhasePlugins,
    PhasePluginsBuilder, RankingId, ReportMatchResult, Score, Seed, SetCompetitionSeed, Source,
    SourceOf, StartCompetition, StartPhase,
    plugins::{
        metrics_calculator::{
            LaterFirstLoss, MorePoints, PointsCalculator, RoundOfFirstLossCalculator,
            WinsDrawsLossesCalculator,
        },
        phase_format::SingleElimination,
    },
    transitions::{
        AddPhaseError, Change, ConflictsError, RemovePhaseError, StartCompetitionError,
        StartPhaseError, UndoMatchResultError,
    },
    undoable_state_machine::{SessionHistory, Transition, TransitionId},
    uuid::Uuid,
};
use vec1::{Vec1, vec1};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

const SEED: Seed = Seed::new(*b"multi-phase!");

fn phase(id: u128) -> PhaseId {
    PhaseId::new(Uuid::from_u128(id))
}

fn place(place: u32) -> NonZeroU32 {
    NonZeroU32::new(place).unwrap()
}

/// The places `from..=to` of the phase `of`, to its last place if `to` is `None`.
fn places(of: PhaseId, from: u32, to: Option<u32>) -> Source {
    Source::new(SourceOf::Phase(of), place(from), to.map(place))
}

/// A round robin ranked by points.
fn round_robin() -> PhasePlugins {
    PhasePluginsBuilder::new()
        .add_ranking(MorePoints)
        .add_metric(WinsDrawsLossesCalculator)
        .add_metric(PointsCalculator::new(Score::ONE, Score::new(0.5)))
        .build()
        .unwrap()
}

/// A knockout ranked by the round of the first loss.
fn knockout() -> PhasePlugins {
    PhasePluginsBuilder::new()
        .format(SingleElimination::new())
        .add_ranking(LaterFirstLoss)
        .add_metric(RoundOfFirstLossCalculator)
        .build()
        .unwrap()
}

/// A seeded competition of `entrants` entrants, no phase yet.
fn registered(entrants: u128) -> CompetitionStateMachine {
    let mut machine = CompetitionStateMachine::default();
    machine
        .apply_new(SetCompetitionSeed::new(SEED).into())
        .unwrap();
    for entrant in 0..entrants {
        let entrant = Entrant::new(EntrantId::new(Uuid::from_u128(entrant)));
        machine.apply_new(AddEntrant::new(entrant).into()).unwrap();
    }
    machine
}

#[track_caller]
fn add(machine: &mut CompetitionStateMachine, phase: AddPhase) -> TransitionId {
    machine.apply_new(phase.into()).expect("the phase is added")
}

/// The phase `id` of `plugins`, taking its entrants from `sources`.
fn phase_of(id: PhaseId, sources: Vec1<Source>, plugins: PhasePlugins) -> AddPhase {
    AddPhase::new(id, plugins).with_sources(sources)
}

/// The open matches of the phase `phase_id`, in the order they were scheduled.
fn open_matches(machine: &CompetitionStateMachine, phase_id: PhaseId) -> Vec<MatchId> {
    machine.current().phases()[&phase_id]
        .divisions()
        .iter()
        .flat_map(|division| division.data().matches().values())
        .filter(|r#match| !r#match.is_finished())
        .map(|r#match| r#match.id())
        .collect()
}

/// Reports the first open match of the phase `phase_id` as won by the team with the better lot,
/// so that a round robin ends in the order of the lots, without a tie.
#[track_caller]
fn report_one(machine: &mut CompetitionStateMachine, phase_id: PhaseId) -> TransitionId {
    let open = open_matches(machine, phase_id)[0];
    let division = machine.current().phases()[&phase_id]
        .division_of(&open)
        .unwrap()
        .1
        .data();
    let r#match = &division.matches()[&open];
    let lot = |team: &Vec1<RankingId>| division.rankings()[&team[0]].lot();
    let result = if lot(r#match.team_a()) < lot(r#match.team_b()) {
        MatchResult::AWins
    } else {
        MatchResult::BWins
    };
    machine
        .apply_new(ReportMatchResult::new(phase_id, open, result).into())
        .expect("an open match is reported")
}

/// Reports every match of the phase `phase_id` until it is finished, see [`report_one`].
fn play_out(machine: &mut CompetitionStateMachine, phase_id: PhaseId) {
    while !open_matches(machine, phase_id).is_empty() {
        report_one(machine, phase_id);
    }
    assert!(machine.current().phases()[&phase_id].is_finished());
}

#[track_caller]
fn start(machine: &mut CompetitionStateMachine, phase_id: PhaseId) -> TransitionId {
    machine
        .apply_new(StartPhase::new(phase_id).into())
        .expect("the phase starts")
}

/// The entrants of the overall standings of the phase `phase_id`, best first.
fn standings(machine: &CompetitionStateMachine, phase_id: PhaseId) -> Vec<EntrantId> {
    machine.current().phases()[&phase_id]
        .standings()
        .iter()
        .map(|standing| standing.ranking().entrants()[0])
        .collect()
}

/// The entrants of the phase `phase_id` in the order its field started it with.
fn seeding(machine: &CompetitionStateMachine, phase_id: PhaseId) -> Vec<EntrantId> {
    machine.current().phases()[&phase_id].divisions()[0]
        .data()
        .rankings()
        .values()
        .map(|ranking| ranking.entrants()[0])
        .collect()
}

#[track_caller]
fn refusal<E: std::error::Error + Clone + 'static>(
    machine: &CompetitionStateMachine,
    transition: impl Into<CompetitionTransition>,
) -> E {
    let error = transition
        .into()
        .check_applicable(machine.current())
        .expect_err("the transition is refused");
    error
        .downcast_ref::<E>()
        .unwrap_or_else(|| panic!("refused for another reason: {error}"))
        .clone()
}

const QUALIFICATION: u128 = 1;
const UPPER: u128 = 2;
const LOWER: u128 = 3;

/// Scenario 1: a qualification of 16 entrants, then knockouts for its places 1–8 and 9–16.
fn qualification_then_knockouts() -> CompetitionStateMachine {
    let mut machine = registered(16);
    add(
        &mut machine,
        AddPhase::new(phase(QUALIFICATION), round_robin()),
    );
    add(
        &mut machine,
        phase_of(
            phase(UPPER),
            vec1![places(phase(QUALIFICATION), 1, Some(8))],
            knockout(),
        ),
    );
    add(
        &mut machine,
        phase_of(
            phase(LOWER),
            vec1![places(phase(QUALIFICATION), 9, None)],
            knockout(),
        ),
    );
    machine.apply_new(StartCompetition.into()).unwrap();
    machine
}

#[test]
fn the_competition_starts_only_the_phases_fed_by_the_registration() {
    let machine = qualification_then_knockouts();
    let phases = machine.current().phases();

    assert!(phases[&phase(QUALIFICATION)].is_started());
    assert!(!phases[&phase(UPPER)].is_started());
    assert!(!phases[&phase(LOWER)].is_started());
    // the registration, by lot, is the field of the first phase
    assert_eq!(
        seeding(&machine, phase(QUALIFICATION)),
        machine.current().lots()
    );
}

#[test]
fn a_phase_starts_once_its_sources_are_finished_with_their_places_in_order() {
    let mut machine = qualification_then_knockouts();
    assert_eq!(
        refusal::<StartPhaseError>(&machine, StartPhase::new(phase(UPPER))),
        StartPhaseError::SourceNotFinished(phase(QUALIFICATION))
    );

    play_out(&mut machine, phase(QUALIFICATION));
    let qualified = standings(&machine, phase(QUALIFICATION));
    start(&mut machine, phase(UPPER));
    start(&mut machine, phase(LOWER));

    assert_eq!(seeding(&machine, phase(UPPER)), qualified[..8]);
    assert_eq!(seeding(&machine, phase(LOWER)), qualified[8..]);
    let field = machine.current().field_of(&phase(LOWER));
    assert_eq!(
        field[0].origin,
        Origin::Phase {
            phase: phase(QUALIFICATION),
            division: 0,
            rank: place(9),
            unique_rank: place(9),
            overall_rank: place(9),
            overall_unique_rank: place(9),
        }
    );
}

#[test]
fn a_phase_is_started_once() {
    let mut machine = qualification_then_knockouts();
    play_out(&mut machine, phase(QUALIFICATION));
    start(&mut machine, phase(UPPER));

    assert_eq!(
        refusal::<StartPhaseError>(&machine, StartPhase::new(phase(UPPER))),
        StartPhaseError::AlreadyStarted
    );
    assert_eq!(
        refusal::<StartPhaseError>(&machine, StartPhase::new(phase(QUALIFICATION))),
        StartPhaseError::AlreadyStarted
    );
}

#[test]
fn a_phase_is_not_started_before_the_competition_nor_one_of_the_registration() {
    let mut machine = registered(4);
    add(
        &mut machine,
        AddPhase::new(phase(QUALIFICATION), round_robin()),
    );
    add(
        &mut machine,
        phase_of(
            phase(UPPER),
            vec1![places(phase(QUALIFICATION), 1, None)],
            knockout(),
        ),
    );
    assert_eq!(
        refusal::<StartPhaseError>(&machine, StartPhase::new(phase(UPPER))),
        StartPhaseError::CompetitionNotStarted
    );
    assert_eq!(
        refusal::<StartPhaseError>(&machine, StartPhase::new(phase(99))),
        StartPhaseError::CompetitionNotStarted
    );

    machine.apply_new(StartCompetition.into()).unwrap();

    assert_eq!(
        refusal::<StartPhaseError>(&machine, StartPhase::new(phase(99))),
        StartPhaseError::NoSuchPhase
    );
}

#[test]
fn a_competition_needs_a_phase_fed_by_the_registration_alone() {
    let mut machine = registered(4);
    // a phase of places of the registration together with another phase cannot be added before the
    // phase, so only an empty competition has no phase fed by the registration alone
    assert!(matches!(
        refusal::<StartCompetitionError>(&machine, StartCompetition),
        StartCompetitionError::NoPhases
    ));
    add(
        &mut machine,
        AddPhase::new(phase(QUALIFICATION), round_robin()),
    );
    assert!(machine.apply_new(StartCompetition.into()).is_ok());
}

#[test]
fn sources_are_refused_unless_they_take_places_of_earlier_phases_once() {
    let mut machine = registered(4);
    add(
        &mut machine,
        AddPhase::new(phase(QUALIFICATION), round_robin()),
    );
    add(
        &mut machine,
        phase_of(
            phase(UPPER),
            vec1![places(phase(QUALIFICATION), 1, Some(2))],
            knockout(),
        ),
    );
    let adding = |sources| phase_of(phase(LOWER), sources, knockout());

    assert_eq!(
        refusal::<AddPhaseError>(&machine, AddPhase::new(phase(UPPER), knockout())),
        AddPhaseError::PhaseExists
    );
    assert_eq!(
        refusal::<AddPhaseError>(&machine, adding(vec1![places(phase(LOWER), 1, None)])),
        AddPhaseError::UnknownPhase(phase(LOWER))
    );
    let empty = places(phase(QUALIFICATION), 4, Some(3));
    assert_eq!(
        refusal::<AddPhaseError>(&machine, adding(vec1![empty])),
        AddPhaseError::EmptyRange(empty)
    );
    // place 2 goes to the upper phase already
    let overlapping = places(phase(QUALIFICATION), 2, None);
    assert_eq!(
        refusal::<AddPhaseError>(&machine, adding(vec1![overlapping])),
        AddPhaseError::Overlap {
            range: overlapping,
            other: places(phase(QUALIFICATION), 1, Some(2)),
            phase: Some(phase(UPPER)),
        }
    );
    // two sources of the phase itself
    let (third, third_on) = (
        places(phase(QUALIFICATION), 3, Some(3)),
        places(phase(QUALIFICATION), 3, None),
    );
    assert_eq!(
        refusal::<AddPhaseError>(&machine, adding(vec1![third, third_on])),
        AddPhaseError::Overlap {
            range: third_on,
            other: third,
            phase: None,
        }
    );
    // the whole registration goes to the qualification already
    assert!(matches!(
        refusal::<AddPhaseError>(&machine, AddPhase::new(phase(LOWER), knockout())),
        AddPhaseError::Overlap { phase: Some(p), .. } if p == phase(QUALIFICATION)
    ));
    assert!(
        machine
            .apply_new(adding(vec1![places(phase(QUALIFICATION), 3, None)]).into())
            .is_ok()
    );
}

#[test]
fn a_phase_is_added_after_the_start_unless_it_takes_the_registration_alone() {
    let mut machine = registered(4);
    add(
        &mut machine,
        AddPhase::new(phase(QUALIFICATION), round_robin()).with_sources(vec1![Source::new(
            SourceOf::Registration,
            place(1),
            Some(place(2)),
        )]),
    );
    machine.apply_new(StartCompetition.into()).unwrap();

    let registration = Source::new(SourceOf::Registration, place(3), None);
    assert_eq!(
        refusal::<AddPhaseError>(
            &machine,
            phase_of(phase(UPPER), vec1![registration], knockout())
        ),
        AddPhaseError::RegistrationClosed
    );
    add(
        &mut machine,
        phase_of(
            phase(UPPER),
            vec1![places(phase(QUALIFICATION), 1, None), registration],
            knockout(),
        ),
    );
    play_out(&mut machine, phase(QUALIFICATION));
    let qualified = standings(&machine, phase(QUALIFICATION));
    start(&mut machine, phase(UPPER));

    // the qualification first, then the registration from place 3 on
    let lots = machine.current().lots();
    assert_eq!(
        seeding(&machine, phase(UPPER)),
        [&qualified[..], &lots[2..]].concat()
    );
}

/// Scenario 2, of three layers: everyone in a qualification, its places 1–6 in an intermediate
/// round, its places 7–10 in a consolation round, and places 1–4 of the intermediate round in a
/// final knockout.
fn three_layers() -> CompetitionStateMachine {
    const INTERMEDIATE: u128 = 4;
    const CONSOLATION: u128 = 5;
    const FINAL: u128 = 6;
    let mut machine = registered(10);
    add(
        &mut machine,
        AddPhase::new(phase(QUALIFICATION), round_robin()),
    );
    add(
        &mut machine,
        phase_of(
            phase(INTERMEDIATE),
            vec1![places(phase(QUALIFICATION), 1, Some(6))],
            round_robin(),
        ),
    );
    add(
        &mut machine,
        phase_of(
            phase(CONSOLATION),
            vec1![places(phase(QUALIFICATION), 7, None)],
            round_robin(),
        ),
    );
    add(
        &mut machine,
        phase_of(
            phase(FINAL),
            vec1![places(phase(INTERMEDIATE), 1, Some(4))],
            knockout(),
        ),
    );
    machine.apply_new(StartCompetition.into()).unwrap();
    play_out(&mut machine, phase(QUALIFICATION));
    start(&mut machine, phase(INTERMEDIATE));
    start(&mut machine, phase(CONSOLATION));
    play_out(&mut machine, phase(INTERMEDIATE));
    play_out(&mut machine, phase(CONSOLATION));
    start(&mut machine, phase(FINAL));
    play_out(&mut machine, phase(FINAL));
    machine
}

#[test]
fn the_competition_places_everyone_by_the_latest_phase_they_reached() {
    let machine = three_layers();
    let competition = machine.current();
    let ranking = competition.ranking();

    assert_eq!(ranking.len(), 10);
    let placed_in = |id: u128| {
        ranking
            .iter()
            .filter(|placed| placed.phase == phase(id))
            .map(|placed| placed.place.get())
            .collect_vec()
    };
    // a knockout of four shares the third place
    assert_eq!(placed_in(6), [1, 2, 3, 3]);
    assert_eq!(placed_in(4), [5, 6]);
    assert_eq!(placed_in(5), [7, 8, 9, 10]);
    let finalists = standings(&machine, phase(6));
    assert_eq!(
        ranking[..4]
            .iter()
            .map(|placed| placed.entrant)
            .collect_vec(),
        finalists
    );
    // the intermediate round ranks its last two where the final does not take them
    assert_eq!(
        ranking[4..6]
            .iter()
            .map(|placed| placed.entrant)
            .collect_vec(),
        standings(&machine, phase(4))[4..]
    );
}

#[test]
fn the_ranking_is_provisional_from_the_phases_started() {
    let mut machine = qualification_then_knockouts();
    report_one(&mut machine, phase(QUALIFICATION));

    let ranking = machine.current().ranking();

    assert_eq!(ranking.len(), 16);
    assert!(
        ranking
            .iter()
            .all(|placed| placed.phase == phase(QUALIFICATION))
    );
    assert_eq!(
        ranking.iter().map(|placed| placed.entrant).collect_vec(),
        standings(&machine, phase(QUALIFICATION))
    );
}

#[test]
fn match_numbers_are_unique_in_the_competition() {
    let mut machine = qualification_then_knockouts();
    play_out(&mut machine, phase(QUALIFICATION));
    start(&mut machine, phase(UPPER));
    start(&mut machine, phase(LOWER));
    play_out(&mut machine, phase(UPPER));
    play_out(&mut machine, phase(LOWER));

    let competition = machine.current();
    let numbers = competition
        .phases()
        .values()
        .flat_map(|phase| phase.divisions())
        .flat_map(|division| {
            division
                .data()
                .matches()
                .values()
                .map(|r#match| r#match.number().get())
        })
        .collect_vec();
    let round_robin = 16 * 15 / 2;
    // a knockout of eight has seven matches, numbered after those before it
    assert_eq!(
        numbers.iter().copied().sorted().collect_vec(),
        (1..=round_robin + 14).collect_vec()
    );
    let lower = &competition.phases()[&phase(LOWER)].divisions()[0];
    assert_eq!(lower.number_base(), round_robin + 7);
}

/// The match of the phase `phase_id` reported last.
fn last_match(machine: &CompetitionStateMachine, phase_id: PhaseId) -> MatchId {
    machine
        .rev_in_effect_applied()
        .find_map(|(_, transition)| match transition {
            CompetitionTransition::ReportMatchResult(report) if report.phase_id() == phase_id => {
                Some(report.match_id())
            }
            _ => None,
        })
        .expect("a match of the phase was reported")
}

/// Undoing a result of a finished phase leaves it unfinished, so the start of a phase taking
/// entrants from it no longer applies, with or without results of its own.
#[test]
fn a_result_of_a_source_is_not_undone_once_the_next_phase_started() {
    for results_in_the_next_phase in [0, 1] {
        let mut machine = qualification_then_knockouts();
        play_out(&mut machine, phase(QUALIFICATION));
        let started = start(&mut machine, phase(UPPER));
        for _ in 0..results_in_the_next_phase {
            report_one(&mut machine, phase(UPPER));
        }
        let last = last_match(&machine, phase(QUALIFICATION));
        let reported = machine.reported_by(phase(QUALIFICATION), last).unwrap();
        let log_len = machine.log().len();

        let error =
            machine.undo_match_result(&mut SessionHistory::new(), phase(QUALIFICATION), last);

        assert!(matches!(error, Err(UndoMatchResultError::Rejected(_))));
        assert_eq!(machine.log().len(), log_len);
        assert!(matches!(
            machine.conflicts_of(Change::Undo(reported)),
            Err(ConflictsError::Other(entry, _)) if entry == started
        ));
    }
}

#[test]
fn editing_the_first_phase_after_the_second_started_starts_it_with_the_new_standings() {
    let mut machine = qualification_then_knockouts();
    play_out(&mut machine, phase(QUALIFICATION));
    start(&mut machine, phase(UPPER));
    let mut history = SessionHistory::new();
    let unranked = PhasePluginsBuilder::new().build().unwrap();

    machine
        .reconfigure_phase(&mut history, AddPhase::new(phase(QUALIFICATION), unranked))
        .unwrap()
        .expect("the ranking changed");

    // ranked by nothing, the qualification ends in the order of the lots
    let qualified = standings(&machine, phase(QUALIFICATION));
    assert_eq!(qualified, machine.current().lots());
    assert_eq!(seeding(&machine, phase(UPPER)), qualified[..8]);
}

#[test]
fn a_phase_is_reconfigured_by_its_name_sources_and_plugins() {
    let mut machine = qualification_then_knockouts();
    let mut history = SessionHistory::new();
    let upper = |to| {
        phase_of(
            phase(UPPER),
            vec1![places(phase(QUALIFICATION), 1, Some(to))],
            knockout(),
        )
    };

    assert!(matches!(
        machine.reconfigure_phase(&mut history, upper(8)),
        Ok(None)
    ));
    machine
        .reconfigure_phase(&mut history, upper(4))
        .unwrap()
        .expect("the sources changed");
    assert_eq!(
        machine.current().phases()[&phase(UPPER)].sources(),
        upper(4).sources()
    );
    machine
        .reconfigure_phase(&mut history, upper(4).with_name(Some("Upper".to_owned())))
        .unwrap()
        .expect("the name changed");
    assert_eq!(
        machine.current().phases()[&phase(UPPER)].name(),
        Some("Upper")
    );
}

#[test]
fn a_phase_is_removed_unless_it_started_or_another_takes_entrants_from_it() {
    let mut machine = qualification_then_knockouts();
    let mut history = SessionHistory::new();

    assert!(matches!(
        machine.remove_phase(&mut history, phase(QUALIFICATION)),
        Err(RemovePhaseError::Started)
    ));
    assert!(matches!(
        machine.remove_phase(&mut history, phase(99)),
        Err(RemovePhaseError::NotAdded)
    ));
    machine.remove_phase(&mut history, phase(LOWER)).unwrap();
    assert!(machine.current().phase(&phase(LOWER)).is_none());

    // undoing the removal brings it back
    history.undo_last(&mut machine).unwrap();
    assert!(machine.current().phase(&phase(LOWER)).is_some());

    let mut machine = registered(4);
    add(
        &mut machine,
        AddPhase::new(phase(QUALIFICATION), round_robin()),
    );
    add(
        &mut machine,
        phase_of(
            phase(UPPER),
            vec1![places(phase(QUALIFICATION), 1, None)],
            knockout(),
        ),
    );
    assert!(matches!(
        machine.remove_phase(&mut history, phase(QUALIFICATION)),
        Err(RemovePhaseError::InUse(users)) if users == [phase(UPPER)]
    ));
}

#[test]
fn a_competition_of_several_phases_round_trips_as_its_log() {
    let machine = three_layers();
    let expected = serde_json::to_value(machine.current()).unwrap();

    let json: CompetitionStateMachine =
        serde_json::from_str(&serde_json::to_string(&machine).unwrap()).unwrap();
    let binary: CompetitionStateMachine =
        postcard::from_bytes(&postcard::to_stdvec(&machine).unwrap()).unwrap();

    assert_eq!(serde_json::to_value(json.current()).unwrap(), expected);
    assert_eq!(serde_json::to_value(binary.current()).unwrap(), expected);
}

#[test]
fn a_competition_of_several_phases_round_trips_as_a_snapshot() {
    let mut machine = qualification_then_knockouts();
    play_out(&mut machine, phase(QUALIFICATION));
    start(&mut machine, phase(UPPER));
    let expected = serde_json::to_value(machine.current()).unwrap();

    let json: Competition = serde_json::from_value(expected.clone()).unwrap();
    let binary: Competition =
        postcard::from_bytes(&postcard::to_stdvec(machine.current()).unwrap()).unwrap();

    assert_eq!(serde_json::to_value(json).unwrap(), expected);
    assert_eq!(serde_json::to_value(binary).unwrap(), expected);
}
