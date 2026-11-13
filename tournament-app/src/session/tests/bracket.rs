#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;

/// Who `entry` is, as a test reads it: the names of a team, `bye`, or where a side not decided
/// yet comes from, by the number of the match or else the stage.
fn entry(entry: &BracketEntry) -> String {
    let source = |source: &BracketSource| match (source.number, source.position) {
        (Some(number), _) => format!("#{number}"),
        (None, Some(position)) => {
            format!("{} ({position})", english(source.stage.clone()).unwrap())
        }
        (None, None) => english(source.stage.clone()).unwrap(),
    };
    match entry {
        BracketEntry::Team(team) => names(team).join(" & "),
        BracketEntry::Bye => "bye".to_owned(),
        BracketEntry::WinnerOf(from) => format!("winner of {}", source(from)),
        BracketEntry::LoserOf(from) => format!("loser of {}", source(from)),
    }
}

/// `column` as its stage in English and its pairings, each as its two sides.
fn column(column: &BracketColumn) -> (String, Vec<[String; 2]>) {
    (
        english(column.stage.clone()).unwrap(),
        column
            .pairings
            .iter()
            .map(|pairing| pairing.sides.each_ref().map(entry))
            .collect(),
    )
}

/// Every column of `section`, see [`column`].
fn columns(section: &BracketSection) -> Vec<(String, Vec<[String; 2]>)> {
    section.columns.iter().map(column).collect()
}

#[test]
fn only_a_started_knockout_has_a_bracket() {
    assert_eq!(Given::new().started().bracket(), None, "a round robin");
    assert_eq!(
        Given::new().format(SingleElimination::ID).added().bracket(),
        None
    );
    assert!(
        Given::new()
            .format(SingleElimination::ID)
            .started()
            .bracket()
            .is_some()
    );
}

/// The final and the match for third place, below it, are in the bracket before they are
/// scheduled, waiting for the semifinals, and take the entrants as those are decided.
#[test]
fn the_bracket_of_a_knockout_shows_who_meets_whom() {
    let mut session = Given::new().format(SingleElimination::ID).started();
    let first = matches(&session)[0].clone();

    let bracket = session.bracket().unwrap();

    let parts = bracket.sections.iter().map(|section| section.part);
    assert_eq!(parts.collect::<Vec<_>>(), [BracketPart::Knockout]);
    let [semifinals, the_final] = &columns(&bracket.sections[0])[..] else {
        panic!("expected the semifinals and the final");
    };
    assert_eq!(semifinals.0, "Semifinals");
    assert_eq!(
        semifinals.1[0],
        [names(&first.a).join(""), names(&first.b).join("")]
    );
    assert_eq!(
        *the_final,
        (
            "Final".to_owned(),
            vec![["winner of #1".to_owned(), "winner of #2".to_owned()]]
        )
    );
    let knockout = &bracket.sections[0].columns;
    assert_eq!(knockout[0].below, None);
    assert_eq!(
        knockout[1].below.as_deref().map(column),
        Some((
            "Third place".to_owned(),
            vec![["loser of #2".to_owned(), "loser of #1".to_owned()]]
        ))
    );
    let lines = |column: &BracketColumn| -> Vec<_> {
        column
            .pairings
            .iter()
            .map(|pairing| (pairing.line_in, pairing.line_out))
            .collect()
    };
    assert_eq!(
        lines(&knockout[0]),
        [
            (false, Some(BracketLine::Down)),
            (false, Some(BracketLine::Up))
        ]
    );
    assert_eq!(lines(&knockout[1]), [(true, None)]);

    session.report(first.id, MatchResult::AWins).unwrap();

    let bracket = session.bracket().unwrap();
    let the_final = &bracket.sections[0].columns[1].pairings[0];
    assert_eq!(entry(&the_final.sides[0]), names(&first.a).join(""));
    assert_eq!(
        the_final.r#match, None,
        "it still waits for the other semifinal"
    );
    let first_row = bracket.sections[0].columns[0].pairings[0].r#match.as_ref();
    assert_eq!(
        first_row.map(|row| row.result),
        Some(Some(MatchResult::AWins))
    );
}

/// A knockout's matches have the numbers of their places in the bracket, whichever is scheduled
/// first, and a result is entered by them.
#[test]
fn a_knockout_numbers_its_matches_by_the_bracket() {
    let mut session = Given::new()
        .entrants(&["Alice", "Bob", "Carol", "Dave", "Erin", "Frank"])
        .format(SingleElimination::ID)
        .started();

    session.report_entered("2 1").unwrap();

    let second_round = session.round(round(2)).unwrap();
    let scheduled: Vec<_> = second_round.matches.iter().map(|row| row.number).collect();
    assert_eq!(
        scheduled,
        [4],
        "the second semifinal, though scheduled first"
    );
    session.report_entered("4 1").unwrap();
    assert_eq!(
        session.round(round(2)).unwrap().matches[0].result,
        Some(MatchResult::AWins)
    );
}

/// Of six entrants, the best two have byes in the quarterfinals, so neither semifinal is scheduled
/// at the start; the final names them by the numbers their matches will have, which they show.
#[test]
fn a_side_waiting_for_a_pairing_not_scheduled_yet_names_the_number_of_its_match() {
    let session = Given::new()
        .entrants(&["Alice", "Bob", "Carol", "Dave", "Erin", "Frank"])
        .format(SingleElimination::ID)
        .started();

    let bracket = session.bracket().unwrap();

    let semifinals = &bracket.sections[0].columns[1].pairings;
    assert_eq!(
        semifinals
            .iter()
            .map(|pairing| (pairing.number, pairing.r#match.is_some()))
            .collect::<Vec<_>>(),
        [(Some(3), false), (Some(4), false)]
    );

    let knockout = columns(&bracket.sections[0]);
    assert_eq!(
        knockout[1].1[0][1], "winner of #1",
        "a quarterfinal is scheduled"
    );
    assert_eq!(
        knockout[2],
        (
            "Final".to_owned(),
            vec![["winner of #3".to_owned(), "winner of #4".to_owned()]]
        )
    );
}

/// The losers of the winner bracket drop into the loser bracket, and the reset of the grand final
/// is only played if the champion of the loser bracket wins it.
#[test]
fn the_bracket_of_a_double_elimination_shows_where_the_losers_drop() {
    let session = Given::new().format(DoubleElimination::ID).started();

    let bracket = session.bracket().unwrap();

    let parts = bracket.sections.iter().map(|section| section.part);
    assert_eq!(
        parts.collect::<Vec<_>>(),
        [
            BracketPart::Winners,
            BracketPart::Losers,
            BracketPart::GrandFinal
        ]
    );
    let stages = |section: &BracketSection| -> Vec<String> {
        columns(section)
            .into_iter()
            .map(|(stage, _)| stage)
            .collect()
    };
    assert_eq!(
        stages(&bracket.sections[1]),
        ["Losers round 1", "Losers final"]
    );
    assert_eq!(
        columns(&bracket.sections[1])[1].1,
        [["winner of #4".to_owned(), "loser of #3".to_owned()]]
    );
    let losers = &bracket.sections[1].columns;
    assert_eq!(
        losers[0].pairings[0].line_out,
        Some(BracketLine::Straight),
        "the survivor meets one dropping down"
    );
    let grand_final = &bracket.sections[2].columns;
    assert_eq!(
        stages(&bracket.sections[2]),
        ["Grand final", "Grand final reset"]
    );
    assert!(!grand_final[0].pairings[0].only_if_needed);
    assert!(grand_final[1].pairings[0].only_if_needed);
}

/// Of five entrants, three have byes in the first round, whose pairings are left out: the first
/// column is drawn in the rows of the second, its one match level with the pairing it leads to,
/// which names who moved on by a bye.
#[test]
fn a_pairing_decided_by_a_bye_is_left_out_of_its_column() {
    let session = Given::new()
        .entrants(&["Alice", "Bob", "Carol", "Dave", "Erin"])
        .format(DoubleElimination::ID)
        .started();

    let bracket = session.bracket().unwrap();

    let winners = &bracket.sections[0].columns;
    assert_eq!(winners[0].rows, 2, "the rows of the second round");
    let [first] = &winners[0].pairings[..] else {
        panic!("expected the one match of the first round");
    };
    assert_eq!(
        (first.number, first.row, first.line_out),
        (Some(1), 0, Some(BracketLine::Straight))
    );
    let second: Vec<_> = winners[1]
        .pairings
        .iter()
        .map(|pairing| (pairing.after_bye, pairing.line_in))
        .collect();
    assert_eq!(second, [([true, false], true), ([true, true], false)]);
    assert_eq!(
        winners[1].rows,
        winners[1].pairings.len(),
        "not folded, as a line leads into it"
    );
}

/// Of twenty entrants, twelve have byes in the round of 32, which is drawn in the eight rows of
/// the round of 16, half as high as with a row for each of its pairings.
#[test]
fn a_knockout_with_many_byes_is_half_as_high() {
    let names: Vec<&'static str> = (1..=20)
        .map(|number| &*format!("Entrant {number}").leak())
        .collect();
    let session = Given::new()
        .entrants(&names)
        .format(SingleElimination::ID)
        .started();

    let bracket = session.bracket().unwrap();

    let first = &bracket.sections[0].columns[0];
    assert_eq!((first.rows, first.pairings.len()), (8, 4));
    assert!(
        first
            .pairings
            .iter()
            .all(|pairing| pairing.line_out == Some(BracketLine::Straight))
    );
}

/// Of five entrants, two byes meet in the first round of the loser bracket, which is left out
/// whole, and a side waiting for the winner of a pairing against a bye waits for whoever plays the
/// bye.
#[test]
fn a_bye_is_passed_through_to_whoever_plays_it() {
    let session = Given::new()
        .entrants(&["Alice", "Bob", "Carol", "Dave", "Erin"])
        .format(DoubleElimination::ID)
        .started();

    let bracket = session.bracket().unwrap();

    let losers = &bracket.sections[1].columns;
    let stages = losers
        .iter()
        .map(|column| english(column.stage.clone()).unwrap());
    assert_eq!(
        stages.collect::<Vec<_>>(),
        ["Losers round 2", "Losers round 3", "Losers final"]
    );
    assert_eq!(
        column(&losers[0]).1,
        [["loser of #1".to_owned(), "loser of #3".to_owned()]],
        "the losers of the winner bracket, rather than the winners of the byes they get first"
    );
    assert_eq!(losers[0].pairings[0].after_bye, [true, false]);
    assert_eq!(
        column(&losers[1]).1,
        [["winner of #4".to_owned(), "loser of #2".to_owned()]],
        "whoever gets the bye of the pairing of a bye and the loser of #2"
    );
    assert_eq!(losers[1].pairings[0].after_bye, [false, true]);
}

/// Reports side A as the winner of every match playable, as long as there is one, except
/// `exception`, a stage in English whose match side B wins.
fn play_out(session: &mut Session, exception: &str) {
    while let Some(row) = session.playable().into_iter().next() {
        let result = if english(row.stage.clone()).as_deref() == Some(exception) {
            MatchResult::BWins
        } else {
            MatchResult::AWins
        };
        session.report(row.id, result).unwrap();
    }
}

/// The reset of the grand final is "if needed" until it is scheduled, or until the champion of the
/// winner bracket won the grand final, which leaves it unplayed.
#[test]
fn the_reset_of_the_grand_final_is_only_if_needed_until_that_is_decided() {
    let reset = |session: &Session| {
        let bracket = session.bracket().unwrap();
        bracket.sections[2].columns[1].pairings[0].clone()
    };
    let mut session = Given::new().format(DoubleElimination::ID).started();
    assert!(reset(&session).only_if_needed);

    play_out(&mut session, "none");

    let not_needed = reset(&session);
    assert!(!not_needed.only_if_needed);
    assert_eq!(not_needed.unplayed, Some(Unplayed::NotNeeded));
    let grand_final = &session.bracket().unwrap().sections[2].columns[0].pairings[0];
    assert_eq!(grand_final.line_out, None);

    let mut session = Given::new().format(DoubleElimination::ID).started();
    play_out(&mut session, "Grand final");
    let played = reset(&session);
    assert!(!played.only_if_needed && played.unplayed.is_none());
    assert!(played.r#match.is_some());
}
