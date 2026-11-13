use std::num::NonZero;

use itertools::Itertools;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::entities::{Bye, Match, PhaseBuilder, PhaseData};

fn assert_every_pair_played_at_most_once(phase: &PhaseData) {
    assert!(phase.matches().values().all(|m| m.team_a().len() == 1));
    assert!(phase.matches().values().all(|m| m.team_b().len() == 1));
    assert!(
        phase
            .matches()
            .values()
            .map(|m| (m.team_a()[0], m.team_b()[0]))
            .all_unique()
    );
}

#[test]
fn random_rounds_schedules_number_of_rounds_with_no_match_overlaps() {
    let mut phase = PhaseBuilder::new()
        .format(RandomRounds::new(3))
        .add_players(6)
        .build();

    phase.create_new_matches();

    let phase = phase.data();
    assert_every_pair_played_at_most_once(phase);
    assert_eq!(phase.matches().len(), 3 * 3);
    assert_eq!(
        phase.matches().values().map(Match::round).max(),
        Some(NonZero::new(3).unwrap())
    );
}

#[test]
fn random_rounds_schedules_number_of_rounds_with_no_match_overlaps_with_odd_players() {
    let mut phase = PhaseBuilder::new()
        .format(RandomRounds::new(3))
        .add_players(7)
        .build();

    phase.create_new_matches();

    let phase = phase.data();
    assert_every_pair_played_at_most_once(phase);
    assert_eq!(phase.matches().len(), 3 * 3);
    assert_eq!(
        phase.matches().values().map(Match::round).max(),
        Some(NonZero::new(3).unwrap())
    );
}

#[test]
fn random_rounds_with_odd_players_give_a_different_ranking_a_bye_every_round() {
    let mut phase = PhaseBuilder::new()
        .format(RandomRounds::new(3))
        .add_players(7)
        .build();

    phase.create_new_matches();

    let phase = phase.data();
    assert_eq!(
        phase
            .byes()
            .values()
            .map(|bye| bye.round().get())
            .sorted()
            .collect_vec(),
        [1, 2, 3]
    );
    assert!(phase.byes().values().map(Bye::ranking).all_unique());
}

#[test]
fn random_rounds_if_rounds_is_larger_than_players_does_full_round_robin() {
    let mut phase = PhaseBuilder::new()
        .format(RandomRounds::new(8))
        .add_players(7)
        .build();

    phase.create_new_matches();

    let phase = phase.data();
    assert_every_pair_played_at_most_once(phase);
    assert_eq!(phase.matches().len(), 7 * 3);
    assert_eq!(
        phase.matches().values().map(Match::round).max(),
        Some(NonZero::new(7).unwrap())
    );
}

/// More rounds keep the rounds drawn for fewer, so that rounds can be added to a phase whose
/// first ones are played.
#[test]
fn more_rounds_keep_the_rounds_drawn_for_fewer() {
    let drawn = |rounds| {
        let mut phase = PhaseBuilder::new()
            .format(RandomRounds::new(rounds))
            .add_players(8)
            .build();
        phase.create_new_matches();
        phase
            .data()
            .matches()
            .values()
            .map(Match::id)
            .collect::<Vec<_>>()
    };

    let (fewer, more) = (drawn(3), drawn(5));

    assert_eq!(more.len(), 5 * 4);
    assert_eq!(fewer, more[..3 * 4]);
}
