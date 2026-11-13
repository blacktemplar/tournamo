#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::{
    entities::{Phase, PhaseBuilder},
    plugins::metrics_calculator::test_utils::{self, AnnotatedMatchSpec, MatchSpec, match_id},
    type_map::TypeMap,
};

/// A ranking that is still in the phase, which is the best this metric gets.
const STILL_IN: RoundOfElimination = RoundOfElimination(None);

/// Builds a phase with `num_players` rankings and all matches of `matches` already scheduled,
/// each carrying the annotations given next to its schedule.
///
/// [`RoundOfEliminationCalculator`] reads nothing but the [`Eliminates`] annotation and the
/// result of the match it is called for, so it is the only calculator of the phase.
fn phase_with(num_players: u32, matches: Vec<AnnotatedMatchSpec>) -> Phase {
    test_utils::phase_with_annotated_matches(
        PhaseBuilder::new()
            .add_players(num_players)
            .add_metric(RoundOfEliminationCalculator),
        matches,
        vec![],
    )
}

/// `r#match` as a match that knocks out whoever `eliminates` names.
fn eliminating(r#match: MatchSpec, eliminates: Eliminates) -> AnnotatedMatchSpec {
    let mut annotations = TypeMap::new();
    annotations.insert(eliminates);
    (r#match, annotations)
}

/// `r#match` as a match whose loser is out, whichever side it is.
fn eliminating_the_loser(r#match: MatchSpec) -> AnnotatedMatchSpec {
    eliminating(r#match, Eliminates::Loser)
}

fn metric_of(phase: &Phase, ranking_index: usize) -> RoundOfElimination {
    test_utils::metric_of(phase, ranking_index)
}

/// The metric of a ranking that was knocked out in round `round`.
fn out_in(round: u32) -> RoundOfElimination {
    RoundOfElimination(NonZeroU32::new(round))
}

/// Reports `result` for the match of `round` between the two single-ranking teams.
fn report(phase: &mut Phase, round: u32, result: MatchResult) {
    let match_id = match_id(phase, round, &[0], &[1]);
    phase.report_match_result(match_id, result);
}

#[test]
fn initialize_metrics_leaves_every_ranking_still_in() {
    let phase = phase_with(4, vec![]);

    for ranking_index in 0..4 {
        assert_eq!(metric_of(&phase, ranking_index), STILL_IN);
    }
}

#[test]
fn the_loser_of_a_match_that_eliminates_is_out_in_the_round_of_that_match() {
    let mut phase = phase_with(2, vec![eliminating_the_loser((3, vec![0], vec![1]))]);

    report(&mut phase, 3, MatchResult::AWins);

    assert_eq!(metric_of(&phase, 0), STILL_IN);
    assert_eq!(metric_of(&phase, 1), out_in(3));
}

/// A phase whose format does not knock anybody out — a round robin, or the winner bracket of a
/// double elimination — leaves every ranking in.
#[test]
fn a_match_without_the_annotation_knocks_nobody_out() {
    let mut phase = phase_with(2, vec![((1, vec![0], vec![1]), TypeMap::new())]);

    report(&mut phase, 1, MatchResult::AWins);

    assert_eq!(metric_of(&phase, 0), STILL_IN);
    assert_eq!(metric_of(&phase, 1), STILL_IN);
}

#[test]
fn a_draw_knocks_nobody_out() {
    let mut phase = phase_with(2, vec![eliminating_the_loser((1, vec![0], vec![1]))]);

    report(&mut phase, 1, MatchResult::Draw);

    assert_eq!(metric_of(&phase, 0), STILL_IN);
    assert_eq!(metric_of(&phase, 1), STILL_IN);
}

/// The grand final of a double elimination with a reset to come: side B is out if it loses,
/// side A plays the reset instead.
#[test]
fn a_match_that_eliminates_one_side_leaves_the_other_in_when_it_loses() {
    for (result, expected) in [
        (MatchResult::AWins, out_in(1)),
        (MatchResult::BWins, STILL_IN),
    ] {
        let mut phase = phase_with(
            2,
            vec![eliminating(
                (1, vec![0], vec![1]),
                Eliminates::Only(Team::B),
            )],
        );

        report(&mut phase, 1, result);

        assert_eq!(metric_of(&phase, 0), STILL_IN, "side A after {result:?}");
        assert_eq!(metric_of(&phase, 1), expected, "side B after {result:?}");
    }
}

#[test]
fn every_ranking_of_a_losing_team_is_knocked_out() {
    let mut phase = phase_with(4, vec![eliminating_the_loser((1, vec![0, 1], vec![2, 3]))]);
    let match_id = match_id(&phase, 1, &[0, 1], &[2, 3]);

    phase.report_match_result(match_id, MatchResult::BWins);

    assert_eq!(metric_of(&phase, 0), out_in(1));
    assert_eq!(metric_of(&phase, 1), out_in(1));
    assert_eq!(metric_of(&phase, 2), STILL_IN);
    assert_eq!(metric_of(&phase, 3), STILL_IN);
}

#[test]
fn a_ranking_that_is_already_out_keeps_the_round_it_was_knocked_out_in() {
    let mut phase = phase_with(
        2,
        vec![
            eliminating_the_loser((1, vec![0], vec![1])),
            eliminating_the_loser((2, vec![0], vec![1])),
        ],
    );

    report(&mut phase, 1, MatchResult::AWins);
    report(&mut phase, 2, MatchResult::AWins);

    assert_eq!(metric_of(&phase, 1), out_in(1));
}

#[test]
fn a_bye_knocks_nobody_out() {
    let phase = test_utils::phase_with_byes(
        PhaseBuilder::new()
            .add_players(2)
            .add_metric(RoundOfEliminationCalculator),
        vec![],
        vec![(1, 0)],
    );

    assert_eq!(phase.data().byes().len(), 1);
    assert_eq!(metric_of(&phase, 0), STILL_IN);
}

#[test]
fn a_later_elimination_is_better_and_still_being_in_is_best() {
    let compare = |first, second| LaterElimination.compare(&first, &second);
    assert_eq!(compare(STILL_IN, out_in(3)), Comparison::Better);
    assert_eq!(compare(out_in(3), STILL_IN), Comparison::Worse);
    assert_eq!(compare(out_in(3), out_in(2)), Comparison::Better);
    assert_eq!(compare(out_in(2), out_in(3)), Comparison::Worse);
    assert_eq!(compare(out_in(2), out_in(2)), Comparison::Equal);
    assert_eq!(compare(STILL_IN, STILL_IN), Comparison::Equal);
}
