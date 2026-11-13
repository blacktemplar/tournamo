use plugin_factory::{config::ConfigError, draft::Draft};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::{
    entities::{Phase, PhaseBuilder, details::EnteredValue},
    plugins::{
        match_format::scored::SCORE,
        metrics_calculator::test_utils::{match_id, phase_with},
    },
};

/// A game `a` to `b`, decided by its score.
fn game(a: u16, b: u16) -> Part {
    let score = Sides::new(Some(EnteredValue::Count(a)), Some(EnteredValue::Count(b)));
    Part::new()
        .with_decision(result_of(Sides::new(a, b)))
        .with_value(SCORE, score)
}

/// A match decided by `result`, with `games` entered.
fn decided(result: MatchResult, games: Vec<Part>) -> Part {
    games
        .into_iter()
        .fold(Part::new().with_decision(result), Part::with_part)
}

fn phase(format: BestOf) -> Phase {
    phase_with(
        PhaseBuilder::new().match_format(format).add_players(2),
        vec![(1, vec![0], vec![1])],
    )
}

#[test]
fn a_match_tallies_the_games_won_and_the_points_of_all_games() {
    let mut phase = phase(BestOf::new(3));
    let id = match_id(&phase, 1, &[0], &[1]);
    let games = vec![game(21, 17), game(19, 21), game(21, 15)];

    phase.report_match(id, decided(MatchResult::AWins, games));

    let r#match = &phase.data().matches()[&id];
    assert_eq!(
        r#match.tally::<GamesWon>(),
        Some(&GamesWon(Sides::new(2, 1)))
    );
    assert_eq!(
        r#match.tally::<PointsScored>(),
        Some(&PointsScored(Sides::new(61, 53)))
    );
}

#[test]
fn maximum_scores_sum_in_the_wider_tally() {
    let mut phase = phase(BestOf::new(3));
    let id = match_id(&phase, 1, &[0], &[1]);
    phase.report_match(
        id,
        decided(
            MatchResult::AWins,
            vec![game(u16::MAX, 1), game(u16::MAX, 2)],
        ),
    );
    assert_eq!(
        phase.data().matches()[&id].tally::<PointsScored>(),
        Some(&PointsScored(Sides::new(2 * u32::from(u16::MAX), 3)))
    );
}

#[test]
fn a_points_tally_saturates_at_the_aggregate_limit() {
    // Just enough maximum scores to cross u32::MAX: far more games than a best of is
    // configured with, as one loaded from a file may have.
    let games = 65_538;
    let mut phase = phase(BestOf::new(games));
    let id = match_id(&phase, 1, &[0], &[1]);
    phase.report_match(
        id,
        decided(MatchResult::AWins, vec![game(u16::MAX, 1); games as usize]),
    );
    assert_eq!(
        phase.data().matches()[&id].tally::<PointsScored>(),
        Some(&PointsScored(Sides::new(u32::MAX, games)))
    );
}

#[test]
fn a_match_is_suggested_won_by_the_side_winning_most_games() {
    let format = BestOf::new(3);
    let level = format.match_level();
    let suggest = |games: Vec<Part>| {
        let part = games.into_iter().fold(Part::new(), Part::with_part);
        format.suggest(&level, &part)
    };

    assert_eq!(
        suggest(vec![game(21, 3), game(21, 3)]),
        Some(MatchResult::AWins)
    );
    assert_eq!(
        suggest(vec![game(3, 21), game(21, 3), game(3, 21)]),
        Some(MatchResult::BWins)
    );
    assert_eq!(
        suggest(vec![game(21, 3), game(3, 21)]),
        None,
        "a third game decides"
    );
    assert_eq!(suggest(Vec::new()), None);
}

#[test]
fn draws_count_as_decided_games() {
    let suggest = |format: BestOf, part: &Part| format.suggest(&format.match_level(), part);
    let drawn = Part::new().with_part(game(3, 3));
    assert_eq!(suggest(BestOf::new(1), &drawn), Some(MatchResult::Draw));

    let split = Part::new()
        .with_part(game(21, 3))
        .with_part(game(3, 21))
        .with_part(game(3, 3));
    assert_eq!(suggest(BestOf::new(3), &split), Some(MatchResult::Draw));
}

#[test]
fn a_lead_must_exceed_the_remaining_games_to_finish_early() {
    let format = BestOf::new(5);
    let level = format.match_level();
    for (win, loss, outcome) in [
        (game(21, 3), game(3, 21), MatchResult::AWins),
        (game(3, 21), game(21, 3), MatchResult::BWins),
    ] {
        let first_three = Part::new()
            .with_part(win.clone())
            .with_part(game(3, 3))
            .with_part(game(3, 3));
        assert_eq!(format.suggest(&level, &first_three), None);
        assert_eq!(
            format.suggest(&level, &first_three.clone().with_part(win.clone())),
            Some(outcome)
        );
        let tied = first_three.with_part(loss);
        assert_eq!(format.suggest(&level, &tied), None);
        assert_eq!(
            format.suggest(&level, &tied.with_part(game(3, 3))),
            Some(MatchResult::Draw)
        );
        let two_wins = Part::new()
            .with_part(win.clone())
            .with_part(win)
            .with_part(game(3, 3));
        assert_eq!(format.suggest(&level, &two_wins), None);
        assert_eq!(
            format.suggest(&level, &two_wins.with_part(Part::new())),
            None
        );
    }
}

#[test]
fn an_even_number_of_games_can_end_in_a_draw() {
    let format = BestOf::new(2);
    let level = format.match_level();
    let part = Part::new().with_part(game(21, 3)).with_part(game(3, 21));

    assert_eq!(format.suggest(&level, &part), Some(MatchResult::Draw));
}

#[test]
fn a_match_takes_at_least_the_games_a_side_needs_and_at_most_all() {
    let phase = phase(BestOf::new(5));
    let r#match = phase.data().matches().values().next().unwrap();

    let template = BestOf::new(5).template(phase.data(), r#match);

    assert!(matches!(
        template.root.parts,
        Parts::Same { min: 3, max: 5, .. }
    ));
}

#[test]
fn a_best_of_takes_at_most_99_games() {
    let games = |games: u32| Draft::<BestOf>::new(|_| {}).with("games", games);

    assert!(games(99).is_ok());
    assert!(matches!(games(100), Err(ConfigError::OutOfRange { .. })));
}

#[test]
fn a_game_is_suggested_won_by_its_score() {
    let suggested = BestOf::new(3).suggest(&BestOf::new(3).game(), &game(21, 23));

    assert_eq!(suggested, Some(MatchResult::BWins));
}

/// The tags are part of every log and snapshot written, see `transition_log_serialization.rs`.
#[test]
fn the_rules_and_their_tally_are_serialized_under_their_tags() {
    let format: Box<dyn MatchFormat> = Box::new(BestOf::new(3));
    let mut tallies = TypeMap::new();
    tallies.insert(GamesWon::default());

    assert_eq!(
        serde_json::to_value(&format).unwrap()["type"],
        "tournament_engine::best_of"
    );
    assert_eq!(
        serde_json::to_value(&tallies).unwrap()[0]["type"],
        "tournament_engine::games_won"
    );
}
