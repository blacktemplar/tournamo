use itertools::Itertools;
use uuid::Uuid;
use vec1::vec1;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;

const SEED: Seed = Seed::new(*b"draw-seed-12");

fn entrant(index: usize) -> EntrantId {
    EntrantId::new(Uuid::from_u128(index as u128))
}

fn place(place: usize) -> NonZeroU32 {
    NonZeroU32::new(place as u32).unwrap()
}

/// A field of `count` qualifiers of the registration.
fn registered(count: usize) -> Vec<Qualifier> {
    (0..count)
        .map(|index| Qualifier {
            entrants: vec1![entrant(index)],
            origin: Origin::Registration {
                place: place(index + 1),
            },
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
                field.push(Qualifier {
                    entrants: vec1![entrant(field.len())],
                    origin: Origin::Phase {
                        phase,
                        division,
                        rank: place(rank),
                        unique_rank: place(rank),
                        overall_rank: place(overall),
                        overall_unique_rank: place(overall),
                    },
                });
            }
        }
    }
    field
}

fn draw(draw: &dyn Draw, field: &[Qualifier], divisions: usize) -> Vec<Vec<usize>> {
    let entrants = IdMap::new();
    let dealt = draw.draw(&DrawView::new(field, divisions, &entrants, SEED));
    assert_eq!(dealt.len(), divisions);
    assert!(
        dealt.iter().flatten().copied().sorted().eq(0..field.len()),
        "every position is dealt once: {dealt:?}"
    );
    dealt
}

#[test]
fn in_order_keeps_the_field_and_deals_several_divisions_back_and_forth() {
    assert_eq!(draw(&InOrder, &registered(4), 1), [vec![0, 1, 2, 3]]);
    assert_eq!(
        draw(&InOrder, &registered(11), 3),
        [vec![0, 5, 6], vec![1, 4, 7, 10], vec![2, 3, 8, 9]]
    );
}

#[test]
fn random_deals_evenly_and_always_the_same_for_a_seed() {
    let field = registered(11);

    let dealt = draw(&Random, &field, 3);

    assert_eq!(dealt.iter().map(Vec::len).collect_vec(), [4, 4, 3]);
    assert_ne!(dealt, draw(&InOrder, &field, 3));
    assert_eq!(dealt, draw(&Random, &field, 3));
}

#[test]
fn blocks_deal_consecutive_places_the_first_divisions_one_more() {
    assert_eq!(
        draw(&Blocks, &registered(10), 3),
        [vec![0, 1, 2, 3], vec![4, 5, 6], vec![7, 8, 9]]
    );
    assert_eq!(
        draw(&Blocks, &registered(5), 4),
        [vec![0, 1], vec![2], vec![3], vec![4]]
    );
    assert_eq!(Blocks.order(), DivisionOrder::Levels);
    assert_eq!(InOrder.order(), DivisionOrder::Parallel);
}

/// The group of each seed of the order `GroupWinnersApart` seeds the field of `sending` by.
fn groups_by_seed(sending: &[usize]) -> Vec<usize> {
    let field = of_groups(sending);
    let [order] = &draw(&GroupWinnersApart, &field, 1)[..] else {
        unreachable!("one division")
    };
    order
        .iter()
        .map(|&position| match field[position].origin {
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
        let [order] = &draw(&GroupWinnersApart, &field, 1)[..] else {
            unreachable!("one division")
        };
        let rank = |position: usize| match field[position].origin {
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

    assert_eq!(draw(&GroupWinnersApart, &field, 1), [vec![0, 1, 2, 3]]);
}

#[test]
fn groups_kept_apart_seed_qualifiers_of_the_registration_first() {
    let mut field = registered(2);
    field.extend(of_groups(&[1, 1]));

    assert_eq!(draw(&GroupWinnersApart, &field, 1), [vec![0, 1, 2, 3]]);
}
