use std::num::NonZeroU32;

use itertools::Itertools;
use uuid::Uuid;
use vec1::{Vec1, vec1};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::Origin;

const SEED: Seed = Seed::new(*b"teams-tests!");

fn entrant(index: usize) -> EntrantId {
    EntrantId::new(Uuid::from_u128(index as u128))
}

fn place(place: usize) -> NonZeroU32 {
    NonZeroU32::new(place as u32).unwrap()
}

/// A field of `count` single entrants of the registration, entrant `i` at position `i`.
fn registered(count: usize) -> Vec<Qualifier> {
    (0..count)
        .map(|index| {
            Qualifier::new(
                vec1![entrant(index)],
                Origin::Registration {
                    place: place(index + 1),
                },
            )
        })
        .collect()
}

/// The teams `teams` forms of `field`, as the positions in the field of their entrants.
fn form(teams: &dyn Teams, field: &[Qualifier]) -> Vec<Vec<usize>> {
    let entrants = IdMap::new();
    teams
        .form(&TeamsView::new(field, &entrants, SEED))
        .iter()
        .map(|team| {
            team.iter()
                .map(|member| {
                    (0..)
                        .find(|&index| entrant(index) == *member)
                        .expect("an entrant of the field")
                })
                .collect()
        })
        .collect()
}

#[test]
fn keep_makes_every_qualifier_a_team_of_its_entrants() {
    let pairs = vec![
        Qualifier::new(
            vec1![entrant(0), entrant(1)],
            Origin::Registration { place: place(1) },
        ),
        Qualifier::new(
            vec1![entrant(2), entrant(3)],
            Origin::Registration { place: place(2) },
        ),
    ];

    assert_eq!(form(&Keep, &registered(3)), [[0], [1], [2]]);
    assert_eq!(form(&Keep, &pairs), [[0, 1], [2, 3]]);
    assert!(Keep.fits(3));
    assert_eq!(Keep.entrants_per_team(place(2)), place(2));
}

#[test]
fn best_with_worst_pairs_the_ends_ordered_by_the_sum_of_their_places() {
    // every pair sums to the same, so the better position decides
    assert_eq!(
        form(&BestWithWorst, &registered(6)),
        [[0, 5], [1, 4], [2, 3]]
    );
    assert!(BestWithWorst.fits(6));
    assert!(!BestWithWorst.fits(5));
    assert_eq!(BestWithWorst.entrants_per_team(NonZeroU32::MIN), place(2));
}

#[test]
fn random_pairs_are_ordered_by_the_sum_of_their_places_then_the_better_place() {
    let teams = form(&RandomPairs, &registered(10));

    assert_eq!(
        teams,
        form(&RandomPairs, &registered(10)),
        "a seed draws once"
    );
    assert!(
        teams.iter().flatten().copied().sorted().eq(0..10),
        "every entrant is in one pair: {teams:?}"
    );
    assert!(
        teams
            .iter()
            .all(|team| team.len() == 2 && team[0] < team[1]),
        "the better place first: {teams:?}"
    );
    assert!(
        teams
            .iter()
            .map(|team| (team[0] + team[1], team[0]))
            .is_sorted(),
        "{teams:?}"
    );
    assert_ne!(teams, form(&BestWithWorst, &registered(10)), "drawn by lot");
    assert!(RandomPairs.fits(10));
    assert!(!RandomPairs.fits(9));
}

#[test]
fn split_makes_every_entrant_a_team_in_the_order_of_the_field() {
    let origin = Origin::Registration { place: place(1) };
    let field = vec![
        Qualifier::new(vec1![entrant(3), entrant(1)], origin),
        Qualifier::new(vec1![entrant(0), entrant(2)], origin),
    ];

    assert_eq!(form(&Split, &field), [[3], [1], [0], [2]]);
    assert!(Split.fits(1));
    assert_eq!(Split.entrants_per_team(place(2)), NonZeroU32::MIN);
}

#[test]
fn a_pair_lists_the_entrant_of_the_better_place_first() {
    let teams = teams_of(&registered(4), [(3, 0), (2, 1)]);

    assert_eq!(
        teams,
        [
            Vec1::try_from_vec(vec![entrant(0), entrant(3)]).unwrap(),
            Vec1::try_from_vec(vec![entrant(1), entrant(2)]).unwrap(),
        ]
    );
}
