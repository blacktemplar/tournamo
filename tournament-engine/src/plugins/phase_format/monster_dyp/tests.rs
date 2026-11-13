#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::{
    entities::{Match, MatchResult, Phase, PhaseBuilder, PhaseData, RankingId},
    map::Map,
    plugins::metrics_calculator::wins_draws_losses::{WinsDrawsLosses, WinsDrawsLossesCalculator},
};

fn phase(rounds: u32, num_players: u32) -> Phase {
    let mut phase = PhaseBuilder::new()
        .format(MonsterDyp::new(rounds))
        .add_metric(WinsDrawsLossesCalculator)
        .add_players(num_players)
        .build();
    phase.create_new_matches();
    phase
}

fn matches_per_ranking(phase: &PhaseData) -> Map<RankingId, u32> {
    let mut matches_per_ranking: Map<_, _> = phase.rankings().keys().map(|r| (*r, 0)).collect();
    for ranking_id in phase.matches().values().flat_map(Match::rankings) {
        matches_per_ranking[ranking_id] += 1;
    }
    matches_per_ranking
}

#[test]
fn every_ranking_plays_every_round_in_a_team_of_two_if_the_rankings_are_a_multiple_of_four() {
    let phase = phase(3, 8);
    let phase = phase.data();

    assert_eq!(phase.matches().len(), 3 * 2);
    assert!(
        phase
            .matches()
            .values()
            .all(|m| m.team_a().len() == 2 && m.team_b().len() == 2)
    );
    assert!(matches_per_ranking(phase).values().all(|&n| n == 3));
    assert_eq!(
        phase.matches().values().map(Match::round).max(),
        NonZeroU32::new(3)
    );
}

#[test]
fn the_rankings_left_over_take_turns_sitting_out() {
    // 2 of 10 rankings sit out every round, so in 5 rounds every ranking sits out once
    let phase = phase(5, 10);
    let phase = phase.data();

    assert_eq!(phase.matches().len(), 5 * 2);
    assert!(matches_per_ranking(phase).values().all(|&n| n == 4));
    assert_eq!(phase.byes().len(), 5 * 2);
    assert!(phase.byes().values().map(Bye::ranking).all_unique());
}

#[test]
fn the_numbers_of_matches_played_differ_by_at_most_one() {
    // 3 of 7 rankings sit out every round
    let phase = phase(5, 7);
    let phase = phase.data();

    let (min, max) = matches_per_ranking(phase)
        .values()
        .copied()
        .minmax()
        .into_option()
        .unwrap();
    assert_eq!(phase.matches().len(), 5);
    assert!(max - min <= 1, "{min} to {max} matches played");
}

#[test]
fn the_teams_are_drawn_anew_every_round() {
    let phase = phase(5, 8);
    let phase = phase.data();

    let teams = phase
        .matches()
        .values()
        .flat_map(|m| [m.team_a(), m.team_b()])
        .map(|team| {
            team.iter()
                .copied()
                .sorted_by_key(|r| phase.rankings()[r].initial_unique_rank())
                .collect_vec()
        })
        .unique()
        .count();
    assert!(teams > 4, "only {teams} different teams in 5 rounds");
}

#[test]
fn nothing_is_scheduled_for_fewer_than_four_rankings() {
    let phase = phase(5, 3);
    let phase = phase.data();

    assert!(phase.matches().is_empty());
    assert!(phase.byes().is_empty());
}

#[test]
fn nothing_is_scheduled_after_the_first_call() {
    let mut phase = phase(5, 8);
    let num_matches = phase.data().matches().len();

    phase.create_new_matches();

    assert_eq!(phase.data().matches().len(), num_matches);
}

#[test]
fn both_rankings_of_a_team_get_its_result() {
    let mut phase = phase(1, 4);
    let r#match = phase.data().matches().values().next().unwrap();
    let (match_id, winners, losers) = (
        r#match.id(),
        r#match.team_a().clone(),
        r#match.team_b().clone(),
    );

    phase.report_match_result(match_id, MatchResult::AWins);

    for (team, expected) in [
        (
            winners,
            WinsDrawsLosses {
                wins: 1,
                draws: 0,
                losses: 0,
                byes: 0,
            },
        ),
        (
            losers,
            WinsDrawsLosses {
                wins: 0,
                draws: 0,
                losses: 1,
                byes: 0,
            },
        ),
    ] {
        for ranking_id in team {
            assert_eq!(
                phase.data().rankings()[&ranking_id].metric::<WinsDrawsLosses>(),
                Some(&expected)
            );
        }
    }
}
