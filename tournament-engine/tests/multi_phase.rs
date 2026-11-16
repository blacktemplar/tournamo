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
        draw::Blocks,
        metrics_calculator::{
            LaterFirstLoss, MorePoints, MorePointsPerMatch, PointsCalculator,
            PointsPerMatchCalculator, RoundOfFirstLossCalculator, WinsDrawsLossesCalculator,
        },
        phase_format::SingleElimination,
        seeding::GroupWinnersApart,
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

/// Ranked by nothing, everyone of the qualification shares the first place; the lot takes two of
/// them on, and the two it leaves behind share the place after the final's.
#[test]
fn the_rows_left_behind_by_a_cut_through_a_tie_are_placed_after_those_taken_on() {
    let mut machine = registered(4);
    let unranked = PhasePluginsBuilder::new().build().unwrap();
    add(&mut machine, AddPhase::new(phase(QUALIFICATION), unranked));
    add(
        &mut machine,
        phase_of(
            phase(UPPER),
            vec1![places(phase(QUALIFICATION), 1, Some(2))],
            knockout(),
        ),
    );
    machine.apply_new(StartCompetition.into()).unwrap();
    play_out(&mut machine, phase(QUALIFICATION));
    start(&mut machine, phase(UPPER));
    play_out(&mut machine, phase(UPPER));

    let places = machine
        .current()
        .ranking()
        .iter()
        .map(|placed| (placed.place.get(), placed.phase))
        .collect_vec();

    assert_eq!(
        places,
        [
            (1, phase(UPPER)),
            (2, phase(UPPER)),
            (3, phase(QUALIFICATION)),
            (3, phase(QUALIFICATION)),
        ]
    );
}

const GROUPS: u128 = 7;
const KNOCKOUT: u128 = 8;

/// Round robins in `groups` groups, dealt in order, ranked by points, compared across the groups
/// by points per match, then points.
fn groups(groups: u32) -> PhasePlugins {
    PhasePluginsBuilder::new()
        .divisions(groups)
        .add_ranking(MorePoints)
        .add_metric(WinsDrawsLossesCalculator)
        .add_metric(PointsCalculator::new(Score::ONE, Score::new(0.5)))
        .add_metric(PointsPerMatchCalculator)
        .add_across(MorePointsPerMatch)
        .add_across(MorePoints)
        .build()
        .unwrap()
}

/// Scenario 2: 11 entrants in 3 groups, then a knockout of the best 8 by the overall standings,
/// seeded so that entrants of one group meet late.
fn groups_then_a_knockout() -> CompetitionStateMachine {
    let mut machine = registered(11);
    add(&mut machine, AddPhase::new(phase(GROUPS), groups(3)));
    let knockout = PhasePluginsBuilder::new()
        .format(SingleElimination::new())
        .seeding(GroupWinnersApart)
        .add_ranking(LaterFirstLoss)
        .add_metric(RoundOfFirstLossCalculator)
        .build()
        .unwrap();
    add(
        &mut machine,
        phase_of(
            phase(KNOCKOUT),
            vec1![places(phase(GROUPS), 1, Some(8))],
            knockout,
        ),
    );
    machine.apply_new(StartCompetition.into()).unwrap();
    machine
}

/// The entrants of each division of the phase `phase_id`, in seeding order.
fn divisions(machine: &CompetitionStateMachine, phase_id: PhaseId) -> Vec<Vec<EntrantId>> {
    machine.current().phases()[&phase_id]
        .divisions()
        .iter()
        .map(|division| {
            division
                .data()
                .rankings()
                .values()
                .map(|ranking| ranking.entrants()[0])
                .collect()
        })
        .collect()
}

/// Checks that in the overall standings of the phase `phase_id` no ranking stands before another
/// of its division that stands before it there.
#[track_caller]
fn assert_never_against_a_division(machine: &CompetitionStateMachine, phase_id: PhaseId) {
    let standings = machine.current().phases()[&phase_id].standings();
    for [first, second] in standings.iter().array_combinations() {
        if first.division() != second.division() {
            continue;
        }
        let (in_first, in_second) = (first.in_division(), second.in_division());
        if in_first.rank() < in_second.rank() {
            assert!(first.rank() < second.rank() && first.unique_rank() < second.unique_rank());
        }
        if in_second.rank() < in_first.rank() {
            assert!(second.rank() < first.rank() && second.unique_rank() < first.unique_rank());
        }
    }
}

#[test]
fn the_groups_are_dealt_the_registration_back_and_forth() {
    let machine = groups_then_a_knockout();
    let lots = machine.current().lots();

    assert_eq!(
        divisions(&machine, phase(GROUPS)),
        [
            vec![lots[0], lots[5], lots[6]],
            vec![lots[1], lots[4], lots[7], lots[10]],
            vec![lots[2], lots[3], lots[8], lots[9]],
        ]
    );
    let phase = &machine.current().phases()[&phase(GROUPS)];
    let seeds = phase
        .divisions()
        .iter()
        .map(|division| division.data().seed())
        .collect_vec();
    assert!(seeds.iter().all_unique());
}

#[test]
fn the_overall_standings_of_groups_never_go_against_a_group() {
    let mut machine = groups_then_a_knockout();
    assert_never_against_a_division(&machine, phase(GROUPS));

    while !open_matches(&machine, phase(GROUPS)).is_empty() {
        report_one(&mut machine, phase(GROUPS));
        assert_never_against_a_division(&machine, phase(GROUPS));
    }
}

/// After one result one group has a leader, while everyone of the others is tied; yet each place
/// of a group is a place of the overall standings, the lot of a group deciding a tie in it, so
/// that a next phase taking `k` places of each group takes as many of each.
#[test]
fn each_place_of_a_group_is_a_place_of_the_overall_standings() {
    let mut machine = groups_then_a_knockout();
    report_one(&mut machine, phase(GROUPS));

    let phase = &machine.current().phases()[&phase(GROUPS)];
    let rows = phase
        .standings()
        .iter()
        .map(|row| (row.division(), row.in_division().unique_rank().get()))
        .collect_vec();

    // the groups of four have no fourth to compare with group A's
    let places = rows.iter().map(|row| row.1).collect_vec();
    assert_eq!(places, [1, 1, 1, 2, 2, 2, 3, 3, 3, 4, 4]);
    for place in rows.chunk_by(|a, b| a.1 == b.1) {
        assert!(place.iter().map(|row| row.0).all_unique());
    }
}

/// Group A has three entrants, so everyone of it had a bye, worth no points, and is compared with
/// the same places of the groups of four by points per match.
#[test]
fn the_places_of_groups_of_different_sizes_are_compared_per_match() {
    let mut machine = groups_then_a_knockout();
    play_out(&mut machine, phase(GROUPS));
    let groups = divisions(&machine, phase(GROUPS));

    let phase = &machine.current().phases()[&phase(GROUPS)];
    let rows = phase
        .standings()
        .iter()
        .map(|row| {
            (
                row.ranking().entrants()[0],
                row.division(),
                row.rank().get(),
                row.unique_rank().get(),
            )
        })
        .collect_vec();

    // every group ends in the order it was dealt, the lots deciding every match; the bye costs
    // everyone of group A a third of a point per match, and the lot breaks the ties
    let [a, b, c] = &groups[..] else {
        unreachable!("three groups")
    };
    let expected = [
        (b[0], 1),
        (c[0], 1),
        (a[0], 3),
        (c[1], 4),
        (b[1], 4),
        (a[1], 6),
        (b[2], 7),
        (c[2], 7),
        (a[2], 9),
        (c[3], 10),
        (b[3], 10),
    ];
    assert_eq!(
        rows.iter().map(|row| (row.0, row.2)).collect_vec(),
        expected
    );
    assert_eq!(
        rows.iter().map(|row| row.3).collect_vec(),
        (1..=11).collect_vec()
    );
}

#[test]
fn a_knockout_of_the_best_of_groups_keeps_groups_apart_and_places_everyone() {
    let mut machine = groups_then_a_knockout();
    play_out(&mut machine, phase(GROUPS));
    let groups = divisions(&machine, phase(GROUPS));
    let group_of = |entrant: EntrantId| {
        groups
            .iter()
            .position(|group| group.contains(&entrant))
            .unwrap()
    };
    start(&mut machine, phase(KNOCKOUT));

    let knockout = &machine.current().phases()[&phase(KNOCKOUT)];
    let division = knockout.divisions()[0].data();
    let first_round = division
        .matches()
        .values()
        .filter(|r#match| r#match.round().get() == 1)
        .collect_vec();
    assert_eq!(first_round.len(), 4);
    for r#match in first_round {
        let entrant = |team: &Vec1<RankingId>| division.rankings()[&team[0]].entrants()[0];
        assert_ne!(
            group_of(entrant(r#match.team_a())),
            group_of(entrant(r#match.team_b()))
        );
    }

    play_out(&mut machine, phase(KNOCKOUT));
    let places = machine
        .current()
        .ranking()
        .iter()
        .map(|placed| (placed.place.get(), placed.phase))
        .collect_vec();
    let knockout = |place| (place, phase(KNOCKOUT));
    let groups = |place| (place, phase(GROUPS));
    assert_eq!(
        places,
        [
            knockout(1),
            knockout(2),
            knockout(3),
            knockout(3),
            knockout(5),
            knockout(5),
            knockout(5),
            knockout(5),
            groups(9),
            groups(10),
            groups(10),
        ]
    );
}

/// Scenario 3: a qualification, then knockouts A, B and C for its places in blocks of four.
#[test]
fn knockouts_in_blocks_play_for_places_of_their_own() {
    let mut machine = registered(12);
    add(
        &mut machine,
        AddPhase::new(phase(QUALIFICATION), round_robin()),
    );
    let knockouts = PhasePluginsBuilder::new()
        .format(SingleElimination::new())
        .divisions(3)
        .draw(Blocks)
        .add_ranking(LaterFirstLoss)
        .add_metric(RoundOfFirstLossCalculator)
        .build()
        .unwrap();
    add(
        &mut machine,
        phase_of(
            phase(KNOCKOUT),
            vec1![places(phase(QUALIFICATION), 1, None)],
            knockouts,
        ),
    );
    machine.apply_new(StartCompetition.into()).unwrap();
    play_out(&mut machine, phase(QUALIFICATION));
    let qualified = standings(&machine, phase(QUALIFICATION));
    start(&mut machine, phase(KNOCKOUT));

    assert_eq!(
        divisions(&machine, phase(KNOCKOUT)),
        [&qualified[..4], &qualified[4..8], &qualified[8..]]
    );
    let numbers = machine.current().phases()[&phase(KNOCKOUT)]
        .divisions()
        .iter()
        .flat_map(|division| division.data().matches().values())
        .map(|r#match| r#match.number())
        .collect_vec();
    assert!(numbers.iter().all_unique());

    play_out(&mut machine, phase(KNOCKOUT));
    let rows = machine.current().phases()[&phase(KNOCKOUT)]
        .standings()
        .iter()
        .map(|row| (row.division(), row.rank().get()))
        .collect_vec();
    assert_eq!(
        rows,
        [
            (0, 1),
            (0, 2),
            (0, 3),
            (0, 3),
            (1, 5),
            (1, 6),
            (1, 7),
            (1, 7),
            (2, 9),
            (2, 10),
            (2, 11),
            (2, 11),
        ]
    );
    let places = machine
        .current()
        .ranking()
        .iter()
        .map(|placed| placed.place.get())
        .collect_vec();
    assert_eq!(places, [1, 2, 3, 3, 5, 6, 7, 7, 9, 10, 11, 11]);
}

/// Two knockouts of the registration: the better half of the lots plays for places 1 to 4.
#[test]
fn two_knockouts_in_blocks_of_the_registration() {
    let mut machine = registered(8);
    let knockouts = PhasePluginsBuilder::new()
        .format(SingleElimination::new())
        .divisions(2)
        .draw(Blocks)
        .add_ranking(LaterFirstLoss)
        .add_metric(RoundOfFirstLossCalculator)
        .build()
        .unwrap();
    add(&mut machine, AddPhase::new(phase(KNOCKOUT), knockouts));
    machine.apply_new(StartCompetition.into()).unwrap();
    let lots = machine.current().lots().to_vec();

    assert_eq!(
        divisions(&machine, phase(KNOCKOUT)),
        [&lots[..4], &lots[4..]]
    );
    play_out(&mut machine, phase(KNOCKOUT));
    let ranking = machine.current().ranking();
    assert_eq!(
        ranking
            .iter()
            .map(|placed| placed.place.get())
            .collect_vec(),
        [1, 2, 3, 3, 5, 6, 7, 7]
    );
    assert!(
        ranking[..4]
            .iter()
            .all(|placed| lots[..4].contains(&placed.entrant))
    );
}

#[test]
fn a_phase_has_no_more_divisions_than_leave_each_the_entrants_its_format_needs() {
    let mut machine = registered(5);
    add(&mut machine, AddPhase::new(phase(GROUPS), groups(3)));
    machine.apply_new(StartCompetition.into()).unwrap();

    assert_eq!(
        divisions(&machine, phase(GROUPS))
            .iter()
            .map(Vec::len)
            .collect_vec(),
        [3, 2]
    );
}

#[test]
fn a_competition_of_divisions_round_trips_as_its_log_and_as_a_snapshot() {
    let mut machine = groups_then_a_knockout();
    play_out(&mut machine, phase(GROUPS));
    start(&mut machine, phase(KNOCKOUT));
    report_one(&mut machine, phase(KNOCKOUT));
    let expected = serde_json::to_value(machine.current()).unwrap();

    let json: CompetitionStateMachine =
        serde_json::from_str(&serde_json::to_string(&machine).unwrap()).unwrap();
    let binary: CompetitionStateMachine =
        postcard::from_bytes(&postcard::to_stdvec(&machine).unwrap()).unwrap();
    let snapshot: Competition =
        postcard::from_bytes(&postcard::to_stdvec(machine.current()).unwrap()).unwrap();

    assert_eq!(serde_json::to_value(json.current()).unwrap(), expected);
    assert_eq!(serde_json::to_value(binary.current()).unwrap(), expected);
    assert_eq!(serde_json::to_value(snapshot).unwrap(), expected);
}
