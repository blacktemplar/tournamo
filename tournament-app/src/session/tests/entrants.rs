#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;

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
fn batch_names_are_kept_as_supplied_and_undone_one_at_a_time() {
    let mut session = Session::new("cup");
    session
        .add_named_entrants(&[" Alice ".to_owned(), "Bob".to_owned()])
        .unwrap();
    assert_eq!(names(&session.entrants()), [" Alice ", "Bob"]);

    session.undo().unwrap();
    assert_eq!(names(&session.entrants()), [" Alice "]);
    session.undo().unwrap();
    assert!(session.entrants().is_empty());
    session.redo().unwrap();
    session.redo().unwrap();
    assert_eq!(names(&session.entrants()), [" Alice ", "Bob"]);
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

/// A list adds an entrant for each line not blank, each an action of its own.
#[test]
fn a_list_adds_an_entrant_for_each_line() {
    let mut session = Session::new("cup");
    session.add_entrant("Alice").unwrap();

    let added = session.add_entrants("Bob\r\n  Carol \n\n Dave\n").unwrap();

    assert_eq!(added.len(), 3);
    assert_eq!(
        names(&session.entrants()),
        ["Alice", "Bob", "Carol", "Dave"]
    );
    session.undo().unwrap();
    assert_eq!(names(&session.entrants()), ["Alice", "Bob", "Carol"]);
}

/// A list with a name taken, by an entrant or an earlier line, adds nobody.
#[test]
fn a_list_with_a_name_taken_adds_nobody() {
    let mut session = Session::new("cup");
    session.add_entrant("Alice").unwrap();

    assert!(matches!(
        session.add_entrants("Bob\nALICE"),
        Err(AddEntrantError::DuplicateName(name)) if name == "Alice"
    ));
    assert!(matches!(
        session.add_entrants("Bob\nCarol\nbob"),
        Err(AddEntrantError::DuplicateName(name)) if name == "Bob"
    ));
    assert!(matches!(
        session.add_entrants(" \n"),
        Err(AddEntrantError::EmptyName)
    ));
    assert_eq!(names(&session.entrants()), ["Alice"]);
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
    };

    assert!(NameSearch::new("").finds(&nameless));
    assert!(!NameSearch::new("a").finds(&nameless));
}
