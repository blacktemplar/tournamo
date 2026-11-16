use std::num::NonZeroU32;

use itertools::Itertools;
use uuid::Uuid;
use vec1::vec1;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::Origin;

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
fn snake_keeps_the_field_and_deals_several_divisions_back_and_forth() {
    assert_eq!(draw(&Snake, &registered(4), 1), [vec![0, 1, 2, 3]]);
    assert_eq!(
        draw(&Snake, &registered(11), 3),
        [vec![0, 5, 6], vec![1, 4, 7, 10], vec![2, 3, 8, 9]]
    );
}

#[test]
fn random_deals_evenly_and_always_the_same_for_a_seed() {
    let field = registered(11);

    let dealt = draw(&Random, &field, 3);

    assert_eq!(dealt.iter().map(Vec::len).collect_vec(), [4, 4, 3]);
    assert_ne!(dealt, draw(&Snake, &field, 3));
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
    assert_eq!(Snake.order(), DivisionOrder::Parallel);
}
