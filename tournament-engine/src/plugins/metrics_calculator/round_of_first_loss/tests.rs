#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::{
    entities::{MatchResult, Phase, PhaseBuilder},
    plugins::metrics_calculator::test_utils::{self, MatchSpec, match_id},
};

/// A ranking that has not lost a match yet, which is the best this metric gets.
const UNBEATEN: RoundOfFirstLoss = RoundOfFirstLoss(None);

/// Builds a phase with `num_players` rankings and all matches of `matches` already scheduled.
///
/// [`RoundOfFirstLossCalculator`] reads nothing but the result and the round of the match it is
/// called for, so it is the only calculator of the phase.
fn phase_with(num_players: u32, matches: Vec<MatchSpec>) -> Phase {
    test_utils::phase_with(
        PhaseBuilder::new()
            .add_players(num_players)
            .add_metric(RoundOfFirstLossCalculator),
        matches,
    )
}

fn metric_of(phase: &Phase, ranking_index: usize) -> RoundOfFirstLoss {
    test_utils::metric_of(phase, ranking_index)
}

/// The metric of a ranking whose first loss was in round `round`.
fn lost_in(round: u32) -> RoundOfFirstLoss {
    RoundOfFirstLoss(NonZeroU32::new(round))
}

/// Reports `result` for the match of `round` between the two single-ranking teams.
fn report(phase: &mut Phase, round: u32, result: MatchResult) {
    let match_id = match_id(phase, round, &[0], &[1]);
    phase.report_match_result(match_id, result);
}

#[test]
fn initialize_metrics_leaves_every_ranking_unbeaten() {
    let phase = phase_with(4, vec![]);

    for ranking_index in 0..4 {
        assert_eq!(metric_of(&phase, ranking_index), UNBEATEN);
    }
}

#[test]
fn the_loser_of_a_match_lost_in_the_round_of_that_match() {
    let mut phase = phase_with(2, vec![(3, vec![0], vec![1])]);

    report(&mut phase, 3, MatchResult::AWins);

    assert_eq!(metric_of(&phase, 0), UNBEATEN);
    assert_eq!(metric_of(&phase, 1), lost_in(3));
}

#[test]
fn a_draw_is_not_a_loss() {
    let mut phase = phase_with(2, vec![(1, vec![0], vec![1])]);

    report(&mut phase, 1, MatchResult::Draw);

    assert_eq!(metric_of(&phase, 0), UNBEATEN);
    assert_eq!(metric_of(&phase, 1), UNBEATEN);
}

#[test]
fn every_ranking_of_a_losing_team_lost() {
    let mut phase = phase_with(4, vec![(1, vec![0, 1], vec![2, 3])]);
    let match_id = match_id(&phase, 1, &[0, 1], &[2, 3]);

    phase.report_match_result(match_id, MatchResult::BWins);

    assert_eq!(metric_of(&phase, 0), lost_in(1));
    assert_eq!(metric_of(&phase, 1), lost_in(1));
    assert_eq!(metric_of(&phase, 2), UNBEATEN);
    assert_eq!(metric_of(&phase, 3), UNBEATEN);
}

#[test]
fn a_later_loss_does_not_replace_the_first_one() {
    let mut phase = phase_with(2, vec![(1, vec![0], vec![1]), (2, vec![0], vec![1])]);

    report(&mut phase, 1, MatchResult::AWins);
    report(&mut phase, 2, MatchResult::AWins);

    assert_eq!(metric_of(&phase, 1), lost_in(1));
}

/// Results arrive in whatever order they are reported in, so a loss of an earlier round can
/// follow one of a later round and is then the first loss.
#[test]
fn the_earliest_round_lost_counts_whenever_it_is_reported() {
    let mut phase = phase_with(2, vec![(1, vec![0], vec![1]), (2, vec![0], vec![1])]);

    report(&mut phase, 2, MatchResult::AWins);
    assert_eq!(metric_of(&phase, 1), lost_in(2));

    report(&mut phase, 1, MatchResult::AWins);

    assert_eq!(metric_of(&phase, 1), lost_in(1));
}

#[test]
fn a_bye_is_never_lost() {
    let phase = test_utils::phase_with_byes(
        PhaseBuilder::new()
            .add_players(2)
            .add_metric(RoundOfFirstLossCalculator),
        vec![],
        vec![(1, 0)],
    );

    assert_eq!(phase.data().byes().len(), 1);
    assert_eq!(metric_of(&phase, 0), UNBEATEN);
}

#[test]
fn a_later_first_loss_is_better_and_never_having_lost_is_best() {
    let compare = |first, second| LaterFirstLoss.compare(&first, &second);
    assert_eq!(compare(UNBEATEN, lost_in(3)), Comparison::Better);
    assert_eq!(compare(lost_in(3), UNBEATEN), Comparison::Worse);
    assert_eq!(compare(lost_in(3), lost_in(2)), Comparison::Better);
    assert_eq!(compare(lost_in(2), lost_in(3)), Comparison::Worse);
    assert_eq!(compare(lost_in(2), lost_in(2)), Comparison::Equal);
    assert_eq!(compare(UNBEATEN, UNBEATEN), Comparison::Equal);
}
