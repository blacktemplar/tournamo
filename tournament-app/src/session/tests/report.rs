#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;

#[test]
fn a_reported_result_comes_with_its_match() {
    let mut session = Given::new().started();
    let first = matches(&session).remove(0);

    let reported = session
        .report(session.first(), first.id, MatchResult::BWins)
        .unwrap();

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

    assert!(!session.round(session.all(), round(1)).unwrap().draws);
    assert!(matches!(
        session.report(session.first(), first, MatchResult::Draw),
        Err(ReportError::DrawNotAllowed)
    ));
    session
        .report(session.first(), first, MatchResult::AWins)
        .unwrap();
    assert!(matches!(
        session.report(session.first(), first, MatchResult::BWins),
        Err(ReportError::AlreadyFinished)
    ));
    assert!(matches!(
        session.report(session.first(), elsewhere, MatchResult::AWins),
        Err(ReportError::UnknownMatch)
    ));
    assert!(matches!(
        {
            let mut added = Given::new().added();
            added.report(added.first(), first, MatchResult::AWins)
        },
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
            .report(
                session.first(),
                numbered(&session, number),
                MatchResult::AWins,
            )
            .unwrap();
    }

    let reopened = session
        .undo_report(session.first(), numbered(&session, 1))
        .unwrap();

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
        session.undo_report(session.first(), numbered(&session, 1)),
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
            .report(
                session.first(),
                numbered(&session, number),
                MatchResult::AWins,
            )
            .unwrap();
    }
    let semifinal = numbered(&session, 1);

    assert!(matches!(
        session.undo_report(session.first(), semifinal),
        Err(UndoReportError::DependedOnBy(numbers)) if numbers == [3]
    ));
    session
        .undo_report(session.first(), numbered(&session, 3))
        .unwrap();
    let reopened = session.undo_report(session.first(), semifinal).unwrap();

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
            .report(
                session.first(),
                numbered(&session, number),
                MatchResult::AWins,
            )
            .unwrap();
    }

    let result = session.undo_report(session.first(), numbered(&session, 1));

    assert!(
        matches!(&result, Err(UndoReportError::DependedOnBy(numbers)) if *numbers == [3, 4]),
        "{:?}",
        result.err()
    );
}

/// A knockout of four entrants decided by a score, every match won 3–1 by side A.
fn knockout_played() -> Session {
    let mut session = Given::new()
        .format(SingleElimination::ID)
        .match_format(PluginConfig::of::<Scored>())
        .started();
    for number in 1..=4 {
        session
            .report_entered(session.all(), &format!("{number} 3-1"))
            .unwrap();
    }
    session
}

/// Enters `a`–`b` as the score of the match numbered `number` of `session` in the form editing its
/// result, and hands it to the session.
fn edit_score(
    session: &mut Session,
    number: usize,
    a: u16,
    b: u16,
) -> Result<Edited, EditReportError> {
    let id = numbered(session, number);
    let mut form = session.edit_form(session.first(), id).unwrap();
    let count = |count| Some(EnteredValue::Count(count));
    form.enter_scores(&[Sides::new(count(a), count(b))])
        .unwrap();
    session.edit_report(&form)
}

/// The score shown for the match numbered `number`.
fn scores_of(session: &Session, number: usize) -> Vec<Sides<Option<EnteredValue>>> {
    let id = numbered(session, number);
    let (index, division, r#match) = session.match_of(session.first(), id).unwrap();
    session
        .match_row(session.first(), index, division, r#match)
        .scores
}

/// The form editing a result starts out at it, so that only what was wrong needs changing.
#[test]
fn the_form_editing_a_result_starts_out_at_it() {
    let session = knockout_played();
    let id = numbered(&session, 1);

    let form = session.edit_form(session.first(), id).unwrap();

    assert_eq!(
        form.view(),
        session.reported_result(session.first(), id).unwrap()
    );
}

/// A semifinal corrected to another score of the same winner keeps the final as it was played,
/// where reopening the semifinal is refused.
#[test]
fn a_result_a_later_one_depends_on_is_edited_keeping_its_winner() {
    let mut session = knockout_played();
    let count = |count| Some(EnteredValue::Count(count));

    let edited = edit_score(&mut session, 1, 3, 2).unwrap();

    assert_eq!(edited.row.result, Some(MatchResult::AWins));
    assert_eq!(edited.row.scores, [Sides::new(count(3), count(2))]);
    assert_eq!(scores_of(&session, 3), [Sides::new(count(3), count(1))]);
    assert_eq!(
        session.undo().unwrap(),
        Action::EditResult(edited.row.clone())
    );
    assert_eq!(scores_of(&session, 1), [Sides::new(count(3), count(1))]);
    session.redo().unwrap();
    assert_eq!(scores_of(&session, 1), [Sides::new(count(3), count(2))]);
}

/// Another winner of a semifinal leaves the final and the match for third place played by others,
/// so the edit names both.
#[test]
fn an_edit_later_results_no_longer_fit_names_them() {
    let mut session = knockout_played();
    let changes = session.changes();

    let result = edit_score(&mut session, 1, 1, 3);

    assert!(
        matches!(&result, Err(EditReportError::DoesNotFit(numbers)) if *numbers == [3, 4]),
        "{:?}",
        result.err()
    );
    assert_eq!(session.changes(), changes, "nothing changed");
}

/// A knockout keeps who moved on, so another winner of a semifinal is not corrected in place
/// either.
#[test]
fn a_knockout_is_corrected_in_place_keeping_its_winners_only() {
    let mut session = knockout_played();
    let id = numbered(&session, 1);
    let mut form = session.edit_form(session.first(), id).unwrap();
    let count = |count| Some(EnteredValue::Count(count));
    form.enter_scores(&[Sides::new(count(1), count(3))])
        .unwrap();

    assert!(!session.correctable(&form));
    assert!(matches!(
        session.correct_report(&form),
        Err(EditReportError::NotAccepted)
    ));
}

/// Reopening a match corrected in place takes back its result and its corrections as one action,
/// which undoing brings back as corrected.
#[test]
fn a_corrected_match_is_reopened_as_one_action() {
    let mut session = Given::new()
        .match_format(PluginConfig::of::<Scored>())
        .started();
    let first = numbered(&session, 1);
    session.report_entered(session.all(), "1 3-1").unwrap();
    let mut form = session.edit_form(session.first(), first).unwrap();
    let count = |count| Some(EnteredValue::Count(count));
    form.enter_scores(&[Sides::new(count(1), count(3))])
        .unwrap();
    session.correct_report(&form).unwrap();

    session.undo_report(session.first(), first).unwrap();

    assert_eq!(scores_of(&session, 1), [], "the match is open");
    assert!(matches!(session.undo().unwrap(), Action::Reopen(_)));
    assert_eq!(scores_of(&session, 1), [Sides::new(count(1), count(3))]);
}

/// Without later results in the way, an edit changes who won, and who plays on.
#[test]
fn an_edit_changes_who_won() {
    let mut session = Given::new()
        .format(SingleElimination::ID)
        .match_format(PluginConfig::of::<Scored>())
        .started();
    for number in 1..=2 {
        session
            .report_entered(session.all(), &format!("{number} 3-1"))
            .unwrap();
    }
    let final_before = matches(&session)[2].clone();

    let edited = edit_score(&mut session, 1, 1, 3).unwrap();

    assert_eq!(edited.row.result, Some(MatchResult::BWins));
    assert_ne!(
        matches(&session)[2].a,
        final_before.a,
        "the other side of the semifinal plays the final"
    );
}

#[test]
fn a_result_entered_as_it_was_is_no_action() {
    let mut session = knockout_played();
    let changes = session.changes();

    edit_score(&mut session, 1, 3, 1).unwrap();

    assert_eq!(session.changes(), changes);
    assert!(matches!(session.next_undo(), Some(Action::Report(_))));
}

#[test]
fn only_a_finished_match_is_edited() {
    let session = Given::new().started();

    assert!(matches!(
        session.edit_form(session.first(), numbered(&session, 1)),
        Err(EditReportError::NotReported)
    ));
}

#[test]
fn an_edit_the_match_does_not_take_is_refused() {
    let mut session = Given::new().format(SingleElimination::ID).started();
    let first = numbered(&session, 1);
    session
        .report(session.first(), first, MatchResult::AWins)
        .unwrap();
    let mut form = session.edit_form(session.first(), first).unwrap();

    form.choose(&[], Some(MatchResult::Draw));

    assert!(matches!(
        session.edit_report(&form),
        Err(EditReportError::DrawNotAllowed)
    ));
}

/// Reopening a match leaves it to be entered anew: its form starts out empty, also after its report
/// was undone, which keeps the result in the form.
#[test]
fn a_reopened_match_starts_out_empty() {
    let mut session = knockout_played();
    let final_match = numbered(&session, 3);
    let entered = |session: &Session| {
        session
            .result_form(session.first(), final_match)
            .unwrap()
            .view()
            .root
            .fields[0]
            .values
    };
    let count = |count| Some(EnteredValue::Count(count));
    session.undo_report(session.first(), final_match).unwrap();
    assert_eq!(entered(&session), Sides::new(None, None));

    session.report_entered(session.all(), "3 3-2").unwrap();
    session.undo().unwrap();
    assert_eq!(
        entered(&session),
        Sides::new(count(3), count(2)),
        "the result whose report was undone"
    );

    session.report_entered(session.all(), "3 3-2").unwrap();
    session.undo_report(session.first(), final_match).unwrap();
    assert_eq!(entered(&session), Sides::new(None, None));
}
