#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::people::Person;

#[test]
fn entrants_are_listed_by_name_in_the_order_they_were_added() {
    let mut session = Session::new("cup");

    session.add_entrant("Bob").unwrap();
    session.add_entrant("  Alice ").unwrap();

    assert_eq!(names(&session.entrants()), ["Bob", "Alice"]);
}

#[test]
fn an_empty_or_taken_name_adds_no_entrant() {
    let mut session = Session::new("cup");
    session.add_entrant("Alice").unwrap();

    assert!(matches!(
        session.add_entrant(" "),
        Err(AddEntrantError::EmptyName)
    ));
    assert!(matches!(
        session.add_entrant("Alice "),
        Err(AddEntrantError::DuplicateName(name)) if name == "Alice"
    ));
    assert_eq!(names(&session.entrants()), ["Alice"]);
}

#[test]
fn a_batch_is_checked_before_any_entrant_is_added() {
    let mut session = Session::new("cup");
    session.add_entrant("Alice").unwrap();
    let before = session.export();
    let undo = session.next_undo();

    for names in [
        ["Bob", "Alice"],
        ["Bob", "ALICE"],
        ["Bob", "Ａｌｉｃｅ"],
        ["Bob", "Bob"],
        ["Bob", "bob"],
        ["Bob", " "],
    ] {
        let names = names.map(str::to_owned);
        assert!(session.add_named_entrants(&names).is_err());
        assert_eq!(session.export(), before);
        assert_eq!(session.next_undo(), undo);
    }
}

#[test]
fn batch_names_are_tidied_and_undone_together() {
    let mut session = Session::new("cup");
    session
        .add_named_entrants(&[" Alice ".to_owned(), "Bob".to_owned()])
        .unwrap();
    assert_eq!(names(&session.entrants()), ["Alice", "Bob"]);

    session.undo().unwrap();
    assert!(session.entrants().is_empty());
    session.redo().unwrap();
    assert_eq!(names(&session.entrants()), ["Alice", "Bob"]);
}

#[test]
fn a_batch_is_not_added_after_the_start() {
    let mut session = Given::new().started();
    let before = session.export();

    assert!(matches!(
        session.add_named_entrants(&["Eve".to_owned(), "Frank".to_owned()]),
        Err(AddEntrantError::Started)
    ));
    assert_eq!(session.export(), before);
}

/// Names differing in case or in how Unicode composes their characters look alike, which the
/// operator is told of while typing, and are refused, by the name entered.
#[test]
fn a_name_like_one_entered_is_found_and_refused() {
    let mut session = Session::new("cup");
    // "Zoë" with its "ë" as one character
    session.add_entrant("Zo\u{eb}").unwrap();

    assert_eq!(session.entrant_like(" ZOË "), Some("Zoë".to_owned()));
    // with its "ë" as an "e" and a combining diaeresis
    assert_eq!(session.entrant_like("zoe\u{308}"), Some("Zoë".to_owned()));
    assert_eq!(session.entrant_like("Zoe"), None);
    assert_eq!(session.entrant_like(""), None);
    assert!(matches!(
        session.add_entrant("zoë"),
        Err(AddEntrantError::DuplicateName(name)) if name == "Zoë"
    ));
    assert_eq!(names(&session.entrants()), ["Zoë"]);
}

#[test]
fn a_removed_entrant_comes_back_with_its_name_by_undo() {
    let mut session = Session::new("cup");
    let alice = session.add_entrant("Alice").unwrap();
    session.add_entrant("Bob").unwrap();

    session.remove_entrant(alice).unwrap();
    assert_eq!(names(&session.entrants()), ["Bob"]);
    assert!(matches!(
        session.remove_entrant(alice),
        Err(RemoveEntrantError::UnknownEntrant)
    ));
    session.undo().unwrap();

    assert_eq!(names(&session.entrants()), ["Alice", "Bob"]);
}

/// A list adds an entrant for each line not blank, all of them one action.
#[test]
fn a_list_adds_an_entrant_for_each_line() {
    let mut session = Session::new("cup");
    session.add_entrant("Alice").unwrap();

    let pasted = session.add_entrants("Bob\r\n  Carol \n\n Dave\n").unwrap();

    assert_eq!(pasted.added.len(), 3);
    assert!(pasted.refused.is_empty());
    assert_eq!(
        names(&session.entrants()),
        ["Alice", "Bob", "Carol", "Dave"]
    );
    let added = session.entrants()[1..].to_vec();
    assert_eq!(
        session.next_undo(),
        Some(Action::AddEntrants(added.clone()))
    );
    assert_eq!(session.undo().unwrap(), Action::AddEntrants(added.clone()));
    assert_eq!(names(&session.entrants()), ["Alice"]);
    assert_eq!(session.redo().unwrap(), Action::AddEntrants(added));
    assert_eq!(
        names(&session.entrants()),
        ["Alice", "Bob", "Carol", "Dave"]
    );
}

/// A list of one line adds an entrant as [`Session::add_entrant`] does.
#[test]
fn a_list_of_one_line_is_undone_as_one_entrant() {
    let mut session = Session::new("cup");

    session.add_entrants("Alice\n").unwrap();

    let alice = session.entrants()[0].clone();
    assert_eq!(session.undo().unwrap(), Action::AddEntrant(alice));
}

/// The lines of a list with a name taken, by an entrant or an earlier line, or a rating that is
/// no number are refused, each with its number and why; the others are added.
#[test]
fn a_list_adds_its_good_lines_and_names_the_others() {
    let mut session = Session::new("cup");
    session.add_entrant("Alice").unwrap();

    let pasted = session
        .add_entrants("Bob\nALICE\n\nCarol; strong \nbob\nDave")
        .unwrap();

    assert_eq!(names(&session.entrants()), ["Alice", "Bob", "Dave"]);
    let refused: Vec<_> = pasted
        .refused
        .iter()
        .map(|line| (line.number, line.line.as_str(), line.why.to_string()))
        .collect();
    assert_eq!(
        refused,
        [
            (
                2,
                "ALICE",
                "there already is an entrant named “Alice”".to_owned()
            ),
            (
                4,
                "Carol; strong",
                "“strong” is no rating: a rating is a number, such as 1500".to_owned()
            ),
            (
                5,
                "bob",
                "there already is an entrant named “Bob”".to_owned()
            ),
        ]
    );
    let added = session.entrants()[1..].to_vec();
    assert_eq!(session.undo().unwrap(), Action::AddEntrants(added));
    assert!(matches!(
        session.add_entrants(" \n"),
        Err(AddEntrantError::EmptyName)
    ));
}

/// A name typed is read as a line pasted: a rating follows a `;`.
#[test]
fn a_line_typed_is_read_as_one_pasted() {
    let mut session = Session::new("cup");

    session.add_entrants("Bo;1500").unwrap();

    assert_eq!(seeds(&session), [(Some(1500.0), Some(1))]);
    assert_eq!(session.entrant_like("bo;1700"), Some("Bo".to_owned()));
}

/// Whitespace in a name is tidied, so that names differing in it alone are alike.
#[test]
fn whitespace_in_a_name_is_tidied() {
    let mut session = Session::new("cup");

    session.add_entrant(" Zed   Zed ").unwrap();

    assert_eq!(names(&session.entrants()), ["Zed Zed"]);
    assert!(matches!(
        session.add_entrant("zed zed"),
        Err(AddEntrantError::DuplicateName(name)) if name == "Zed Zed"
    ));
    let id = session.entrants()[0].id;
    session.rename_entrant(id, "Zed\t Zedd").unwrap();
    assert_eq!(names(&session.entrants()), ["Zed Zedd"]);
}

/// A name has at most [`MAX_NAME`] characters, an imported one is cut to them.
#[test]
fn a_name_has_at_most_its_characters() {
    let mut session = Session::new("cup");
    let longest = "x".repeat(MAX_NAME);

    let alice = session.add_entrant(&longest).unwrap();

    assert!(matches!(
        session.add_entrant(&format!("{longest}y")),
        Err(AddEntrantError::NameTooLong)
    ));
    assert!(matches!(
        session.rename_entrant(alice, &format!("{longest}y")),
        Err(RenameEntrantError::NameTooLong)
    ));
    let mut long = Session::new(format!("{longest}z"));
    long.add_entrant("Bob").unwrap();
    let id = long.entrants()[0].id;
    long.people.add(Person::new(id, format!("{longest}b")));
    let imported = Session::import(&long.export(), "cup", &[]).unwrap();
    assert_eq!(imported.name(), longest);
    assert_eq!(names(&imported.entrants()), [longest]);
}

/// A file whose people were cut out by hand imports, its entrants without names, which the
/// session counts.
#[test]
fn entrants_without_names_are_counted() {
    let session = Given::new().added();
    let file = session.export();
    let mut file: serde_json::Value = serde_json::from_str(&file).unwrap();
    file["people"].as_array_mut().unwrap().remove(0);

    let imported = Session::import(&file.to_string(), "cup", &[]).unwrap();

    assert_eq!(session.unnamed(), 0);
    assert_eq!(imported.unnamed(), 1);
}

/// A rating is read as the operator's language writes numbers, and may be negative.
#[test]
fn a_rating_is_read_in_the_operators_language() {
    let mut session = Session::new("cup");
    session.set_language(Language::German);

    session
        .add_entrants("Alice;1.800,5\nBob;-5\nCarol;1.000")
        .unwrap();

    assert_eq!(
        seeds(&session),
        [
            (Some(1800.5), Some(1)),
            (Some(-5.0), Some(3)),
            (Some(1000.0), Some(2)),
        ]
    );
}

/// Renaming keeps the entrant's place, also after the start, and is saved; it appends nothing to
/// the log, so undo undoes the action before it.
#[test]
fn an_entrant_is_renamed_in_place() {
    let storage = MemoryStorage::default();
    let mut session = Given::new().started();
    now(session.save(&storage)).unwrap();
    let bob = session.entrants()[1].id;
    let log = session.machine.log().len();

    session.rename_entrant(bob, " Robert ").unwrap();

    assert_eq!(
        names(&session.entrants()),
        ["Alice", "Robert", "Carol", "Dave"]
    );
    assert_eq!(session.machine.log().len(), log);
    assert!(session.has_unsaved_changes());
    now(session.save(&storage)).unwrap();
    let loaded = now(Session::load(&storage, session.key())).unwrap();
    assert_eq!(loaded.entrants(), session.entrants());
    assert_eq!(session.undo().unwrap(), Action::Start);
}

#[test]
fn a_rename_to_an_empty_or_taken_name_is_refused() {
    let mut session = Session::new("cup");
    let alice = session.add_entrant("Alice").unwrap();
    let bob = session.add_entrant("Bob").unwrap();
    session.remove_entrant(bob).unwrap();

    assert!(matches!(
        session.rename_entrant(alice, " "),
        Err(RenameEntrantError::EmptyName)
    ));
    assert!(matches!(
        session.rename_entrant(bob, "Robert"),
        Err(RenameEntrantError::UnknownEntrant)
    ));
    let carol = session.add_entrant("Carol").unwrap();
    assert!(matches!(
        session.rename_entrant(carol, "alice"),
        Err(RenameEntrantError::DuplicateName(name)) if name == "Alice"
    ));
    // differing from its own name in case only
    session.rename_entrant(alice, "ALICE").unwrap();
    assert_eq!(names(&session.entrants()), ["ALICE", "Carol"]);
}

#[test]
fn no_entrant_is_added_or_removed_after_the_start() {
    let mut session = Given::new().started();
    let alice = session.entrants()[0].id;

    assert!(matches!(
        session.add_entrant("Eve"),
        Err(AddEntrantError::Started)
    ));
    assert!(matches!(
        session.remove_entrant(alice),
        Err(RemoveEntrantError::Started)
    ));
}

#[test]
fn a_search_finds_an_entrant_without_a_name_only_when_nothing_is_typed() {
    let nameless = EntrantRow {
        id: EntrantId::new(Uuid::nil()),
        name: None,
        rating: None,
        seed: None,
    };

    assert!(NameSearch::new("").finds(&nameless));
    assert!(!NameSearch::new("a").finds(&nameless));
}

/// The id of the entrant named `name`.
fn id_of(session: &Session, name: &str) -> EntrantId {
    session
        .entrants()
        .into_iter()
        .find(|row| row.name.as_deref() == Some(name))
        .expect("an entrant of that name")
        .id
}

/// Rates the entrant named `name` by `rating`, a number as typed.
fn rate(session: &mut Session, name: &str, rating: &str) {
    let id = id_of(session, name);
    session
        .set_rating(id, Rating::parse(rating, Language::English))
        .unwrap();
}

/// The rating and seed of each entrant, in the order they were added.
fn seeds(session: &Session) -> Vec<(Option<f64>, Option<u32>)> {
    session
        .entrants()
        .iter()
        .map(|row| (row.rating.map(Rating::get), row.seed))
        .collect()
}

#[test]
fn a_rated_entrant_is_seeded_by_its_rating_equal_ratings_sharing_a_seed() {
    let mut session = Given::new().added();
    let changes = session.changes();

    rate(&mut session, "Alice", "1500");
    rate(&mut session, "Bob", "1700,5");
    rate(&mut session, "Carol", "1500");

    assert_eq!(
        seeds(&session),
        [
            (Some(1500.0), Some(2)),
            (Some(1700.5), Some(1)),
            (Some(1500.0), Some(2)),
            (None, None),
        ]
    );
    assert_eq!(session.changes(), changes + 3);
    let bob = id_of(&session, "Bob");
    session.rename_entrant(bob, "Robert").unwrap();
    session.set_rating(id_of(&session, "Alice"), None).unwrap();
    assert_eq!(
        seeds(&session),
        [
            (None, None),
            (Some(1700.5), Some(1)),
            (Some(1500.0), Some(2)),
            (None, None),
        ]
    );
}

#[test]
fn the_start_orders_the_registration_by_rating_then_by_lot() {
    let mut session = Given::new().added();
    rate(&mut session, "Dave", "2000");
    rate(&mut session, "Bob", "1800");

    session.start().unwrap();

    let competition = session.competition();
    let registration = competition.registration();
    assert_eq!(
        registration[..2],
        [id_of(&session, "Dave"), id_of(&session, "Bob")]
    );
    let rest: Vec<_> = competition
        .lots()
        .iter()
        .filter(|id| registration[2..].contains(id))
        .copied()
        .collect();
    assert_eq!(registration[2..], rest);
}

#[test]
fn equal_ratings_are_a_tier_ordered_by_lot() {
    let mut session = Given::new().added();
    for name in ["Alice", "Bob", "Carol", "Dave"] {
        rate(&mut session, name, "1500");
    }

    session.start().unwrap();

    assert_eq!(
        session.competition().registration(),
        session.competition().lots()
    );
}

/// Without any rating the start gives no order: the competition starts as it did before ratings.
#[test]
fn without_ratings_the_start_gives_no_order() {
    let session = Given::new().started();

    let log = serde_json::to_value(session.machine.log()).unwrap();
    let start = log
        .as_array()
        .unwrap()
        .iter()
        .find_map(|entry| entry.pointer("/kind/Apply/StartCompetition"))
        .unwrap();
    assert_eq!(start, &serde_json::json!({ "order": [] }));
}

#[test]
fn a_rating_is_entered_as_typed_and_blank_unrates() {
    let mut session = Given::new().added();
    let alice = id_of(&session, "Alice");

    session.enter_rating(alice, " 1500 ").unwrap();
    assert_eq!(seeds(&session)[0], (Some(1500.0), Some(1)));
    assert!(matches!(
        session.enter_rating(alice, "strong"),
        Err(RateEntrantError::NoRating(typed)) if typed == "strong"
    ));
    assert_eq!(seeds(&session)[0], (Some(1500.0), Some(1)));
    session.enter_rating(alice, " ").unwrap();
    assert_eq!(seeds(&session)[0], (None, None));
}

#[test]
fn ratings_do_not_change_after_the_start() {
    let mut session = Given::new().started();
    let alice = id_of(&session, "Alice");

    assert!(matches!(
        session.set_rating(alice, Rating::parse("1500", Language::English)),
        Err(RateEntrantError::RegistrationClosed)
    ));
    assert!(matches!(
        session.set_rating(EntrantId::new(Uuid::nil()), None),
        Err(RateEntrantError::UnknownEntrant)
    ));
    assert_eq!(seeds(&session)[0], (None, None));
}

/// A line pasted may give a rating after a tab or a `;`.
#[test]
fn a_list_rates_the_entrants_whose_lines_give_a_rating() {
    let mut session = Session::new("cup");

    session
        .add_entrants("Alice;1500\nBob\t1700\nCarol;\nDave\n")
        .unwrap();

    assert_eq!(
        names(&session.entrants()),
        ["Alice", "Bob", "Carol", "Dave"]
    );
    assert_eq!(
        seeds(&session),
        [
            (Some(1500.0), Some(2)),
            (Some(1700.0), Some(1)),
            (None, None),
            (None, None),
        ]
    );
}

#[test]
fn ratings_are_exported_with_the_people() {
    let mut session = Given::new().added();
    rate(&mut session, "Alice", "1500");

    let imported = Session::import(&session.export(), "cup", &[]).unwrap();

    assert_eq!(seeds(&imported), seeds(&session));
}
