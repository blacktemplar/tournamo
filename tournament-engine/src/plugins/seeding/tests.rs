use std::num::NonZeroU32;

use itertools::Itertools;
use uuid::Uuid;
use vec1::vec1;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::plugins::draw::{Blocks, Draw, DrawView, Random, Snake};

const SEED: Seed = Seed::new(*b"seeding-seed");

fn entrant(index: usize) -> EntrantId {
    EntrantId::new(Uuid::from_u128(index as u128))
}

fn place(place: usize) -> NonZeroU32 {
    NonZeroU32::new(place as u32).unwrap()
}

/// A field of `count` qualifiers of the registration.
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

/// The field of the best `sending[g]` of each group `g` of a phase, by overall standings: every
/// group's best first, then every group's second, and so on.
fn of_groups(sending: &[usize]) -> Vec<Qualifier> {
    let phase = PhaseId::new(Uuid::nil());
    let ranks = sending.iter().copied().max().unwrap_or(0);
    let mut field = Vec::new();
    for rank in 1..=ranks {
        for (division, &sends) in sending.iter().enumerate() {
            if rank <= sends {
                let overall = field.len() + 1;
                field.push(Qualifier::new(
                    vec1![entrant(field.len())],
                    Origin::Phase {
                        phase,
                        division,
                        rank: place(rank),
                        unique_rank: place(rank),
                        overall_rank: place(overall),
                        overall_unique_rank: place(overall),
                    },
                ));
            }
        }
    }
    field
}

/// The order `seeding` gives the positions `dealt` of `field` as division `division` of
/// `divisions`, checked to be a permutation of them.
fn seed_dealt(
    seeding: &dyn Seeding,
    field: &[Qualifier],
    dealt: &[usize],
    (division, divisions): (usize, usize),
) -> Vec<usize> {
    let entrants = IdMap::new();
    let view = SeedingView::new(field, dealt, (division, divisions), &entrants, SEED);
    let order = seeding.seed(&view);
    assert!(
        order.iter().copied().sorted().eq(dealt.iter().copied()),
        "{order:?} orders {dealt:?}"
    );
    order
}

/// The order `seeding` gives the whole of `field` as one division.
fn seed(seeding: &dyn Seeding, field: &[Qualifier]) -> Vec<usize> {
    let dealt: Vec<_> = (0..field.len()).collect();
    seed_dealt(seeding, field, &dealt, (0, 1))
}

#[test]
fn in_order_keeps_the_order_dealt() {
    assert_eq!(
        seed_dealt(&SeededInOrder, &registered(9), &[1, 4, 7], (1, 3)),
        [1, 4, 7]
    );
}

#[test]
fn at_random_shuffles_and_always_the_same_for_a_seed() {
    let field = registered(16);

    let order = seed(&SeededAtRandom, &field);

    assert_ne!(order, seed(&SeededInOrder, &field));
    assert_eq!(order, seed(&SeededAtRandom, &field));
}

/// Knockouts A/B/C in blocks, each seeded at random: every division gets a block of consecutive
/// places, in an order of its own.
#[test]
fn blocks_seeded_at_random_keep_the_blocks() {
    let field = registered(12);
    let entrants = IdMap::new();
    let dealt = Blocks.draw(&DrawView::new(&field, 3, &entrants, SEED));

    for (division, positions) in dealt.iter().enumerate() {
        let order = seed_dealt(&SeededAtRandom, &field, positions, (division, 3));
        assert_eq!(order.iter().copied().sorted().collect_vec(), *positions);
        assert_ne!(order, *positions, "division {division} is shuffled");
    }
}

/// Every draw with every seeding deals each qualifier to exactly one division, none empty
/// unless the field is, and orders the qualifiers of each.
#[test]
fn every_draw_and_seeding_partition_the_field_and_order_each_division() {
    let draws: [&dyn Draw; 3] = [&Snake, &Random, &Blocks];
    let seedings: [&dyn Seeding; 3] = [&SeededInOrder, &SeededAtRandom, &GroupWinnersApart];
    let entrants = IdMap::new();
    for (count, divisions) in (0..=40).cartesian_product(1..=6) {
        if count > 0 && count < divisions {
            continue;
        }
        let field = registered(count);
        for draw in draws {
            let mut dealt = draw.draw(&DrawView::new(&field, divisions, &entrants, SEED));
            assert_eq!(dealt.len(), divisions);
            assert!(
                dealt.iter().flatten().copied().sorted().eq(0..count),
                "{} deals every position once: {dealt:?}",
                draw.id()
            );
            assert!(count == 0 || dealt.iter().all(|positions| !positions.is_empty()));
            for (division, positions) in dealt.iter_mut().enumerate() {
                positions.sort_unstable();
                for seeding in seedings {
                    seed_dealt(seeding, &field, positions, (division, divisions));
                }
            }
        }
    }
}

/// The group of each seed of the order `GroupWinnersApart` seeds the field of `sending` by.
fn groups_by_seed(sending: &[usize]) -> Vec<usize> {
    let field = of_groups(sending);
    seed(&GroupWinnersApart, &field)
        .iter()
        .map(|&position| match *field[position].origins.first() {
            Origin::Phase { division, .. } => division,
            Origin::Registration { .. } => unreachable!("every qualifier is of a group"),
        })
        .collect()
}

/// The pairs of seeds meeting in the first round of a bracket of `seeds`.
fn first_round(seeds: usize) -> impl Iterator<Item = (usize, usize)> {
    let size = seeds.next_power_of_two();
    (0..size / 2)
        .map(move |seed| (seed, size - 1 - seed))
        .filter(move |&(_, opponent)| opponent < seeds)
}

#[test]
fn groups_kept_apart_meet_neither_in_the_first_round_nor_before_the_final() {
    for groups in 2..=8 {
        let group_of = groups_by_seed(&vec![2; groups]);
        let size = group_of.len().next_power_of_two();
        let top_half: Vec<_> = seeding_order(size.trailing_zeros())
            .into_iter()
            .take(size / 2)
            .collect();

        for (seed, opponent) in first_round(group_of.len()) {
            assert_ne!(
                group_of[seed], group_of[opponent],
                "{groups} groups: seeds {seed} and {opponent} meet"
            );
        }
        for group in 0..groups {
            let [best, second] = group_of
                .iter()
                .positions(|&of| of == group)
                .collect_array()
                .unwrap();
            assert_ne!(
                top_half.contains(&best),
                top_half.contains(&second),
                "{groups} groups: the best two of group {group} are in one half"
            );
        }
        // the best of the groups are the best seeds
        assert!((0..groups).all(|seed| group_of[..groups].contains(&seed)));
    }
}

#[test]
fn groups_kept_apart_are_ordered_by_rank_in_the_group() {
    for sending in [vec![3, 3, 2], vec![2, 2, 2, 1], vec![4, 4], vec![3; 5]] {
        let group_of = groups_by_seed(&sending);
        let field = of_groups(&sending);
        let order = seed(&GroupWinnersApart, &field);
        let rank = |position: usize| match *field[position].origins.first() {
            Origin::Phase { unique_rank, .. } => unique_rank,
            Origin::Registration { .. } => unreachable!("every qualifier is of a group"),
        };
        assert!(order.iter().map(|&p| rank(p)).is_sorted(), "{sending:?}");
        for (seed, opponent) in first_round(group_of.len()) {
            assert_ne!(group_of[seed], group_of[opponent], "{sending:?}");
        }
    }
}

#[test]
fn groups_kept_apart_keep_the_order_by_rank_where_nothing_can_be_kept_apart() {
    let field = of_groups(&[4]);

    assert_eq!(seed(&GroupWinnersApart, &field), [0, 1, 2, 3]);
}

#[test]
fn groups_kept_apart_seed_qualifiers_of_the_registration_first() {
    let mut field = registered(2);
    field.extend(of_groups(&[1, 1]));

    assert_eq!(seed(&GroupWinnersApart, &field), [0, 1, 2, 3]);
}
