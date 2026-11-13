#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;

#[test]
fn a_result_is_entered_as_the_number_of_its_match_and_1_x_or_2() {
    let mut session = Given::new().started();

    session.report_entered("1 2").unwrap();
    let reported = session.report_entered("  2  x ").unwrap();
    session.report_entered("3-1").unwrap();
    session.report_entered("4X").unwrap();

    let results: Vec<_> = matches(&session).iter().map(|row| row.result).collect();
    assert_eq!(
        results[..5],
        [
            Some(MatchResult::BWins),
            Some(MatchResult::Draw),
            Some(MatchResult::AWins),
            Some(MatchResult::Draw),
            None
        ]
    );
    assert_eq!(reported.row.number, 2);
}

#[test]
fn a_result_entered_for_a_match_that_has_one_names_the_match() {
    let mut session = Given::new().started();
    session.report_entered("2 1").unwrap();

    assert!(matches!(
        session.report_entered("2 2"),
        Err(ReportEnteredError::AlreadyFinished(2))
    ));
}

#[test]
fn a_result_entered_is_refused_unless_it_reads_as_one_of_a_match() {
    let mut session = Given::new().format(SingleElimination::ID).started();

    for entry in ["", " "] {
        assert!(
            matches!(
                session.report_entered(entry),
                Err(ReportEnteredError::Unreadable)
            ),
            "{entry:?}"
        );
    }
    // "11" could be match 1 won by side A, or match 11, which is read
    let entered = session.read_entered("1");
    assert!(matches!(
        entered.report,
        Err(ReportEnteredError::MissingResult(1))
    ));
    assert_eq!(numbers(&entered.found), [1], "the match is shown");
    // read as part of a name, which nobody has
    for entry in ["1 3", "one 1", "1 1 1", "-1 1", "1x1", "1 x 1", "1:1"] {
        assert!(
            matches!(
                session.report_entered(entry),
                Err(ReportEnteredError::NoNameFound(name)) if name == entry
            ),
            "{entry:?}"
        );
    }
    for (entry, number) in [("0 1", 0), ("9 1", 9), ("11", 11)] {
        assert!(
            matches!(
                session.report_entered(entry),
                Err(ReportEnteredError::NoMatch(n)) if n == number
            ),
            "{entry:?}"
        );
    }
    assert!(matches!(
        session.report_entered("1 X"),
        Err(ReportEnteredError::Report(ReportError::DrawNotAllowed))
    ));
    assert!(matches(&session).iter().all(|row| row.result.is_none()));
}

#[test]
fn scores_are_entered_after_the_number_of_a_match_and_say_its_result() {
    let mut session = Given::new()
        .match_format(PluginConfig::of::<BestOf>().with("games", 3))
        .started();

    let entered = session
        .read_entered(" 1  21-17 19:21 21–15 ")
        .report
        .unwrap();
    assert_eq!(entered.result, Some(MatchResult::AWins));
    assert_eq!(entered.scores, scores([(21, 17), (19, 21), (21, 15)]));
    let reported = session.report_entered("1 21-17 19:21 21–15").unwrap();
    assert_eq!(reported.row.scores, entered.scores);
    assert_eq!(reported.row.result, Some(MatchResult::AWins));
    let reported = session.report_entered("2 17-21 21-17 2").unwrap();
    assert_eq!(
        reported.row.result,
        Some(MatchResult::BWins),
        "the result entered decides where the scores do not"
    );
    assert_eq!(reported.row.scores, scores([(17, 21), (21, 17)]));
}

#[test]
fn scores_entered_are_refused_unless_they_fit_the_result() {
    let mut session = Given::new()
        .match_format(PluginConfig::of::<BestOf>().with("games", 3))
        .started();

    assert!(matches!(
        session.report_entered("1 21-17"),
        Err(ReportEnteredError::Undecided(1))
    ));
    assert!(matches!(
        session.report_entered("1 21-17 21-19 2"),
        Err(ReportEnteredError::AgainstScores(1))
    ));
    assert!(matches!(
        session.report_entered("1 1-0 1-0 1-0 1-0"),
        Err(ReportEnteredError::ScoresNotTaken(1))
    ));
    assert!(matches(&session).iter().all(|row| row.result.is_none()));

    let mut session = Given::new()
        .match_format(PluginConfig::of::<Scored>())
        .started();
    assert!(matches!(
        session.report_entered("1 21.5-17"),
        Err(ReportEnteredError::ScoresNotTaken(1))
    ));
    assert_eq!(
        session.report_entered("1 21,0-17").unwrap().row.scores,
        scores([(21, 17)]),
        "a whole number in a score of counts"
    );

    let mut session = Given::new().started();
    assert!(matches!(
        session.report_entered("1 3-1"),
        Err(ReportEnteredError::ScoresNotTaken(1))
    ));
    assert!(
        session.report_entered("1-1").is_ok(),
        "a win of side A, as without scores"
    );
}

#[test]
fn scores_entered_after_a_name_are_the_named_sides() {
    let mut session = Given::new()
        .entrants(&["Alice", "Bob"])
        .match_format(PluginConfig::of::<Scored>())
        .started();
    let the_match = &matches(&session)[0];
    let alice = win_of(the_match, "Alice");
    let alices_side = |a, b| {
        if alice == MatchResult::AWins {
            scores([(a, b)])
        } else {
            scores([(b, a)])
        }
    };

    assert!(matches!(
        session.read_entered("ali 17-21").report,
        Err(ReportEnteredError::AgainstScores(1))
    ));
    let won = session.read_entered("ali 1 21-17").report.unwrap();
    assert_eq!(won.result, Some(alice));
    assert_eq!(won.scores, alices_side(21, 17));
    let reported = session.report_entered("bob 21-19").unwrap();
    assert_eq!(reported.row.scores, alices_side(19, 21));
    assert_ne!(reported.row.result, Some(alice));
}

#[test]
fn a_draw_entered_with_scores_needs_them_to_say_it() {
    let mut session = Given::new()
        .entrants(&["Alice", "Bob"])
        .match_format(PluginConfig::of::<Scored>())
        .started();

    assert!(matches!(
        session.read_entered("ali x 2-1").report,
        Err(ReportEnteredError::AgainstScores(1))
    ));
    let drawn = session.report_entered("Alice X 2:2").unwrap();
    assert_eq!(drawn.row.result, Some(MatchResult::Draw));
    assert_eq!(drawn.row.scores, scores([(2, 2)]));
}

#[test]
fn a_drawn_score_is_refused_where_there_are_no_draws() {
    let mut session = Given::new()
        .format(SingleElimination::ID)
        .match_format(PluginConfig::of::<Scored>())
        .started();

    assert!(matches!(
        session.report_entered("1 2-2"),
        Err(ReportEnteredError::Report(ReportError::DrawNotAllowed))
    ));
}

/// The numbers of `rows`, in order.
fn numbers(rows: &[MatchRow]) -> Vec<usize> {
    rows.iter().map(|row| row.number).collect()
}

/// The open matches of `name`, by number.
fn open_matches_of(session: &Session, name: &str) -> Vec<MatchRow> {
    let mut rows: Vec<_> = matches(session)
        .into_iter()
        .filter(|row| {
            row.result.is_none()
                && names(&row.a)
                    .iter()
                    .chain(&names(&row.b))
                    .any(|n| *n == name)
        })
        .collect();
    rows.sort_by_key(|row| row.number);
    rows
}

/// The win of the side of `name` in `row`.
fn win_of(row: &MatchRow, name: &str) -> MatchResult {
    if names(&row.a) == [name] {
        MatchResult::AWins
    } else {
        MatchResult::BWins
    }
}

/// Part of the winner's name fits every open match of the entrant, all of which are shown, and
/// reports nothing while they are several, even if just one is playable now: Alice plays a match
/// in every round of a round robin, so the number of the one meant is needed.
#[test]
fn a_win_is_entered_as_part_of_the_winners_name() {
    let mut session = Given::new().started();
    let alices = open_matches_of(&session, "Alice");

    let entered = session.read_entered(" ALI ");

    assert_eq!(numbers(&entered.found), numbers(&alices));
    assert!(matches!(
        entered.report,
        Err(ReportEnteredError::SeveralMatches { name, matches: 3, number })
            if name == "ALI" && number == alices[0].number
    ));
    assert!(session.report_entered("ali").is_err());
    let meant = &alices[1];
    let entry = format!(" ali {} ", meant.number);
    assert_eq!(numbers(&session.read_entered(&entry).found), [meant.number]);
    let reported = session.report_entered(&entry).unwrap();
    assert_eq!(reported.row.id, meant.id);
    assert_eq!(reported.row.result, Some(win_of(meant, "Alice")));
}

/// A draw is entered as part of a name and an `X`, before or after the number of the match.
#[test]
fn a_draw_is_entered_as_part_of_a_name_and_x() {
    let mut session = Given::new().started();
    let alices = open_matches_of(&session, "Alice");

    let first = session
        .report_entered(&format!("ali {} X", alices[0].number))
        .unwrap();
    let second = session
        .report_entered(&format!("Ali x {}", alices[1].number))
        .unwrap();
    let third = session.report_entered("ali x").unwrap();

    assert_eq!(
        [first.row.result, second.row.result, third.row.result],
        [Some(MatchResult::Draw); 3]
    );
    assert_eq!(
        numbers(&[first.row, second.row, third.row]),
        numbers(&alices),
        "the last one without a number, being the only one left"
    );
}

/// A number or an `X` at the end is part of the name as long as a name contains it.
#[test]
fn a_name_with_a_number_or_an_x_is_read_as_the_name_first() {
    let mut session = Given::new().entrants(&["Malcolm X", "Team 7"]).started();
    let the_match = &matches(&session)[0];

    let won = session.read_entered("malcolm x").report.unwrap();
    assert_eq!(won.result, Some(win_of(the_match, "Malcolm X")));
    let won = session.read_entered("team 7").report.unwrap();
    assert_eq!(won.result, Some(win_of(the_match, "Team 7")));
    let drawn = session.read_entered("Malcolm X X").report.unwrap();
    assert_eq!(drawn.result, Some(MatchResult::Draw));
    assert!(
        session.read_entered("team 7 1").report.is_ok(),
        "Team 7 in match 1"
    );
    assert!(matches!(
        session.read_entered("team 7 9").report,
        Err(ReportEnteredError::NotInMatch { name, number: 9 }) if name == "team 7"
    ));
    session.report_entered("team 7 1 x").unwrap();
    assert!(
        matches!(
            session.read_entered("team").report,
            Err(ReportEnteredError::NoNameFound(_))
        ),
        "a match with a result is not open"
    );
}

/// A name found on both sides of a match names no winner, but a draw of it; a name nobody has
/// reports nothing.
#[test]
fn a_name_entered_has_to_fit_one_side() {
    let mut session = Given::new().entrants(&["Anna", "Hannah"]).started();

    assert!(matches!(
        session.report_entered("ANN"),
        Err(ReportEnteredError::BothSides { name, number: 1 }) if name == "ANN"
    ));
    assert!(matches!(
        session.report_entered("Erin"),
        Err(ReportEnteredError::NoNameFound(name)) if name == "Erin"
    ));
    assert!(matches(&session).iter().all(|row| row.result.is_none()));
    let drawn = session.report_entered("ann x").unwrap();
    assert_eq!(drawn.row.result, Some(MatchResult::Draw));
}

/// The number of a match with a result fits it, so that it is shown, but reports nothing.
#[test]
fn a_match_entered_with_a_result_is_shown_but_not_reported() {
    let mut session = Given::new().started();
    session.report_entered("1 1").unwrap();

    let entered = session.read_entered("1 2");

    assert_eq!(numbers(&entered.found), [1]);
    assert!(matches!(
        entered.report,
        Err(ReportEnteredError::AlreadyFinished(1))
    ));
}
