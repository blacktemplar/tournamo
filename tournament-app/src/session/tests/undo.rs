#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;

#[test]
fn undo_and_redo_walk_back_and_forth_through_the_actions() {
    let mut session = Session::new("cup");
    assert!(
        matches!(session.undo(), Err(UndoError::NothingToUndo)),
        "the seed is no action of the session"
    );
    session.add_entrant("Alice").unwrap();
    session.add_entrant("Bob").unwrap();

    session.undo().unwrap();
    session.undo().unwrap();
    assert!(session.entrants().is_empty());
    assert!(matches!(session.undo(), Err(UndoError::NothingToUndo)));
    session.redo().unwrap();
    assert_eq!(names(&session.entrants()), ["Alice"]);

    session.add_entrant("Carol").unwrap();
    assert!(
        matches!(session.redo(), Err(RedoError::NothingToRedo)),
        "a new action discards what could be redone"
    );
    assert_eq!(names(&session.entrants()), ["Alice", "Carol"]);
}

#[test]
fn the_action_an_undo_would_undo_is_told_before() {
    let mut session = Given::new().started();
    assert_eq!(session.next_undo(), Some(Action::Start));
    let first = matches(&session).remove(0);

    session.report(first.id, MatchResult::AWins).unwrap();

    assert!(matches!(session.next_undo(), Some(Action::Report(_))));
    session.undo().unwrap();
    assert_eq!(session.next_undo(), Some(Action::Start));
    assert_eq!(Session::new("cup").next_undo(), None);
}

#[test]
fn undo_and_redo_tell_the_action_they_walked_through() {
    let mut session = Given::new().started();
    let first = matches(&session).remove(0);
    session.report(first.id, MatchResult::AWins).unwrap();
    session.undo_report(first.id).unwrap();
    let reported = MatchRow {
        result: Some(MatchResult::AWins),
        ..first
    };

    assert_eq!(session.undo().unwrap(), Action::Reopen(reported.clone()));
    assert_eq!(session.undo().unwrap(), Action::Report(reported.clone()));
    assert_eq!(session.undo().unwrap(), Action::Start);
    assert_eq!(session.redo().unwrap(), Action::Start);
    assert_eq!(session.redo().unwrap(), Action::Report(reported));

    let mut session = Session::new("cup");
    let alice = session.add_entrant("Alice").unwrap();
    session.remove_entrant(alice).unwrap();
    let alice = EntrantRow {
        id: alice,
        name: Some("Alice".to_owned()),
    };
    assert_eq!(
        session.undo().unwrap(),
        Action::RemoveEntrant(alice.clone())
    );
    assert_eq!(session.undo().unwrap(), Action::AddEntrant(alice));
}

/// `undo_report` is an action like any other, so `undo` brings the result back rather than undoing
/// the action before it.
#[test]
fn undo_brings_back_a_result_undone_by_its_match() {
    let mut session = Given::new().started();
    let first = numbered(&session, 1);
    session.report(first, MatchResult::AWins).unwrap();
    session.undo_report(first).unwrap();

    session.undo().unwrap();

    assert_eq!(matches(&session)[0].result, Some(MatchResult::AWins));
}
