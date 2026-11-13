#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;

/// How long a phase takes shows before the start: a round robin of 24 takes 23 rounds and 276
/// matches, a double elimination of 8 one match more if the grand final is played again, and a
/// phase that does not start with the entrants entered nothing.
#[test]
fn the_scale_of_a_phase_is_known_before_the_start() {
    let names: Vec<&'static str> = (1..=24)
        .map(|number| &*format!("Entrant {number}").leak())
        .collect();
    let scale =
        |session: &Session, format: &str| session.scale(&session.phase_draft(format).unwrap());
    let exactly = |count| Count {
        least: count,
        most: count,
    };

    assert_eq!(
        scale(&Session::new("cup"), RoundRobin::ID),
        None,
        "nobody entered"
    );
    let mut session = Session::new("cup");
    for name in &names {
        session.add_entrant(name).unwrap();
    }
    assert_eq!(
        scale(&session, RoundRobin::ID),
        Some(Scale {
            rounds: exactly(23),
            matches: exactly(276)
        })
    );
    let eight = Given::new()
        .entrants(&names[..8])
        .format(DoubleElimination::ID)
        .added();
    assert_eq!(
        scale(&eight, DoubleElimination::ID).map(|scale| scale.matches),
        Some(Count {
            least: 14,
            most: 15
        })
    );
}
