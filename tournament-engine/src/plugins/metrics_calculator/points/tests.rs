#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::{
    entities::{MatchResult, Phase, PhaseBuilder},
    plugins::metrics_calculator::{
        test_utils::{self, MatchSpec, match_id},
        wins_draws_losses::WinsDrawsLossesCalculator,
    },
};

/// The scoring system of most association football leagues.
const THREE_ONE: PointsCalculator = PointsCalculator::new(Score::new(3.0), Score::ONE);
/// The scoring system association football used before it moved to [`THREE_ONE`], still in
/// use e.g. in ice hockey.
const TWO_ONE: PointsCalculator = PointsCalculator::new(Score::new(2.0), Score::ONE);
/// The scoring system of chess and most other games scoring a draw as half a win.
const ONE_HALF: PointsCalculator = PointsCalculator::new(Score::ONE, Score::new(0.5));

fn points(text: &str) -> Points {
    Points(text.parse().unwrap())
}

/// Builds a phase with `num_players` rankings and all matches of `matches` already scheduled,
/// scoring finished matches with `calculator`.
///
/// [`PointsCalculator`] derives its metric from [`WinsDrawsLosses`], so the phase has to
/// update that metric first.
fn phase_with(num_players: u32, matches: Vec<MatchSpec>, calculator: PointsCalculator) -> Phase {
    test_utils::phase_with(
        PhaseBuilder::new()
            .add_players(num_players)
            .add_metric(WinsDrawsLossesCalculator)
            .add_metric(calculator),
        matches,
    )
}

fn metric_of(phase: &Phase, ranking_index: usize) -> Points {
    test_utils::metric_of(phase, ranking_index)
}

/// Plays all of [`SCENARIO`](test_utils::SCENARIO), scoring with `calculator`.
fn play_scenario(calculator: PointsCalculator) -> Phase {
    test_utils::play_scenario(
        phase_with(4, test_utils::scenario_matches(), calculator),
        0..test_utils::SCENARIO.len(),
    )
}

#[test]
fn initialize_metrics_starts_every_ranking_at_zero() {
    let phase = phase_with(4, vec![], THREE_ONE);

    for ranking_index in 0..4 {
        assert_eq!(metric_of(&phase, ranking_index), points("0"));
    }
}

#[test]
fn three_points_for_a_win_and_one_for_a_draw() {
    let phase = play_scenario(THREE_ONE);

    assert_eq!(metric_of(&phase, 0), points("3"));
    assert_eq!(metric_of(&phase, 1), points("3"));
    assert_eq!(metric_of(&phase, 2), points("7"));
    assert_eq!(metric_of(&phase, 3), points("1"));
}

#[test]
fn two_points_for_a_win_and_one_for_a_draw() {
    let phase = play_scenario(TWO_ONE);

    assert_eq!(metric_of(&phase, 0), points("2"));
    assert_eq!(metric_of(&phase, 1), points("2"));
    assert_eq!(metric_of(&phase, 2), points("5"));
    assert_eq!(metric_of(&phase, 3), points("1"));
}

#[test]
fn one_point_for_a_win_and_half_a_point_for_a_draw() {
    let phase = play_scenario(ONE_HALF);

    assert_eq!(metric_of(&phase, 0), points("1"));
    assert_eq!(metric_of(&phase, 1), points("1"));
    assert_eq!(metric_of(&phase, 2), points("2.5"));
    assert_eq!(metric_of(&phase, 3), points("0.5"));
}

#[test]
fn half_points_for_draws_add_up_without_rounding() {
    let matches = vec![
        (1, vec![0], vec![1]),
        (2, vec![0], vec![1]),
        (3, vec![0], vec![1]),
    ];
    let mut phase = phase_with(2, matches, ONE_HALF);

    for round in 1..=3 {
        let match_id = match_id(&phase, round, &[0], &[1]);
        phase.report_match_result(match_id, MatchResult::Draw);
    }

    assert_eq!(metric_of(&phase, 0), points("1.5"));
    assert_eq!(metric_of(&phase, 1), points("1.5"));
}

#[test]
fn a_loss_is_worth_no_points() {
    let mut phase = phase_with(2, vec![(1, vec![0], vec![1])], THREE_ONE);
    let match_id = match_id(&phase, 1, &[0], &[1]);

    phase.report_match_result(match_id, MatchResult::BWins);

    assert_eq!(metric_of(&phase, 0), points("0"));
    assert_eq!(metric_of(&phase, 1), points("3"));
}

#[test]
fn only_the_rankings_of_the_reported_match_get_points() {
    let matches = vec![(1, vec![0], vec![1]), (1, vec![2], vec![3])];
    let mut phase = phase_with(4, matches, THREE_ONE);
    let match_id = match_id(&phase, 1, &[0], &[1]);

    phase.report_match_result(match_id, MatchResult::AWins);

    assert_eq!(metric_of(&phase, 0), points("3"));
    assert_eq!(metric_of(&phase, 1), points("0"));
    // the rankings of the other match are still at their initial value
    assert_eq!(metric_of(&phase, 2), points("0"));
    assert_eq!(metric_of(&phase, 3), points("0"));
}

#[test]
fn every_ranking_of_a_team_gets_the_points_of_the_team() {
    let mut phase = phase_with(4, vec![(1, vec![0, 1], vec![2, 3])], THREE_ONE);
    let match_id = match_id(&phase, 1, &[0, 1], &[2, 3]);

    phase.report_match_result(match_id, MatchResult::Draw);

    for ranking_index in 0..4 {
        assert_eq!(metric_of(&phase, ranking_index), points("1"));
    }
}

/// Builds a phase with three rankings, in which `2` has a bye in round 1 and `0` beats `1`
/// in round 2, scoring with `calculator`.
fn phase_with_a_bye(calculator: PointsCalculator) -> Phase {
    let mut phase = test_utils::phase_with_byes(
        PhaseBuilder::new()
            .add_players(3)
            .add_metric(WinsDrawsLossesCalculator)
            .add_metric(calculator),
        vec![(2, vec![0], vec![1])],
        vec![(1, 2)],
    );
    let match_id = match_id(&phase, 2, &[0], &[1]);
    phase.report_match_result(match_id, MatchResult::AWins);
    phase
}

#[test]
fn a_bye_is_worth_no_points_by_default() {
    let phase = phase_with_a_bye(ONE_HALF);

    assert_eq!(metric_of(&phase, 2), points("0"));
}

#[test]
fn a_bye_is_worth_the_points_for_a_bye() {
    let phase = phase_with_a_bye(ONE_HALF.with_points_for_bye(Score::ONE));

    assert_eq!(metric_of(&phase, 0), points("1"));
    assert_eq!(metric_of(&phase, 1), points("0"));
    assert_eq!(metric_of(&phase, 2), points("1"));
}

#[test]
fn the_points_of_a_bye_add_to_those_of_matches() {
    let mut phase = test_utils::phase_with_byes(
        PhaseBuilder::new()
            .add_players(2)
            .add_metric(WinsDrawsLossesCalculator)
            .add_metric(THREE_ONE.with_points_for_bye(Score::new(2.0))),
        vec![(2, vec![0], vec![1])],
        vec![(1, 0)],
    );
    let match_id = match_id(&phase, 2, &[0], &[1]);

    phase.report_match_result(match_id, MatchResult::Draw);

    assert_eq!(metric_of(&phase, 0), points("3"));
    assert_eq!(metric_of(&phase, 1), points("1"));
}

#[test]
#[should_panic(expected = "called `Option::unwrap()` on a `None` value")]
fn scoring_without_the_wins_draws_losses_metric_panics() {
    let mut phase = test_utils::phase_with(
        PhaseBuilder::new().add_players(2).add_metric(THREE_ONE),
        vec![(1, vec![0], vec![1])],
    );
    let match_id = match_id(&phase, 1, &[0], &[1]);

    phase.report_match_result(match_id, MatchResult::AWins);
}

#[test]
fn more_points_are_better() {
    let compare = |first, second| MorePoints.compare(&points(first), &points(second));
    assert_eq!(compare("3", "1"), Comparison::Better);
    assert_eq!(compare("1", "3"), Comparison::Worse);
    assert_eq!(compare("1.5", "1"), Comparison::Better);
    assert_eq!(compare("1.5", "1.5"), Comparison::Equal);
}
