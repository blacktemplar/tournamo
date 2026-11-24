#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;

const NOON: Timestamp = Timestamp::from_unix_millis(1_764_000_000_000);
const A_MINUTE_LATER: Timestamp = Timestamp::from_unix_millis(1_764_000_060_000);
const TWO_MINUTES_LATER: Timestamp = Timestamp::from_unix_millis(1_764_000_120_000);

/// When each entry of the log of `session` was appended, oldest first.
fn times(session: &Session) -> Vec<Option<Timestamp>> {
    session
        .machine
        .log()
        .iter()
        .map(|entry| entry.meta().at())
        .collect()
}

#[test]
fn every_entry_a_session_appends_is_dated() {
    let session = Given::new().entrants(&["Alice", "Bob", "Carol"]).started();

    assert!(times(&session).iter().all(Option::is_some));
}

#[test]
fn an_undo_and_a_redo_are_dated_when_they_are_made() {
    let mut session = Given::new().started();
    let first = numbered(&session, 1);

    session.set_clock(|| NOON);
    session
        .report(session.first(), first, MatchResult::AWins)
        .unwrap();
    session.set_clock(|| A_MINUTE_LATER);
    session.undo().unwrap();
    session.set_clock(|| TWO_MINUTES_LATER);
    session.redo().unwrap();

    assert_eq!(
        times(&session).split_off(session.machine.log().len() - 3),
        [Some(NOON), Some(A_MINUTE_LATER), Some(TWO_MINUTES_LATER)]
    );
}

#[test]
fn loading_and_catching_up_keep_when_each_entry_was_appended() {
    let storage = MemoryStorage::default();
    let mut changing = Given::new().started();
    changing.set_clock(|| NOON);
    now(changing.save(&storage)).unwrap();
    let mut following = now(Session::load(&storage, changing.key())).unwrap();

    changing
        .report(changing.first(), numbered(&changing, 1), MatchResult::AWins)
        .unwrap();
    now(changing.save(&storage)).unwrap();
    now(following.catch_up(&storage)).unwrap();

    assert_eq!(times(&following), times(&changing));
    assert_eq!(times(&following).last(), Some(&Some(NOON)));
}
