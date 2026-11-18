#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::form::Editable as _;

/// How long a phase takes shows before the start: a round robin of 24 takes 23 rounds and 276
/// matches, a double elimination of 8 one match more if the grand final is played again, and a
/// phase that does not start with the entrants entered nothing.
#[test]
fn the_scale_of_a_phase_is_known_before_the_start() {
    let names: Vec<&'static str> = (1..=24)
        .map(|number| &*format!("Entrant {number}").leak())
        .collect();
    let scale = |session: &Session, format: &str| {
        session.scale(&form(session.phase_draft(format).unwrap()))
    };
    let exactly = |count| Count {
        least: count,
        most: Some(count),
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
            entrants: 24,
            teams: 24,
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
            most: Some(15)
        })
    );
}

/// A phase that may take more rounds than a competition usually plays is warned of, also one of
/// too many matches to be played out, whose rounds are known only to be at least as many.
#[test]
fn a_phase_of_too_many_rounds_is_warned_of() {
    let warning = |least, most| {
        Scale {
            entrants: 60,
            teams: 60,
            rounds: Count { least, most },
            matches: Count { least: 0, most },
        }
        .warning()
    };
    let many =
        |count: usize| Some(crate::i18n::Text::new("warning-many-rounds").with("count", count));

    assert_eq!(warning(50, Some(50)), None);
    assert_eq!(warning(40, Some(51)), many(51));
    assert_eq!(warning(59, None), many(59));
}

/// More groups than the entrants fill are said while the phase is configured, with how many they
/// fill.
#[test]
fn a_phase_says_while_configured_that_it_has_too_many_groups() {
    let names: Vec<&'static str> = (1..=11)
        .map(|number| &*format!("Entrant {number}").leak())
        .collect();
    let unfit = |divisions| {
        let session = Given::new().entrants(&names).divisions(divisions).added();
        session.unfit(&session.phase_form_to_edit(session.first()).unwrap())
    };

    assert_eq!(unfit(5), None);
    // what the field did not take is not counted with what it held before
    let session = Given::new().entrants(&names).divisions(20).added();
    let mut form = session.phase_form_to_edit(session.first()).unwrap();
    form.enter(&ConfigPath::ROOT.field("divisions"), "abc")
        .unwrap();
    assert_eq!(session.unfit(&form), None);
    assert_eq!(session.scale(&form), None);
    assert_eq!(
        unfit(20),
        Some(
            crate::i18n::Text::new("warning-too-many-groups")
                .with("divisions", 20_usize)
                .with("count", 40_usize)
                .with("teams", 11_usize)
                .with("most", 5_usize)
        )
    );
}
