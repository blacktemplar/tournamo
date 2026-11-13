#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;

#[test]
fn the_matches_are_listed_by_round_with_the_names_of_their_entrants() {
    let session = Given::new().started();

    let rounds = session.rounds();

    assert_eq!(
        rounds,
        (1..=3)
            .map(|number| RoundSummary {
                round: round(number),
                matches: 2,
                open: 2,
                stage: None,
            })
            .collect::<Vec<_>>(),
        "a round robin of four plays three rounds of two matches, none of a stage"
    );
    let first = session.round(round(1)).unwrap();
    assert_eq!(
        first
            .matches
            .iter()
            .map(|row| (row.number, row.round))
            .collect::<Vec<_>>(),
        [(1, round(1)), (2, round(1))]
    );
    let mut entrants: Vec<_> = first
        .matches
        .iter()
        .flat_map(|row| names(&row.a).into_iter().chain(names(&row.b)))
        .collect();
    entrants.sort_unstable();
    assert_eq!(entrants, ["Alice", "Bob", "Carol", "Dave"]);
    assert!(first.matches.iter().all(|row| row.stage.is_none()));
    assert!(first.byes.is_empty());
    assert!(first.draws, "a round robin takes draws");
    assert_eq!(session.round(round(4)), None);
}

#[test]
fn nothing_is_played_before_the_start() {
    let session = Given::new().added();

    assert!(session.rounds().is_empty());
    assert_eq!(session.round(round(1)), None);
    assert_eq!(session.standings(), None);
}

/// A round robin of three has one entrant sit out every round.
#[test]
fn a_round_lists_its_byes() {
    let session = Given::new().entrants(&["Alice", "Bob", "Carol"]).started();

    let second = session.round(round(2)).unwrap();

    assert_eq!(second.matches.len(), 1);
    assert_eq!(second.byes.len(), 1);
    assert_eq!(second.byes[0].entrants.len(), 1);
    assert_eq!(second.byes[0].stage, None, "a round robin names no stages");
}

/// Of six entrants in a double elimination, the best two sit out the first round of the winner
/// bracket, so the two that lose in it sit out the first round of the loser bracket, which is
/// played in the round of the second round of the winner bracket.
#[test]
fn a_bye_is_named_by_its_stage_where_its_round_has_several() {
    let mut session = Given::new()
        .entrants(&["Alice", "Bob", "Carol", "Dave", "Erin", "Frank"])
        .format(DoubleElimination::ID)
        .started();
    let bye_stages = |session: &Session, number| -> Vec<Option<String>> {
        let round = session.round(round(number)).unwrap();
        round
            .byes
            .into_iter()
            .map(|bye| english(bye.stage))
            .collect()
    };
    assert_eq!(
        bye_stages(&session, 1),
        [None, None],
        "every match and bye of the first round is of the winner bracket"
    );

    for number in 1..=2 {
        session
            .report(numbered(&session, number), MatchResult::AWins)
            .unwrap();
    }

    let losers = Some("Losers round 1".to_owned());
    assert_eq!(bye_stages(&session, 2), [losers.clone(), losers]);
}

#[test]
fn a_knockout_names_the_stage_of_every_match() {
    let mut session = Given::new().format(SingleElimination::ID).started();
    let stages = |session: &Session| -> Vec<Option<String>> {
        matches(session)
            .into_iter()
            .map(|row| english(row.stage))
            .collect()
    };
    assert_eq!(
        stages(&session),
        [Some("Semifinals".to_owned()), Some("Semifinals".to_owned())]
    );
    assert_eq!(
        matches(&session)[0].stage.as_ref().unwrap().format,
        SingleElimination::ID,
        "a stage is translated by its format"
    );

    let first = session
        .report(numbered(&session, 1), MatchResult::AWins)
        .unwrap();
    let second = session
        .report(numbered(&session, 2), MatchResult::AWins)
        .unwrap();

    assert_eq!((first.new_matches, first.winners), (0, None));
    assert_eq!(
        (second.new_matches, second.winners),
        (2, None),
        "the final and the match for third place"
    );
    assert_eq!(
        stages(&session)[2..],
        [Some("Final".to_owned()), Some("Third place".to_owned())]
    );
    session
        .report(numbered(&session, 3), MatchResult::AWins)
        .unwrap();
    let last = session
        .report(numbered(&session, 4), MatchResult::BWins)
        .unwrap();
    assert_eq!(last.new_matches, 0);
    assert_eq!(last.winners, session.winners());
    assert!(last.winners.is_some_and(|winners| winners.len() == 1));
}

/// A round whose matches are all of one stage is named by it, one of several stages is not.
#[test]
fn a_round_of_one_stage_is_named_by_it() {
    let mut session = Given::new().format(SingleElimination::ID).started();
    for number in 1..=2 {
        session
            .report(numbered(&session, number), MatchResult::AWins)
            .unwrap();
    }

    let stages: Vec<_> = session
        .rounds()
        .into_iter()
        .map(|summary| english(summary.stage))
        .collect();

    assert_eq!(stages, [Some("Semifinals".to_owned()), None]);
}

/// A round a cell of the standings names, e.g. that of the first loss, is named by the stage of
/// the entrant's match in it, also in a round of several stages.
#[test]
fn the_standings_name_the_round_of_a_cell_by_its_stage() {
    let mut session = Given::new().format(SingleElimination::ID).started();
    for number in 1..=4 {
        session
            .report(numbered(&session, number), MatchResult::AWins)
            .unwrap();
    }

    let standings = session.standings().unwrap();

    let column = standings
        .columns
        .iter()
        .position(|column| column.calculator == RoundOfFirstLossCalculator::ID)
        .unwrap();
    let lost_in: Vec<_> = standings
        .rows
        .iter()
        .map(|row| match &row.cells[column] {
            Some(StandingCell::Stage(stage)) => stage.stage.to_string(),
            Some(StandingCell::Metric(MetricCell::Text(phrase))) => phrase.to_string(),
            cell => panic!("{cell:?} is no round"),
        })
        .collect();
    assert_eq!(lost_in, ["-", "Final", "Semifinals", "Semifinals"]);
}

/// The columns of the metrics the phase is ranked by come first, in the order of the criteria,
/// so that what decides the rank is next to the names; the others follow in running order.
#[test]
fn the_standings_show_the_columns_of_the_metrics() {
    let mut session = Given::new().started();
    for number in 1..=6 {
        session
            .report(numbered(&session, number), MatchResult::AWins)
            .unwrap();
    }

    let standings = session.standings().unwrap();

    assert_eq!(
        standings.columns,
        [
            Column {
                calculator: "tournament_engine::points",
                header: "points",
                label: "points",
                description: Some("scored for every win, every draw and every bye"),
                ranks: true,
            },
            Column {
                calculator: "tournament_engine::sonneborn_berger",
                header: "Sonneborn-Berger",
                label: "Sonneborn-Berger",
                description: Some(
                    "the points of the opponents that were beaten or played to a draw"
                ),
                ranks: true,
            },
            Column {
                calculator: "tournament_engine::wins_draws_losses",
                header: "W-D-L",
                label: "win-draw-loss record",
                description: Some("how many matches were won, drawn and lost"),
                ranks: false,
            },
        ]
    );
    let points: Vec<Score> = standings
        .rows
        .iter()
        .map(|row| match row.cells[0] {
            Some(StandingCell::Metric(MetricCell::Number(points))) => points,
            ref cell => panic!("{cell:?} is no number of points"),
        })
        .collect();
    assert_eq!(points.len(), 4);
    assert!(
        points.windows(2).all(|pair| pair[0] >= pair[1]),
        "{standings:?}"
    );
    assert!(points[0] > points[3], "{standings:?}");
}

/// A ranking that lost every match is told apart from one that has not played yet.
#[test]
fn the_standings_count_the_matches_played() {
    let mut session = Given::new().started();

    session
        .report(numbered(&session, 1), MatchResult::AWins)
        .unwrap();

    let mut played: Vec<_> = session
        .standings()
        .unwrap()
        .rows
        .iter()
        .map(|row| row.played)
        .collect();
    played.sort_unstable();
    assert_eq!(played, [0, 0, 1, 1]);
}

/// Once every match has a result, the first three ranks win medals; Carol and Dave, tied for
/// third, both win bronze.
#[test]
fn the_first_three_ranks_win_medals_once_every_match_has_a_result() {
    let mut session = Given::new().started();
    let medals = |session: &Session| -> Vec<_> {
        session
            .standings()
            .unwrap()
            .rows
            .iter()
            .map(|row| (names(&row.entrants).join(""), row.medal))
            .collect()
    };
    assert!(medals(&session).iter().all(|(_, medal)| medal.is_none()));

    // Alice beats everyone, Bob everyone else, Carol and Dave draw
    let strength = ["Alice", "Bob", "Carol"];
    while let Some(row) = session.playable().first().cloned() {
        let rank = |team: &[EntrantRow]| {
            strength
                .iter()
                .position(|name| names(team) == [*name])
                .unwrap_or(strength.len() - 1)
        };
        let result = match rank(&row.a).cmp(&rank(&row.b)) {
            Ordering::Less => MatchResult::AWins,
            Ordering::Greater => MatchResult::BWins,
            Ordering::Equal => MatchResult::Draw,
        };
        session.report(row.id, result).unwrap();
    }

    let mut medals = medals(&session);
    // tied, in no order of their own
    medals[2..].sort_by(|(a, _), (b, _)| a.cmp(b));
    assert_eq!(
        medals,
        [
            ("Alice".to_owned(), Some(Medal::Gold)),
            ("Bob".to_owned(), Some(Medal::Silver)),
            ("Carol".to_owned(), Some(Medal::Bronze)),
            ("Dave".to_owned(), Some(Medal::Bronze)),
        ]
    );
}

/// In a round robin of four, every match of the second round has an entrant of each match of the
/// first, so it can only be played once both of those have.
#[test]
fn a_match_can_be_played_once_its_entrants_played_the_rounds_before() {
    let mut session = Given::new().started();
    let numbers = |session: &Session| -> Vec<usize> {
        session.playable().iter().map(|row| row.number).collect()
    };
    assert_eq!(numbers(&session), [1, 2]);

    session
        .report(numbered(&session, 1), MatchResult::AWins)
        .unwrap();
    assert_eq!(numbers(&session), [2]);
    session
        .report(numbered(&session, 2), MatchResult::AWins)
        .unwrap();

    assert_eq!(numbers(&session), [3, 4]);
    assert!(Given::new().added().playable().is_empty());
}

/// The numbers of the matches a list of rounds shows.
fn listed(view: &RoundsView) -> Vec<usize> {
    view.matches
        .iter()
        .map(|listed| listed.row.number)
        .collect()
}

#[test]
fn the_rounds_show_the_first_round_with_an_open_match_unless_another_is_chosen() {
    let mut session = Given::new().started();
    session
        .report(numbered(&session, 1), MatchResult::AWins)
        .unwrap();
    session
        .report(numbered(&session, 2), MatchResult::AWins)
        .unwrap();

    let view = session.rounds_view(None, "").unwrap();
    assert_eq!(view.current, round(2));
    assert_eq!(view.shown, Shown::Round(round(2)));
    assert_eq!((view.previous, view.next), (Some(round(1)), Some(round(3))));
    assert_eq!(listed(&view), [3, 4]);
    assert!(
        !view.with_round,
        "a round shown names none of its matches by it"
    );

    let first = session
        .rounds_view(Some(Shown::Round(round(1))), "")
        .unwrap();
    assert_eq!((first.previous, first.next), (None, Some(round(2))));
    assert_eq!(listed(&first), [1, 2]);

    let gone = session
        .rounds_view(Some(Shown::Round(round(9))), "")
        .unwrap();
    assert_eq!(
        gone.shown,
        Shown::Round(round(2)),
        "a round that no longer exists falls back to the current one"
    );
}

#[test]
fn the_rounds_show_the_playable_matches_each_by_its_round() {
    let session = Given::new().started();

    let view = session.rounds_view(Some(Shown::Playable), "").unwrap();

    assert_eq!(listed(&view), [1, 2]);
    assert_eq!(view.playable, 2);
    assert_eq!((view.previous, view.next), (None, None));
    assert!(view.with_round);
}

/// After a result is reported, the operator goes on to the next match without one, wrapping around
/// to the first.
#[test]
fn each_open_match_listed_leads_on_to_the_next_open_one() {
    let mut session = Given::new()
        .entrants(&["Alice", "Bob", "Carol", "Dave", "Erin", "Frank"])
        .started();
    session
        .report(numbered(&session, 2), MatchResult::AWins)
        .unwrap();

    let view = session.rounds_view(None, "").unwrap();

    let next: Vec<_> = view
        .matches
        .iter()
        .map(|listed| (listed.row.number, listed.next_open))
        .collect();
    let id = |number| Some(numbered(&session, number));
    assert_eq!(next, [(1, id(3)), (2, None), (3, id(1))]);
}

#[test]
fn the_rounds_show_the_matches_a_typed_result_fits() {
    let mut session = Given::new().started();

    let view = session.rounds_view(None, " 3 1 ").unwrap();

    let entry = view.entry.as_ref().unwrap();
    assert_eq!(entry.text, "3 1");
    assert_eq!(entry.report.as_ref().unwrap().number, 3);
    assert_eq!(listed(&view), [3]);
    assert!(view.with_round);
    assert!(view.byes.is_empty());
    let blank = session.rounds_view(None, "  ").unwrap();
    assert!(blank.entry.is_none());
    assert_eq!(listed(&blank), [1, 2]);

    while let Some(row) = session.playable().first().cloned() {
        session.report(row.id, MatchResult::AWins).unwrap();
    }
    let finished = session.rounds_view(None, "3 1").unwrap();
    assert!(finished.finished);
    assert!(
        finished.entry.is_none(),
        "nothing is left to type once every match has a result"
    );
    assert_eq!(finished.current, round(3), "the last round");
}

#[test]
fn there_are_no_rounds_to_show_before_the_start() {
    assert!(Given::new().added().rounds_view(None, "1 1").is_none());
}

/// A round robin of four plays three rounds of two matches.
#[test]
fn the_progress_is_the_first_round_with_a_match_to_play() {
    let mut session = Given::new().added();
    assert_eq!(session.progress(), Progress::Setup { entrants: 4 });
    session.start().unwrap();
    assert_eq!(
        session.progress(),
        Progress::Playing {
            round: round(1),
            open: 2
        }
    );

    for number in 1..=2 {
        session
            .report(numbered(&session, number), MatchResult::Draw)
            .unwrap();
    }
    assert_eq!(
        session.progress(),
        Progress::Playing {
            round: round(2),
            open: 2
        }
    );
    for number in 3..=6 {
        session
            .report(numbered(&session, number), MatchResult::Draw)
            .unwrap();
    }

    let Progress::Won { winners } = session.progress() else {
        panic!("every match has a result");
    };
    // every match drawn, everyone is tied for first, listed by name
    assert_eq!(
        winners,
        ["Alice", "Bob", "Carol", "Dave"].map(|name| vec![Some(name.to_owned())])
    );
}

/// The winner is known once every match has a result, not before.
#[test]
fn the_winner_is_the_team_ranked_first_once_every_match_is_played() {
    let mut session = Given::new()
        .entrants(&["Alice", "Bob"])
        .format(SingleElimination::ID)
        .started();
    assert_eq!(session.winners(), None);
    let final_ = &matches(&session)[0];
    let winner = final_.a.clone();

    session.report(final_.id, MatchResult::AWins).unwrap();

    assert_eq!(session.winners(), Some(vec![winner.clone()]));
    assert_eq!(
        session.progress(),
        Progress::Won {
            winners: vec![winner.into_iter().map(|entrant| entrant.name).collect()]
        }
    );
    session.undo().unwrap();
    assert_eq!(session.winners(), None);
}
