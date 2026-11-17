//! Round trip tests for the transition log: a [`CompetitionStateMachine`] serializes as its log,
//! so every transition this crate ships has to survive being written and read back as a
//! [`CompetitionTransition`], and the restored log has to replay into exactly the
//! competition it was written from.
//!
//! This is the second persistence path, next to the snapshot of a [`Competition`] alone covered by
//! `competition_serialization.rs`. It is the richer of the two: a snapshot holds the current state,
//! while a log also holds how that state came about, which is what keeps undo — and undoing an
//! undo — available after a restore.
//!
//! [`CompetitionTransition`]: tournament_engine::transitions::CompetitionTransition

use std::num::NonZeroU32;

use itertools::Itertools;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tournament_engine::{
    AddEntrant, AddPhase, Annotated, Competition, CompetitionStateMachine,
    CompetitionStateMachineExt, Decision, EnteredValue, Entrant, EntrantId, FieldKey, Match,
    MatchId, MatchResult, Part, Phase, PhaseId, PhasePluginsBuilder, RemoveEntrant,
    ReportMatchResult, ResultReason, Score, Seed, SetCompetitionSeed, Sides, Source, SourceOf,
    StartCompetition, StartPhase, TypeMapValue,
    plugins::{
        draw::{Blocks, Draw, Random, Snake},
        metrics_calculator::{
            BuchholzCalculator, LaterFirstLoss, MetricsCalculator, MorePoints, MorePointsPerMatch,
            Points, PointsCalculator, PointsPerMatchCalculator, RoundOfFirstLossCalculator,
            WinsDrawsLossesCalculator,
        },
        phase_format::SingleElimination,
        ranker::SliceRanker,
        seeding::{GroupWinnersApart, SeededAtRandom, SeededInOrder, Seeding},
        teams::{BestWithWorst, Keep, RandomPairs, Split, Teams},
    },
    undoable_state_machine::{LogEntry, LogEntryKind, SessionHistory, TransitionId},
    uuid::Uuid,
};
use vec1::vec1;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use crate::common::games;

mod common;

/// Everything random here is seeded, so that all runs use the exact same tournament.
const SEED: Seed = Seed::new(*b"log-trip!!!!");

/// The scoring system of most association football leagues.
const THREE_ONE: PointsCalculator = PointsCalculator::new(Score::new(3.0), Score::ONE);

fn phase_id() -> PhaseId {
    PhaseId::new(Uuid::from_u128(0xfa5e))
}

fn entrant_id(entrant: u128) -> EntrantId {
    EntrantId::new(Uuid::from_u128(entrant))
}

/// A consumer annotation on an [`Entrant`], standing in for whatever a real consumer attaches. It
/// reaches the competition only through the [`AddEntrant`] that carries it, so a log that loses it
/// cannot replay into the same competition — which is what makes it worth having here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Nickname(String);

#[typetag::serde(name = "test::nickname")]
impl TypeMapValue for Nickname {}

#[track_caller]
fn undo(machine: &mut CompetitionStateMachine, target: TransitionId) -> TransitionId {
    let id = TransitionId::new();
    machine
        .apply(LogEntry::new(id, LogEntryKind::Undo(target)))
        .expect("the undo target must exist");
    id
}

/// A competition that was seeded, given five entrants of which one was removed again, given a
/// single elimination phase, started in an order given, and played up to its final — so its log
/// holds at least one of every transition this crate ships.
fn played_competition() -> CompetitionStateMachine {
    let mut machine = CompetitionStateMachine::default();

    machine
        .apply_new(SetCompetitionSeed::new(SEED).into())
        .unwrap();
    for entrant in 0..5 {
        let entrant = Entrant::new(entrant_id(entrant))
            .with_annotation(Nickname(format!("entrant {entrant}")));
        machine.apply_new(AddEntrant::new(entrant).into()).unwrap();
    }
    machine
        .apply_new(RemoveEntrant::new(entrant_id(4)).into())
        .unwrap();
    let plugins = PhasePluginsBuilder::new()
        .format(SingleElimination::new())
        .add_ranking(LaterFirstLoss)
        .add_ranking(MorePoints)
        .add_metric(RoundOfFirstLossCalculator)
        .add_metric(WinsDrawsLossesCalculator)
        .add_metric(THREE_ONE)
        .add_metric(BuchholzCalculator)
        .build()
        .unwrap();
    machine
        .apply_new(AddPhase::new(phase_id(), plugins).into())
        .unwrap();
    machine
        .apply_new(
            StartCompetition::new(vec![
                vec1![entrant_id(2)],
                vec1![entrant_id(0), entrant_id(3)],
            ])
            .into(),
        )
        .unwrap();
    for match_id in matches_of_round(&machine.current().phases()[&phase_id()], 1) {
        machine
            .apply_new(ReportMatchResult::new(phase_id(), match_id, MatchResult::AWins).into())
            .unwrap();
    }

    machine
}

/// The matches of `round`, ordered by the initial unique rank of the first ranking of their team A,
/// which is unique within a round and unaffected by any reported result, so that the order does not
/// depend on the iteration order of the match map.
fn matches_of_round(phase: &Phase, round: u32) -> Vec<MatchId> {
    let phase = phase.divisions()[0].data();
    phase
        .matches()
        .values()
        .filter(|r#match| r#match.round().get() == round)
        .sorted_by_key(|r#match| phase.rankings()[&r#match.team_a()[0]].initial_unique_rank())
        .map(Match::id)
        .collect()
}

fn round_trip(machine: &CompetitionStateMachine) -> CompetitionStateMachine {
    let json = serde_json::to_string(machine).expect("serializing a log must not fail");
    serde_json::from_str(&json).expect("a serialized log must deserialize again")
}

/// A round trip through [`postcard`], a compact binary format that is not self-describing and, more
/// importantly here, not human readable.
fn binary_round_trip(machine: &CompetitionStateMachine) -> CompetitionStateMachine {
    let bytes = postcard::to_stdvec(machine).expect("serializing a log must not fail");
    postcard::from_bytes(&bytes).expect("a serialized log must deserialize again")
}

fn serialized_log(machine: &CompetitionStateMachine) -> Value {
    serde_json::to_value(machine).expect("serializing a log must not fail")
}

/// The tag of the transition `entry`, an entry of a serialized log, applies, if it applies one.
fn tag_of(entry: &Value) -> Option<&str> {
    let transition = entry["kind"]["Apply"].as_object()?;
    assert_eq!(transition.len(), 1, "a transition is its tag and value");
    transition.keys().next().map(String::as_str)
}

/// The serialized form of a competition, compared as is — see `competition_serialization.rs` for
/// why two competitions holding the same state serialize identically however each was built.
fn serialized(competition: &Competition) -> Value {
    serde_json::to_value(competition).expect("serializing a competition must not fail")
}

/// Asserts that `machine` holds `expected` as its current state, first through a few public getters
/// so that a failure points at the part that differs, and then through the whole serialized form,
/// which also covers the private fields, the plugins and the metrics.
#[track_caller]
fn assert_current_is(machine: &CompetitionStateMachine, expected: &Competition) {
    let current = machine.current();

    assert_eq!(current.is_started(), expected.is_started());
    assert_eq!(current.seed(), expected.seed());
    assert_eq!(current.entrants().len(), expected.entrants().len());
    assert_eq!(current.phases().len(), expected.phases().len());
    assert_eq!(serialized(current), serialized(expected));
}

/// The name of a transition's variant is its tag in the serialized form of every log written, so a
/// change to it needs a converter of the files saved before and has to be deliberate. Pinning
/// every tag here makes renaming a variant fail this test instead of silently invalidating old
/// logs.
///
/// Asking for the tags in order of first appearance also asserts that the log below really does
/// exercise all six transitions.
#[test]
fn every_transition_is_serialized_under_its_pinned_tag() {
    let machine = played_competition();

    let Value::Array(entries) = serialized_log(&machine) else {
        panic!("a log must serialize to a sequence");
    };

    let tags = entries
        .iter()
        .map(|entry| tag_of(entry).expect("every applied transition must carry a tag"))
        .unique()
        .collect_vec();
    assert_eq!(
        tags,
        [
            "SetCompetitionSeed",
            "AddEntrant",
            "RemoveEntrant",
            "AddPhase",
            "StartCompetition",
            "ReportMatchResult",
        ]
    );
}

/// A phase taking entrants from another is added with its name and sources, and started by a
/// transition of its own, each under its pinned tag.
#[test]
fn a_phase_of_another_phase_is_added_and_started_under_pinned_tags() {
    let mut machine = played_competition();
    let next = PhaseId::new(Uuid::from_u128(0x2e));
    let sources = vec1![Source::new(
        SourceOf::Phase(phase_id()),
        NonZeroU32::MIN,
        NonZeroU32::new(2),
    )];
    let plugins = PhasePluginsBuilder::new().build().unwrap();
    let added = machine
        .apply_new(
            AddPhase::new(next, plugins)
                .with_name(Some("Final".to_owned()))
                .with_sources(sources)
                .into(),
        )
        .unwrap();
    while let Some(open) = machine.current().phases()[&phase_id()].divisions()[0]
        .data()
        .matches()
        .values()
        .find(|r#match| !r#match.is_finished())
    {
        let report = ReportMatchResult::new(phase_id(), open.id(), MatchResult::AWins);
        machine.apply_new(report.into()).unwrap();
    }
    let started = machine.apply_new(StartPhase::new(next).into()).unwrap();

    let log = serialized_log(&machine);
    let entry = |id| {
        log.as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["id"] == json!(id))
            .unwrap()["kind"]["Apply"]
            .clone()
    };

    let added = &entry(added)["AddPhase"];
    assert_eq!(added["name"], json!("Final"));
    assert_eq!(
        added["sources"],
        json!([{ "of": { "Phase": phase_id() }, "from": 1, "to": 2 }])
    );
    assert_eq!(
        entry(started),
        json!({ "StartPhase": { "phase_id": next } })
    );
    assert_current_is(&round_trip(&machine), machine.current());
    assert_current_is(&binary_round_trip(&machine), machine.current());
}

#[test]
fn a_transition_is_serialized_as_its_payload_under_its_tag() {
    let mut machine = CompetitionStateMachine::default();
    let seed_id = machine
        .apply_new(SetCompetitionSeed::new(SEED).into())
        .unwrap();

    let log = serialized_log(&machine);

    assert_eq!(
        log,
        json!([{
            "id": seed_id,
            "kind": {
                "Apply": { "SetCompetitionSeed": { "seed": SEED } },
            },
        }])
    );
}

/// What a report says of how a match was played reaches the match, through any log format.
#[test]
fn a_reported_contest_survives_a_round_trip() {
    let (mut machine, open) = games::competition_entering_games();
    let points = FieldKey::new("points");
    machine
        .apply_new(
            ReportMatchResult::new(
                games::phase_id(),
                open,
                Decision::new(MatchResult::AWins).with_reason(ResultReason::new("retired")),
            )
            .with_value(
                FieldKey::new("games"),
                Sides::new(Some(EnteredValue::Count(1)), None),
            )
            .with_part(
                Part::new()
                    .with_decision(MatchResult::AWins)
                    .with_value(points.clone(), games::count(21, 17)),
            )
            .with_part(Part::new().with_value(points, games::count(3, 1)))
            .into(),
        )
        .unwrap();
    let contest = machine.current().phases()[&games::phase_id()].divisions()[0]
        .data()
        .matches()[&open]
        .contest();
    assert_eq!(
        contest.decision().unwrap().reason(),
        Some(&ResultReason::new("retired"))
    );
    assert_eq!(
        contest.value(&FieldKey::new("games")),
        Some(&Sides::new(Some(EnteredValue::Count(1)), None))
    );
    assert_eq!(contest.parts().len(), 2);
    assert_eq!(
        contest.parts()[0].decision().unwrap().result(),
        MatchResult::AWins
    );
    assert!(contest.parts()[1].decision().is_none());

    assert_current_is(&round_trip(&machine), machine.current());
    assert_current_is(&binary_round_trip(&machine), machine.current());
}

/// The start is saved with the order it gives the registration, as tiers of entrant ids.
#[test]
fn the_start_is_serialized_with_its_order() {
    let machine = played_competition();

    let Value::Array(entries) = serialized_log(&machine) else {
        panic!("a log must serialize to a sequence");
    };

    let starts = entries
        .iter()
        .filter(|entry| tag_of(entry) == Some("StartCompetition"))
        .collect_vec();
    assert_eq!(starts.len(), 1, "the log must hold exactly one start");
    assert_eq!(
        starts[0]["kind"]["Apply"],
        json!({ "StartCompetition": { "order": [
            ["00000000-0000-0000-0000-000000000002"],
            ["00000000-0000-0000-0000-000000000000", "00000000-0000-0000-0000-000000000003"],
        ] } })
    );
}

#[test]
fn an_empty_machine_round_trips() {
    let machine = round_trip(&CompetitionStateMachine::default());

    assert!(machine.log().is_empty());
    assert_current_is(&machine, &Competition::default());
}

/// The whole point of the log path: a competition can be rebuilt from nothing but its transitions.
/// Deserializing replays them, which also needs every plugin inside the round-tripped [`AddPhase`]
/// back as its concrete type, since starting the phase and reporting a result is what runs them.
#[test]
fn a_log_round_trips_and_replays_into_the_same_competition() {
    let machine = played_competition();

    let restored = round_trip(&machine);

    assert_eq!(restored.log().len(), machine.log().len());
    assert_current_is(&restored, machine.current());
}

/// A log must survive a format that is not self-describing, whose values look nothing like those
/// of a human readable one, which is easy to get wrong for the plugins inside an [`AddPhase`]:
/// `typetag` serializes the content of an *internally* tagged trait object through an adapter that
/// reports itself as human readable no matter what the real format says, so a [`Uuid`] below that
/// content would be written as a string and read back as raw bytes. Every plugin trait is therefore
/// adjacently tagged, which keeps the content out of that adapter.
#[test]
fn a_log_round_trips_through_a_binary_format() {
    let machine = played_competition();

    let restored = binary_round_trip(&machine);

    assert_current_is(&restored, machine.current());
}

/// A reconfiguration is a replacement of the transition that added the phase, which has to round
/// trip like an undo, in both kinds of format.
#[test]
fn a_reconfigured_phase_round_trips() {
    let mut machine = played_competition();
    let ranked_by_points = PhasePluginsBuilder::new()
        .format(SingleElimination::new())
        .add_ranking(MorePoints)
        .add_metric(WinsDrawsLossesCalculator)
        .add_metric(THREE_ONE)
        .build()
        .unwrap();
    machine
        .reconfigure_phase(
            &mut SessionHistory::new(),
            AddPhase::new(phase_id(), ranked_by_points),
        )
        .unwrap()
        .expect("the plugins changed");

    for restored in [round_trip(&machine), binary_round_trip(&machine)] {
        assert_eq!(restored.log().len(), machine.log().len());
        assert_current_is(&restored, machine.current());
    }
}

#[test]
fn a_round_trip_does_not_change_the_serialized_form_of_a_log() {
    let machine = played_competition();
    let expected = serialized_log(&machine);

    let restored = round_trip(&round_trip(&machine));

    assert_eq!(serialized_log(&restored), expected);
}

/// Consumer annotations are only ever in the log, never derivable from state, so this is the one
/// thing a round trip of an [`AddEntrant`] cannot afford to drop.
#[test]
fn a_round_tripped_log_still_carries_its_consumer_annotations() {
    let machine = round_trip(&played_competition());

    let competition = machine.current();

    assert_eq!(competition.entrants().len(), 4);
    // entrant 4 was removed again, so only the first four are left
    for entrant in 0..4 {
        let found = competition
            .entrants()
            .get(&entrant_id(entrant))
            .expect("the entrant must exist");
        assert_eq!(
            found.annotation::<Nickname>(),
            Some(&Nickname(format!("entrant {entrant}")))
        );
    }
}

/// What a snapshot of the current state cannot do: the restored machine still knows the history, so
/// a result reported before the restore can be taken back after it.
#[test]
fn a_round_tripped_log_can_still_be_undone() {
    let machine = played_competition();
    let last_result = machine
        .log()
        .last()
        .expect("the log must hold a result")
        .id();
    let mut restored = round_trip(&machine);

    undo(&mut restored, last_result);

    // one of the two first round results is gone, so the final it scheduled is gone with it and
    // only one ranking holds the points of a win
    let phase = restored.current().phases()[&phase_id()].divisions()[0].data();
    assert_eq!(phase.matches().len(), 2);
    assert_eq!(
        phase
            .rankings()
            .values()
            .filter(|ranking| ranking.metric::<Points>() == Some(&Points(Score::new(3.0))))
            .count(),
        1
    );
}

/// An undo is itself a log entry, so it has to round trip like any other — and stay undoable, which
/// is what a redo is.
#[test]
fn an_undo_survives_a_round_trip_and_can_itself_be_undone() {
    let mut machine = played_competition();
    let played = serialized(machine.current());
    let last_result = machine
        .log()
        .last()
        .expect("the log must hold a result")
        .id();
    let undo_id = undo(&mut machine, last_result);
    let undone = serialized(machine.current());
    assert_ne!(undone, played, "the undo must have changed the state");

    let mut restored = round_trip(&machine);

    // the undo came back as an undo, not as a shorter log
    assert_eq!(restored.log().len(), machine.log().len());
    assert_eq!(serialized(restored.current()), undone);
    undo(&mut restored, undo_id);
    assert_eq!(serialized(restored.current()), played);
}

#[test]
fn a_round_tripped_log_can_continue_the_tournament() {
    let mut machine = round_trip(&played_competition());
    let final_match = matches_of_round(&machine.current().phases()[&phase_id()], 2)[0];

    machine
        .apply_new(ReportMatchResult::new(phase_id(), final_match, MatchResult::BWins).into())
        .unwrap();

    let phase = machine.current().phases()[&phase_id()].divisions()[0].data();
    assert!(phase.matches().values().all(Match::is_finished));
}

/// A plugin is serialized under its id, which is as much a persistence contract as the tag of a
/// transition, so every plugin in the log is pinned here as well.
#[test]
fn every_plugin_is_serialized_under_its_id() {
    let log = serialized_log(&played_competition());
    let plugins = log
        .as_array()
        .expect("a log must serialize to a sequence")
        .iter()
        .find(|entry| tag_of(entry) == Some("AddPhase"))
        .expect("the log must hold the phase")["kind"]["Apply"]["AddPhase"]["plugins"]
        .clone();
    let ids = |plugins: &Value| -> Vec<String> {
        plugins
            .as_array()
            .expect("a list of plugins must serialize to a sequence")
            .iter()
            .map(|plugin| plugin["type"].as_str().unwrap().to_owned())
            .collect()
    };

    assert_eq!(
        plugins["format"]["type"],
        "tournament_engine::single_elimination"
    );
    assert_eq!(
        plugins["match_format"]["type"],
        "tournament_engine::winner_only"
    );
    assert_eq!(
        ids(&plugins["ranking"]),
        [
            "tournament_engine::later_first_loss",
            "tournament_engine::more_points"
        ]
    );
    assert_eq!(
        ids(&plugins["metrics"]),
        [
            "tournament_engine::round_of_first_loss",
            "tournament_engine::wins_draws_losses",
            "tournament_engine::points",
            "tournament_engine::buchholz",
        ]
    );
    assert_eq!(plugins["teams"]["type"], "tournament_engine::keep");
    assert_eq!(plugins["divisions"], 1);
    assert_eq!(plugins["draw"]["type"], "tournament_engine::snake");
    assert_eq!(
        plugins["seeding"]["type"],
        "tournament_engine::seeded_in_order"
    );
    assert_eq!(plugins["across"], json!([]));
}

/// The plugins no phase of [`played_competition`] has, pinned like those it has.
#[test]
fn the_plugins_of_divisions_and_teams_are_serialized_under_their_ids() {
    let id = |plugin: Value| plugin["type"].as_str().unwrap().to_owned();
    let draws: [Box<dyn Draw>; 3] = [Box::new(Snake), Box::new(Random), Box::new(Blocks)];

    assert_eq!(
        draws
            .map(|draw| id(serde_json::to_value(draw).unwrap()))
            .as_slice(),
        [
            "tournament_engine::snake",
            "tournament_engine::random",
            "tournament_engine::blocks",
        ]
    );
    let seedings: [Box<dyn Seeding>; 3] = [
        Box::new(SeededInOrder),
        Box::new(SeededAtRandom),
        Box::new(GroupWinnersApart),
    ];
    assert_eq!(
        seedings
            .map(|seeding| id(serde_json::to_value(seeding).unwrap()))
            .as_slice(),
        [
            "tournament_engine::seeded_in_order",
            "tournament_engine::seeded_at_random",
            "tournament_engine::group_winners_apart",
        ]
    );
    let teams: [Box<dyn Teams>; 4] = [
        Box::new(Keep),
        Box::new(RandomPairs),
        Box::new(BestWithWorst),
        Box::new(Split),
    ];
    assert_eq!(
        teams
            .map(|teams| id(serde_json::to_value(teams).unwrap()))
            .as_slice(),
        [
            "tournament_engine::keep",
            "tournament_engine::random_pairs",
            "tournament_engine::best_with_worst",
            "tournament_engine::split",
        ]
    );
    let calculator: Box<dyn MetricsCalculator> = Box::new(PointsPerMatchCalculator);
    assert_eq!(
        id(serde_json::to_value(calculator).unwrap()),
        "tournament_engine::points_per_match"
    );
    let criterion: Box<dyn SliceRanker> = Box::new(MorePointsPerMatch);
    assert_eq!(
        id(serde_json::to_value(criterion).unwrap()),
        "tournament_engine::more_points_per_match"
    );
}

#[test]
fn deserializing_a_log_with_an_unknown_transition_fails() {
    let machine = played_competition();
    let mut log = serialized_log(&machine);
    log[0]["kind"]["Apply"] = json!({ "NotATransition": null });

    let Err(error) = serde_json::from_value::<CompetitionStateMachine>(log) else {
        panic!("an unknown transition must not deserialize");
    };

    assert!(
        error.to_string().contains("NotATransition"),
        "the error must name the unknown transition, but was: {error}"
    );
}

/// A log is replayed against a fresh competition, so a transition that was applicable when it was
/// written is not trusted to be applicable again: `check_applicable` is re-run on every entry while
/// deserializing.
#[test]
fn deserializing_a_log_whose_transitions_do_not_apply_fails() {
    let machine = played_competition();
    let mut log = serialized_log(&machine);
    let Value::Array(entries) = &mut log else {
        panic!("a log must serialize to a sequence");
    };
    // drop the phase, which the start below it and every reported result after that one need
    entries.retain(|entry| tag_of(entry) != Some("AddPhase"));

    let Err(error) = serde_json::from_value::<CompetitionStateMachine>(log) else {
        panic!("a log that does not replay must not deserialize");
    };

    // the start is the first entry the missing phase makes inapplicable, so replaying stops there
    // rather than carrying on to the results that would also have failed
    assert!(
        error.to_string().contains("no phases"),
        "the error must carry the reason the log stopped replaying, but was: {error}"
    );
}
