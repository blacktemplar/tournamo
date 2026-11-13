#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;

#[test]
fn a_reported_result_comes_with_its_match() {
    let mut session = Given::new().started();
    let first = matches(&session).remove(0);

    let reported = session.report(first.id, MatchResult::BWins).unwrap();

    assert_eq!(
        reported.row,
        MatchRow {
            result: Some(MatchResult::BWins),
            ..first
        }
    );
}

#[test]
fn a_result_the_match_does_not_take_is_refused() {
    let mut session = Given::new().format(SingleElimination::ID).started();
    let first = numbered(&session, 1);
    let elsewhere = numbered(&Given::new().started(), 1);

    assert!(!session.round(round(1)).unwrap().draws);
    assert!(matches!(
        session.report(first, MatchResult::Draw),
        Err(ReportError::DrawNotAllowed)
    ));
    session.report(first, MatchResult::AWins).unwrap();
    assert!(matches!(
        session.report(first, MatchResult::BWins),
        Err(ReportError::AlreadyFinished)
    ));
    assert!(matches!(
        session.report(elsewhere, MatchResult::AWins),
        Err(ReportError::UnknownMatch)
    ));
    assert!(matches!(
        Given::new().added().report(first, MatchResult::AWins),
        Err(ReportError::NotStarted)
    ));
}

/// The result of a match reported long ago is undone by its id, which leaves every result reported
/// after it alone, unlike `undo`, which would walk back through all of them.
#[test]
fn a_result_is_undone_by_its_match() {
    let mut session = Given::new().started();
    for number in 1..=3 {
        session
            .report(numbered(&session, number), MatchResult::AWins)
            .unwrap();
    }

    let reopened = session.undo_report(numbered(&session, 1)).unwrap();

    assert_eq!(reopened.unscheduled, 0);
    assert_eq!(
        (reopened.row.number, reopened.row.result),
        (1, Some(MatchResult::AWins)),
        "the match with the result it had"
    );
    let results: Vec<_> = matches(&session).iter().map(|row| row.result).collect();
    assert_eq!(
        results[..3],
        [None, Some(MatchResult::AWins), Some(MatchResult::AWins)]
    );
    assert!(matches!(
        session.undo_report(numbered(&session, 1)),
        Err(UndoReportError::NotReported)
    ));
}

/// Undoing the result of a semifinal unschedules the final it decided the participants of, so a
/// final that has a result itself has to be undone first.
#[test]
fn a_result_a_later_one_depends_on_is_undone_after_it() {
    let mut session = Given::new().format(SingleElimination::ID).started();
    for number in 1..=3 {
        session
            .report(numbered(&session, number), MatchResult::AWins)
            .unwrap();
    }
    let semifinal = numbered(&session, 1);

    assert!(matches!(
        session.undo_report(semifinal),
        Err(UndoReportError::DependedOnBy(numbers)) if numbers == [3]
    ));
    session.undo_report(numbered(&session, 3)).unwrap();
    let reopened = session.undo_report(semifinal).unwrap();

    assert_eq!(
        reopened.unscheduled, 2,
        "the final and the match for third place go with it"
    );
}

/// The winner of a semifinal plays the final and its loser the match for third place, so undoing
/// it names both once they have results.
#[test]
fn every_result_depending_on_an_undone_one_is_named() {
    let mut session = Given::new().format(SingleElimination::ID).started();
    for number in 1..=4 {
        session
            .report(numbered(&session, number), MatchResult::AWins)
            .unwrap();
    }

    let result = session.undo_report(numbered(&session, 1));

    assert!(
        matches!(&result, Err(UndoReportError::DependedOnBy(numbers)) if *numbers == [3, 4]),
        "{:?}",
        result.err()
    );
}
