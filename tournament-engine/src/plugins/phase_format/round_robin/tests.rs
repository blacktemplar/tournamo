use std::num::NonZero;

use itertools::Itertools;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::entities::{Bye, Match, PhaseBuilder, PhaseData};

/// A format asking for no numbers gets its matches numbered in the order it scheduled them.
#[test]
fn matches_are_numbered_as_scheduled_unless_the_format_numbers_them() {
    let mut phase = PhaseBuilder::new()
        .format(RoundRobin)
        .add_players(4)
        .build();

    phase.create_new_matches();

    let numbers = phase
        .data()
        .matches()
        .values()
        .map(|r#match| r#match.number().get())
        .collect_vec();
    assert_eq!(numbers, [1, 2, 3, 4, 5, 6]);
}

/// Asserts that every unordered pair of ranking ids in the phase played against each other
/// in exactly one scheduled match.
fn assert_every_pair_played_exactly_once(phase: &PhaseData) {
    let mut expected_pairs = phase
        .rankings()
        .keys()
        .copied()
        .array_combinations::<2>()
        .collect_vec();
    for r#match in phase.matches().values() {
        let team_a_id = r#match.team_a()[0];
        let team_b_id = r#match.team_b()[0];
        let position = expected_pairs
            .iter()
            .position(|&[a, b]| {
                (a, b) == (team_a_id, team_b_id) || (a, b) == (team_b_id, team_a_id)
            })
            .expect("every scheduled match must be an expected pair that wasn't played yet");
        expected_pairs.remove(position);
    }
    assert!(
        expected_pairs.is_empty(),
        "not all pairs got scheduled, missing: {expected_pairs:?}"
    );
}

#[test]
fn round_robin_schedules_every_pair_exactly_once_for_even_number_of_players() {
    let mut phase = PhaseBuilder::new().add_players(6).build();

    phase.create_new_matches();

    assert_every_pair_played_exactly_once(phase.data());
    assert_eq!(
        phase.data().matches().values().map(Match::round).max(),
        Some(NonZero::new(5).unwrap())
    );
}

#[test]
fn round_robin_schedules_every_pair_exactly_once_for_odd_number_of_players() {
    let mut phase = PhaseBuilder::new().add_players(7).build();

    phase.create_new_matches();

    assert_every_pair_played_exactly_once(phase.data());
    assert_eq!(
        phase.data().matches().values().map(Match::round).max(),
        Some(NonZero::new(7).unwrap())
    );
}

#[test]
fn every_ranking_has_one_bye_in_a_round_robin_of_an_odd_number_of_players() {
    let mut phase = PhaseBuilder::new().add_players(7).build();

    phase.create_new_matches();

    let byes = phase.data().byes();
    assert_eq!(byes.len(), 7);
    assert!(byes.values().map(Bye::ranking).all_unique());
    assert!(byes.values().map(Bye::round).all_unique());
}

#[test]
fn nobody_has_a_bye_in_a_round_robin_of_an_even_number_of_players() {
    let mut phase = PhaseBuilder::new().add_players(6).build();

    phase.create_new_matches();

    assert!(phase.data().byes().is_empty());
}
