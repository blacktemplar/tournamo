use serde_json::{Value, json};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::{
    plugins::metrics_calculator::{
        buchholz::{Buchholz, BuchholzOfBuchholz, SonnebornBerger},
        points::Points,
        round_of_elimination::RoundOfElimination,
        round_of_first_loss::RoundOfFirstLoss,
        wins_draws_losses::WinsDrawsLosses,
    },
    score::Score,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Count {
    count: u32,
}

#[typetag::serde]
impl TypeMapValue for Count {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Label {
    text: String,
    weight: f64,
}

#[typetag::serde]
impl TypeMapValue for Label {}

/// A value whose fields are themselves compound, so that a round trip has to preserve more
/// than a flat list of scalars.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct History {
    entries: Vec<Count>,
    finished: bool,
}

#[typetag::serde]
impl TypeMapValue for History {}

/// A map holding one value of every test type.
fn filled_map() -> TypeMap {
    let mut map = TypeMap::new();
    map.insert(Count { count: 7 });
    map.insert(Label {
        text: "buchholz".to_owned(),
        weight: -0.5,
    });
    map.insert(History {
        entries: vec![Count { count: 1 }, Count { count: 2 }],
        finished: true,
    });
    map
}

fn round_trip(map: &TypeMap) -> TypeMap {
    let json = serde_json::to_string(map).expect("serializing a type map must not fail");
    serde_json::from_str(&json).expect("a serialized type map must deserialize again")
}

/// The serialized values of `map` as a set, since the order in which the values of a
/// [`TypeMap`] get serialized is an implementation detail.
fn serialized_values(map: &TypeMap) -> Vec<Value> {
    let Value::Array(mut values) = serde_json::to_value(map).unwrap() else {
        panic!("a type map must serialize to a sequence");
    };
    values.sort_by_key(ToString::to_string);
    values
}

#[test]
fn an_empty_map_serializes_to_an_empty_sequence() {
    assert_eq!(serde_json::to_string(&TypeMap::new()).unwrap(), "[]");
}

#[test]
fn every_value_is_serialized_as_one_tagged_element() {
    let mut expected = vec![
        json!({ "type": "Count", "value": { "count": 7 } }),
        json!({ "type": "Label", "value": { "text": "buchholz", "weight": -0.5 } }),
        json!({
            "type": "History",
            "value": {
                "entries": [{ "count": 1 }, { "count": 2 }],
                "finished": true,
            },
        }),
    ];
    expected.sort_by_key(ToString::to_string);

    assert_eq!(serialized_values(&filled_map()), expected);
}

#[test]
fn an_empty_map_round_trips() {
    let map = round_trip(&TypeMap::new());

    assert_eq!(map.0.len(), 0);
    assert_eq!(map.get::<Count>(), None);
}

#[test]
fn every_value_survives_a_round_trip() {
    let map = round_trip(&filled_map());

    assert_eq!(map.0.len(), 3);
    assert_eq!(map.get::<Count>(), Some(&Count { count: 7 }));
    assert_eq!(
        map.get::<Label>(),
        Some(&Label {
            text: "buchholz".to_owned(),
            weight: -0.5,
        })
    );
    assert_eq!(
        map.get::<History>(),
        Some(&History {
            entries: vec![Count { count: 1 }, Count { count: 2 }],
            finished: true,
        })
    );
}

/// The tag of a metric is part of every snapshot saved, so a change to it is a breaking one
/// and has to be deliberate. Pinning them here makes a rename of a type fail this test instead
/// of silently invalidating old snapshots.
#[test]
fn metric_types_of_the_engine_are_serialized_under_their_explicitly_chosen_tags() {
    let map = TypeMap::new()
        .with(WinsDrawsLosses::default())
        .with(Points::default())
        .with(Buchholz::default())
        .with(BuchholzOfBuchholz::default())
        .with(SonnebornBerger::default())
        .with(RoundOfFirstLoss::default())
        .with(RoundOfElimination::default());

    let value = serde_json::to_value(&map).unwrap();

    let tags: Vec<_> = value
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["type"].as_str().unwrap())
        .collect();
    assert_eq!(
        tags,
        [
            "tournament_engine::wins_draws_losses",
            "tournament_engine::points",
            "tournament_engine::buchholz",
            "tournament_engine::buchholz_of_buchholz",
            "tournament_engine::sonneborn_berger",
            "tournament_engine::round_of_first_loss",
            "tournament_engine::round_of_elimination",
        ]
    );
}

#[test]
fn metric_types_of_the_engine_survive_a_round_trip() {
    let mut map = TypeMap::new();
    map.insert(Points(Score::new(2.5)));
    map.insert(WinsDrawsLosses {
        wins: 3,
        draws: 1,
        losses: 2,
        byes: 1,
    });

    let map = round_trip(&map);

    assert_eq!(map.get::<Points>(), Some(&Points(Score::new(2.5))));
    assert_eq!(
        map.get::<WinsDrawsLosses>(),
        Some(&WinsDrawsLosses {
            wins: 3,
            draws: 1,
            losses: 2,
            byes: 1,
        })
    );
}

#[test]
fn a_round_trip_does_not_change_the_serialized_form() {
    let map = filled_map();
    let round_tripped = round_trip(&map);

    assert_eq!(serialized_values(&round_tripped), serialized_values(&map));
}

#[test]
fn round_tripped_values_are_keyed_by_their_concrete_type() {
    let mut map = TypeMap::new();
    map.insert(Count { count: 7 });

    let mut map = round_trip(&map);

    // deserialization has to key the value by the type id of `Count`, not by the type id of
    // `Box<dyn TypeMapValue>` or anything else it got deserialized as
    *map.get_mut::<Count>().expect("the count must be found") = Count { count: 8 };
    assert_eq!(map.get::<Count>(), Some(&Count { count: 8 }));
    assert_eq!(map.get::<Label>(), None);
}

#[test]
fn deserializing_the_same_type_twice_keeps_the_last_value() {
    let json = r#"[
        { "type": "Count", "value": { "count": 1 } },
        { "type": "Count", "value": { "count": 2 } }
    ]"#;

    let map: TypeMap = serde_json::from_str(json).unwrap();

    assert_eq!(map.0.len(), 1);
    assert_eq!(map.get::<Count>(), Some(&Count { count: 2 }));
}

#[test]
fn deserializing_an_unknown_tag_fails() {
    let json = r#"[{ "type": "NotAKnownTypeMapValue" }]"#;

    let error = serde_json::from_str::<TypeMap>(json).unwrap_err();

    assert!(
        error.to_string().contains("NotAKnownTypeMapValue"),
        "the error must name the unknown tag, but was: {error}"
    );
}

#[test]
fn deserializing_something_that_is_not_a_sequence_fails() {
    let json = r#"{ "Count": { "count": 1 } }"#;

    assert!(serde_json::from_str::<TypeMap>(json).is_err());
}

#[test]
fn deserializing_a_value_with_a_missing_field_fails() {
    let json = r#"[{ "type": "Label", "value": { "text": "buchholz" } }]"#;

    let error = serde_json::from_str::<TypeMap>(json).unwrap_err();

    assert!(
        error.to_string().contains("weight"),
        "the error must name the missing field, but was: {error}"
    );
}
