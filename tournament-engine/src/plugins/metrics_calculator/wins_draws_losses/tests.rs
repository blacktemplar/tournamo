#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::{
    entities::{MatchResult, Phase, PhaseBuilder},
    plugins::metrics_calculator::test_utils::{self, MatchSpec, match_id},
};

/// Builds a phase with `num_players` rankings whose wins/draws/losses got initialized and
/// with all matches of `matches` already scheduled.
fn phase_with(num_players: u32, matches: Vec<MatchSpec>) -> Phase {
    test_utils::phase_with(
        PhaseBuilder::new()
            .add_players(num_players)
            .add_metric(WinsDrawsLossesCalculator),
        matches,
    )
}

fn metric_of(phase: &Phase, ranking_index: usize) -> WinsDrawsLosses {
    test_utils::metric_of(phase, ranking_index)
}

const fn wins_draws_losses(wins: u32, draws: u32, losses: u32) -> WinsDrawsLosses {
    WinsDrawsLosses {
        wins,
        draws,
        losses,
        byes: 0,
    }
}

#[test]
fn initialize_metrics_starts_every_ranking_at_zero() {
    let phase = phase_with(4, vec![]);

    for ranking_index in 0..4 {
        assert_eq!(metric_of(&phase, ranking_index), wins_draws_losses(0, 0, 0));
    }
}

#[test]
fn a_wins_counts_as_win_for_team_a_and_as_loss_for_team_b() {
    let mut phase = phase_with(4, vec![(1, vec![0], vec![1]), (1, vec![2], vec![3])]);
    let match_id = match_id(&phase, 1, &[0], &[1]);

    phase.report_match_result(match_id, MatchResult::AWins);

    assert_eq!(metric_of(&phase, 0), wins_draws_losses(1, 0, 0));
    assert_eq!(metric_of(&phase, 1), wins_draws_losses(0, 0, 1));
    // the rankings of the other match are untouched
    assert_eq!(metric_of(&phase, 2), wins_draws_losses(0, 0, 0));
    assert_eq!(metric_of(&phase, 3), wins_draws_losses(0, 0, 0));
}

#[test]
fn b_wins_counts_as_win_for_team_b_and_as_loss_for_team_a() {
    let mut phase = phase_with(2, vec![(1, vec![0], vec![1])]);
    let match_id = match_id(&phase, 1, &[0], &[1]);

    phase.report_match_result(match_id, MatchResult::BWins);

    assert_eq!(metric_of(&phase, 0), wins_draws_losses(0, 0, 1));
    assert_eq!(metric_of(&phase, 1), wins_draws_losses(1, 0, 0));
}

#[test]
fn draw_counts_as_draw_for_both_teams() {
    let mut phase = phase_with(2, vec![(1, vec![0], vec![1])]);
    let match_id = match_id(&phase, 1, &[0], &[1]);

    phase.report_match_result(match_id, MatchResult::Draw);

    assert_eq!(metric_of(&phase, 0), wins_draws_losses(0, 1, 0));
    assert_eq!(metric_of(&phase, 1), wins_draws_losses(0, 1, 0));
}

#[test]
fn every_ranking_of_a_team_gets_the_result_of_the_team() {
    let mut phase = phase_with(4, vec![(1, vec![0, 1], vec![2, 3])]);
    let match_id = match_id(&phase, 1, &[0, 1], &[2, 3]);

    phase.report_match_result(match_id, MatchResult::BWins);

    assert_eq!(metric_of(&phase, 0), wins_draws_losses(0, 0, 1));
    assert_eq!(metric_of(&phase, 1), wins_draws_losses(0, 0, 1));
    assert_eq!(metric_of(&phase, 2), wins_draws_losses(1, 0, 0));
    assert_eq!(metric_of(&phase, 3), wins_draws_losses(1, 0, 0));
}

#[test]
fn results_accumulate_over_multiple_matches() {
    let mut phase = phase_with(
        4,
        vec![
            (1, vec![0], vec![1]),
            (1, vec![2], vec![3]),
            (2, vec![0], vec![2]),
            (2, vec![1], vec![3]),
            (3, vec![1], vec![2]),
        ],
    );

    for (round, team_a, team_b, result) in [
        (1, vec![0], vec![1], MatchResult::AWins),
        (1, vec![2], vec![3], MatchResult::Draw),
        (2, vec![0], vec![2], MatchResult::BWins),
        (2, vec![1], vec![3], MatchResult::AWins),
        (3, vec![1], vec![2], MatchResult::BWins),
    ] {
        let match_id = match_id(&phase, round, &team_a, &team_b);
        phase.report_match_result(match_id, result);
    }

    assert_eq!(metric_of(&phase, 0), wins_draws_losses(1, 0, 1));
    assert_eq!(metric_of(&phase, 1), wins_draws_losses(1, 0, 2));
    assert_eq!(metric_of(&phase, 2), wins_draws_losses(2, 1, 0));
    assert_eq!(metric_of(&phase, 3), wins_draws_losses(0, 1, 1));
}

#[test]
fn a_bye_is_counted_as_soon_as_it_is_scheduled_but_not_as_a_match() {
    let mut phase = test_utils::phase_with_byes(
        PhaseBuilder::new()
            .add_players(3)
            .add_metric(WinsDrawsLossesCalculator),
        vec![(1, vec![0], vec![1])],
        vec![(1, 2), (2, 2)],
    );
    assert_eq!(metric_of(&phase, 2).byes, 1);

    // finishing the first round schedules the bye of the second one
    let match_id = match_id(&phase, 1, &[0], &[1]);
    phase.report_match_result(match_id, MatchResult::AWins);

    let with_two_byes = metric_of(&phase, 2);
    assert_eq!(with_two_byes.byes, 2);
    assert_eq!(with_two_byes.matches_played(), 0);
    assert_eq!(metric_of(&phase, 0), wins_draws_losses(1, 0, 0));
}

/// How `criterion` orders a ranking with `first` against one with `second`.
fn comparison(
    criterion: &impl RankingCriterion<Metric = WinsDrawsLosses>,
    first: WinsDrawsLosses,
    second: WinsDrawsLosses,
) -> Comparison {
    criterion.compare(&first, &second)
}

#[test]
fn more_wins_are_better() {
    assert_eq!(
        comparison(
            &MoreWins,
            wins_draws_losses(2, 0, 0),
            wins_draws_losses(1, 0, 0)
        ),
        Comparison::Better
    );
    assert_eq!(
        comparison(
            &MoreWins,
            wins_draws_losses(1, 0, 0),
            wins_draws_losses(2, 0, 0)
        ),
        Comparison::Worse
    );
    // the same number of wins ties, however the draws and losses differ
    assert_eq!(
        comparison(
            &MoreWins,
            wins_draws_losses(1, 3, 0),
            wins_draws_losses(1, 0, 3)
        ),
        Comparison::Equal
    );
}

#[test]
fn more_draws_are_better() {
    assert_eq!(
        comparison(
            &MoreDraws,
            wins_draws_losses(0, 2, 0),
            wins_draws_losses(0, 1, 0)
        ),
        Comparison::Better
    );
    assert_eq!(
        comparison(
            &MoreDraws,
            wins_draws_losses(0, 1, 0),
            wins_draws_losses(0, 2, 0)
        ),
        Comparison::Worse
    );
    // the same number of draws ties, however the wins and losses differ
    assert_eq!(
        comparison(
            &MoreDraws,
            wins_draws_losses(3, 1, 0),
            wins_draws_losses(0, 1, 3)
        ),
        Comparison::Equal
    );
}

#[test]
fn fewer_losses_are_better() {
    assert_eq!(
        comparison(
            &FewerLosses,
            wins_draws_losses(0, 0, 1),
            wins_draws_losses(0, 0, 2)
        ),
        Comparison::Better
    );
    assert_eq!(
        comparison(
            &FewerLosses,
            wins_draws_losses(0, 0, 2),
            wins_draws_losses(0, 0, 1)
        ),
        Comparison::Worse
    );
    // the same number of losses ties, however the wins and draws differ
    assert_eq!(
        comparison(
            &FewerLosses,
            wins_draws_losses(3, 0, 1),
            wins_draws_losses(0, 3, 1)
        ),
        Comparison::Equal
    );
}

#[test]
fn matches_played_counts_every_result() {
    assert_eq!(wins_draws_losses(0, 0, 0).matches_played(), 0);
    assert_eq!(wins_draws_losses(2, 1, 3).matches_played(), 6);
}

#[test]
fn fewer_played_matches_are_better() {
    assert_eq!(
        comparison(
            &FewerMatchesPlayed,
            wins_draws_losses(1, 0, 0),
            wins_draws_losses(1, 1, 0)
        ),
        Comparison::Better
    );
    assert_eq!(
        comparison(
            &FewerMatchesPlayed,
            wins_draws_losses(1, 1, 0),
            wins_draws_losses(1, 0, 0)
        ),
        Comparison::Worse
    );
    // only how many matches were played counts, not how they went
    assert_eq!(
        comparison(
            &FewerMatchesPlayed,
            wins_draws_losses(3, 0, 0),
            wins_draws_losses(0, 1, 2)
        ),
        Comparison::Equal
    );
}
