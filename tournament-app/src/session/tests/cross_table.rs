use tournament_engine::plugins::metrics_calculator::{MorePoints, PointsCalculator};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;

/// A round robin's cross table has a row per team, in the order of the standings, and in each the
/// one match against each other team, seen from the row's team.
#[test]
fn the_cross_table_shows_the_match_of_every_two_teams() {
    let mut session = Given::new().started();
    let added = Given::new().added();
    assert!(
        added.cross_table(added.only()).is_none(),
        "before the start"
    );
    let first = numbered(&session, 1);
    session
        .report(session.first(), first, MatchResult::AWins)
        .unwrap();

    let table = session.cross_table(session.only()).unwrap();

    let teams: Vec<_> = table
        .rows
        .iter()
        .map(|row| names(&row.entrants).join(""))
        .collect();
    let standings: Vec<_> = session
        .standings(session.only())
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
        [
            Some(Score::ONE),
            Some(Score::ZERO),
            Some(Score::ZERO),
            Some(Score::ZERO)
        ],
        "the winner's one win"
    );
}

/// A draw adds a half to the total of each of its teams, by the points of a round robin.
#[test]
fn a_draw_adds_a_half_to_the_cross_table() {
    let mut session = Given::new().started();
    session
        .report(session.first(), numbered(&session, 1), MatchResult::Draw)
        .unwrap();

    let table = session.cross_table(session.only()).unwrap();

    let halves = table
        .rows
        .iter()
        .filter(|row| row.total == Some(Score::new(0.5)))
        .count();
    assert_eq!(halves, 2);
}

/// A result is worth the points of the phase, e.g. 3 for a win and 1 for a draw, and a row's total
/// is its points as the standings show them, with those of its byes in a column of their own.
#[test]
fn the_cross_table_counts_the_points_of_the_phase() {
    let mut session = Given::new()
        .entrants(&["Alice", "Bob", "Carol", "Dave", "Erin"])
        .ranking(vec![PluginConfig::of::<MorePoints>()])
        .metrics(vec![
            PluginConfig::of::<PointsCalculator>()
                .with("points_for_win", 3.0)
                .with("points_for_draw", 1.0)
                .with("points_for_bye", 2.0),
        ])
        .started();
    for (number, result) in [(1, MatchResult::AWins), (2, MatchResult::Draw)] {
        session
            .report(session.first(), numbered(&session, number), result)
            .unwrap();
    }

    let table = session.cross_table(session.only()).unwrap();

    let mut points: Vec<_> = table
        .rows
        .iter()
        .flat_map(|row| &row.cells)
        .flatten()
        .filter_map(|played| played.points)
        .collect();
    points.sort_unstable();
    let [zero, one, three] = [0.0, 1.0, 3.0].map(Score::new);
    assert_eq!(points, [zero, one, one, three]);
    for row in &table.rows {
        let cells = row
            .cells
            .iter()
            .flatten()
            .filter_map(|played| played.points)
            .fold(Score::ZERO, |sum, points| sum + points);
        assert_eq!(row.total, Some(cells + row.byes.unwrap()));
    }
    let totals: Vec<_> = table.rows.iter().map(|row| row.total.unwrap()).collect();
    assert_eq!(totals[0], Score::new(3.0), "the winner first");
    assert_eq!(
        table.rows.iter().filter_map(|row| row.byes).max(),
        Some(Score::new(2.0)),
        "the bye of the first round"
    );
}

/// A phase counting no points has no totals, and its results are worth none.
#[test]
fn a_cross_table_without_points_has_no_totals() {
    let mut session = Given::new()
        .ranking(vec![PluginConfig::of::<MoreWins>()])
        .metrics(Vec::new())
        .started();
    session
        .report(session.first(), numbered(&session, 1), MatchResult::AWins)
        .unwrap();

    let table = session.cross_table(session.only()).unwrap();

    assert!(
        table
            .rows
            .iter()
            .all(|row| row.total.is_none() && row.byes.is_none())
    );
    assert!(
        table
            .rows
            .iter()
            .flat_map(|row| &row.cells)
            .flatten()
            .all(|played| played.points.is_none())
    );
}

/// A cell of the cross table has the result at its highest level, seen from the row's team: the
/// games won of a best of, not their scores.
#[test]
fn the_cross_table_has_the_games_won_from_the_side_of_the_row() {
    let mut session = Given::new()
        .match_format(PluginConfig::of::<BestOf>().with("games", 3))
        .started();
    session
        .report_entered(session.all(), "1 21-17 19-21 21-15")
        .unwrap();

    let table = session.cross_table(session.only()).unwrap();

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
    session.report_entered(session.all(), "1 21-17").unwrap();

    let table = session.cross_table(session.only()).unwrap();

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
        .report(unscored.first(), numbered(&unscored, 1), MatchResult::AWins)
        .unwrap();
    assert!(
        unscored
            .cross_table(unscored.only())
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

        assert!(session.cross_table(session.only()).is_none(), "{format}");
    }
    let session = Given::new().format(RandomRounds::ID).started();
    assert!(session.cross_table(session.only()).is_some());
}

/// The chart a phase shows is the one it has, without building it.
#[test]
fn the_chart_is_the_bracket_or_else_the_cross_table() {
    let added = Given::new().added();
    assert_eq!(added.chart(added.only()), None, "before the start");
    for format in [
        RoundRobin::ID,
        RandomRounds::ID,
        SingleElimination::ID,
        DoubleElimination::ID,
        MonsterDyp::ID,
    ] {
        let session = Given::new().format(format).started();

        let has = if session.bracket(session.only()).is_some() {
            Some(Chart::Bracket)
        } else {
            session
                .cross_table(session.only())
                .map(|_| Chart::CrossTable)
        };
        assert_eq!(session.chart(session.only()), has, "{format}");
    }
}
