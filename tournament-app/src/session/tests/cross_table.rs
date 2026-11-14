#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;

/// A round robin's cross table has a row per team, in the order of the standings, and in each the
/// one match against each other team, seen from the row's team.
#[test]
fn the_cross_table_shows_the_match_of_every_two_teams() {
    let mut session = Given::new().started();
    assert!(
        Given::new().added().cross_table().is_none(),
        "before the start"
    );
    let first = numbered(&session, 1);
    session.report(first, MatchResult::AWins).unwrap();

    let table = session.cross_table().unwrap();

    let teams: Vec<_> = table
        .rows
        .iter()
        .map(|row| names(&row.entrants).join(""))
        .collect();
    let standings: Vec<_> = session
        .standings()
        .unwrap()
        .rows
        .iter()
        .map(|row| names(&row.entrants).join(""))
        .collect();
    assert_eq!(teams, standings);
    for (i, row) in table.rows.iter().enumerate() {
        assert_eq!(row.cells.len(), 4);
        assert!(row.cells[i].is_empty(), "no match against itself");
        for (j, cell) in row.cells.iter().enumerate().filter(|(j, _)| *j != i) {
            let [played] = cell.as_slice() else {
                panic!("one match of {i} against {j}: {cell:?}");
            };
            let [mirrored] = table.rows[j].cells[i].as_slice() else {
                panic!("one match of {j} against {i}");
            };
            assert_eq!(played.row, mirrored.row);
            let expected = match played.outcome {
                Some(Outcome::Win) => Some(Outcome::Loss),
                Some(Outcome::Loss) => Some(Outcome::Win),
                outcome => outcome,
            };
            assert_eq!(mirrored.outcome, expected);
        }
    }
    let outcomes: Vec<_> = table
        .rows
        .iter()
        .flat_map(|row| &row.cells)
        .flatten()
        .filter_map(|played| played.outcome.map(|outcome| (played.row.id, outcome)))
        .collect();
    assert_eq!(outcomes.len(), 2, "the match reported, from either side");
    assert!(outcomes.iter().all(|(id, _)| *id == first));
    assert!(
        table.rows[0]
            .cells
            .iter()
            .flatten()
            .any(|played| played.outcome == Some(Outcome::Win)),
        "the winner first: {table:?}"
    );
    let totals: Vec<_> = table.rows.iter().map(|row| row.total).collect();
    assert_eq!(
        totals,
        [Score::ONE, Score::ZERO, Score::ZERO, Score::ZERO],
        "the winner's one win"
    );
}

/// A draw adds a half to the total of each of its teams.
#[test]
fn a_draw_adds_a_half_to_the_cross_table() {
    let mut session = Given::new().started();
    session
        .report(numbered(&session, 1), MatchResult::Draw)
        .unwrap();

    let table = session.cross_table().unwrap();

    let halves = table
        .rows
        .iter()
        .filter(|row| row.total == Score::new(0.5))
        .count();
    assert_eq!(halves, 2);
}

/// A cell of the cross table has the result at its highest level, seen from the row's team: the
/// games won of a best of, not their scores.
#[test]
fn the_cross_table_has_the_games_won_from_the_side_of_the_row() {
    let mut session = Given::new()
        .match_format(PluginConfig::of::<BestOf>().with("games", 3))
        .started();
    session.report_entered("1 21-17 19-21 21-15").unwrap();

    let table = session.cross_table().unwrap();

    let mut played: Vec<_> = table
        .rows
        .iter()
        .flat_map(|row| &row.cells)
        .flatten()
        .filter_map(|played| Some((played.outcome?, played.score?)))
        .collect();
    played.sort_by_key(|(outcome, _)| *outcome != Outcome::Win);
    assert_eq!(
        played,
        [
            (Outcome::Win, scores([(2, 1)])[0]),
            (Outcome::Loss, scores([(1, 2)])[0])
        ]
    );
}

/// A match scored as a whole has its score in the cross table, seen from the row's team.
#[test]
fn the_cross_table_has_the_score_of_a_match_from_the_side_of_the_row() {
    let mut session = Given::new()
        .match_format(PluginConfig::of::<Scored>())
        .started();
    session.report_entered("1 21-17").unwrap();

    let table = session.cross_table().unwrap();

    let mut played: Vec<_> = table
        .rows
        .iter()
        .flat_map(|row| &row.cells)
        .flatten()
        .filter_map(|played| Some((played.outcome?, played.score?)))
        .collect();
    played.sort_by_key(|(outcome, _)| *outcome != Outcome::Win);
    assert_eq!(
        played,
        [
            (Outcome::Win, scores([(21, 17)])[0]),
            (Outcome::Loss, scores([(17, 21)])[0])
        ]
    );
    let mut unscored = Given::new().started();
    unscored
        .report(numbered(&unscored, 1), MatchResult::AWins)
        .unwrap();
    assert!(
        unscored
            .cross_table()
            .unwrap()
            .rows
            .iter()
            .flat_map(|row| &row.cells)
            .flatten()
            .all(|played| played.score.is_none()),
        "nothing but who won"
    );
}

/// A knockout has its bracket instead, and the teams of a Monster DYP change every round.
#[test]
fn a_cross_table_is_only_of_teams_that_stay_the_same_without_a_bracket() {
    for format in [SingleElimination::ID, DoubleElimination::ID, MonsterDyp::ID] {
        let session = Given::new().format(format).started();

        assert!(session.cross_table().is_none(), "{format}");
    }
    let session = Given::new().format(RandomRounds::ID).started();
    assert!(session.cross_table().is_some());
}

/// The chart a phase shows is the one it has, without building it.
#[test]
fn the_chart_is_the_bracket_or_else_the_cross_table() {
    assert_eq!(Given::new().added().chart(), None, "before the start");
    for format in [
        RoundRobin::ID,
        RandomRounds::ID,
        SingleElimination::ID,
        DoubleElimination::ID,
        MonsterDyp::ID,
    ] {
        let session = Given::new().format(format).started();

        let has = if session.bracket().is_some() {
            Some(Chart::Bracket)
        } else {
            session.cross_table().map(|_| Chart::CrossTable)
        };
        assert_eq!(session.chart(), has, "{format}");
    }
}
