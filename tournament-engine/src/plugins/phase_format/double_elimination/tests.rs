use DoubleEliminationRound::{GrandFinal, GrandFinalReset, Losers, Winners};
use itertools::Itertools;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::{
    entities::{Annotated, Match, MatchId, MatchResult, Phase, PhaseBuilder, RankingId},
    plugins::{
        metrics_calculator::{
            round_of_elimination::{
                LaterElimination, RoundOfElimination, RoundOfEliminationCalculator,
            },
            wins_draws_losses::{FewerLosses, WinsDrawsLosses, WinsDrawsLossesCalculator},
        },
        phase_format::Advance,
    },
};

#[test]
fn double_elimination_rounds_are_named_by_their_bracket() {
    let names = [
        Winners {
            round: 1,
            rounds: 3,
        },
        Winners {
            round: 3,
            rounds: 3,
        },
        Losers {
            round: 2,
            rounds: 4,
        },
        Losers {
            round: 4,
            rounds: 4,
        },
        GrandFinal,
        GrandFinalReset,
    ]
    .map(|round| round.to_string());

    assert_eq!(
        names,
        [
            "Winners round 1",
            "Winners final",
            "Losers round 2",
            "Losers final",
            "Grand final",
            "Grand final reset"
        ]
    );
}

#[test]
fn a_stage_is_keyed_by_its_bracket_and_whether_it_is_the_final() {
    let stages = [
        Winners {
            round: 1,
            rounds: 3,
        },
        Winners {
            round: 3,
            rounds: 3,
        },
        Losers {
            round: 2,
            rounds: 4,
        },
        Losers {
            round: 4,
            rounds: 4,
        },
        GrandFinal,
        GrandFinalReset,
    ]
    .map(Stage::from);

    assert_eq!(
        stages
            .each_ref()
            .map(|stage| (stage.key(), stage.numbers())),
        [
            ("winners-round", &[("round", 1)][..]),
            ("winners-final", &[]),
            ("losers-round", &[("round", 2)]),
            ("losers-final", &[]),
            ("grand-final", &[]),
            ("grand-final-reset", &[]),
        ]
    );
    assert_eq!(stages[0].to_string(), "Winners round 1");
}

/// A double elimination between `num_players` entrants with its first matches scheduled,
/// ranked by the round of elimination and then by the losses.
fn bracket(format: DoubleElimination, num_players: u32) -> Phase {
    let mut phase = PhaseBuilder::new()
        .format(format)
        .add_players(num_players)
        .add_ranking(LaterElimination)
        .add_ranking(FewerLosses)
        .add_metric(RoundOfEliminationCalculator)
        .add_metric(WinsDrawsLossesCalculator)
        .build();
    phase.create_new_matches();
    phase
}

fn seed_of(phase: &Phase, ranking_id: &RankingId) -> u32 {
    phase.data().rankings()[ranking_id]
        .initial_unique_rank()
        .get()
}

/// Every match as its round, its stage and the seeds of its two sides, in the order the
/// matches were scheduled in.
fn schedule(phase: &Phase) -> Vec<(u32, DoubleEliminationRound, (u32, u32))> {
    phase
        .data()
        .matches()
        .values()
        .map(|r#match| {
            (
                r#match.round().get(),
                *r#match.annotation::<DoubleEliminationRound>().unwrap(),
                (
                    seed_of(phase, &r#match.team_a()[0]),
                    seed_of(phase, &r#match.team_b()[0]),
                ),
            )
        })
        .collect()
}

/// Reports the open matches one at a time, in the order they were scheduled in, as won by the
/// side `winner` picks given the match and the seeds of its sides, until none is left.
fn play_out(phase: &mut Phase, winner: impl Fn(&Match, u32, u32) -> Team) {
    while let Some(r#match) = phase.data().matches().values().find(|m| !m.is_finished()) {
        let seeds = (
            seed_of(phase, &r#match.team_a()[0]),
            seed_of(phase, &r#match.team_b()[0]),
        );
        let result = match winner(r#match, seeds.0, seeds.1) {
            Team::A => MatchResult::AWins,
            Team::B => MatchResult::BWins,
        };
        phase.report_match_result(r#match.id(), result);
    }
}

fn favourite(_: &Match, seed_a: u32, seed_b: u32) -> Team {
    if seed_a < seed_b { Team::A } else { Team::B }
}

/// The favourites win everything but the grand final.
fn underdog_wins_the_grand_final(r#match: &Match, seed_a: u32, seed_b: u32) -> Team {
    match r#match.annotation::<DoubleEliminationRound>() {
        Some(GrandFinal) => Team::B,
        _ => favourite(r#match, seed_a, seed_b),
    }
}

/// The rank and the round of elimination of every entrant, ordered by seed.
fn standings(phase: &Phase) -> Vec<(u32, Option<u32>)> {
    phase
        .standings()
        .iter()
        .sorted_by_key(|standing| standing.ranking().initial_unique_rank())
        .map(|standing| {
            let eliminated = standing.ranking().metric::<RoundOfElimination>().unwrap().0;
            (standing.rank().get(), eliminated.map(NonZeroU32::get))
        })
        .collect()
}

#[test]
fn four_entrants_play_through_both_brackets_into_the_grand_final() {
    let mut phase = bracket(DoubleElimination::new(), 4);

    play_out(&mut phase, favourite);

    let winners = |round| Winners { round, rounds: 2 };
    let losers = |round| Losers { round, rounds: 2 };
    assert_eq!(
        schedule(&phase),
        [
            (1, winners(1), (1, 4)),
            (1, winners(1), (2, 3)),
            (2, winners(2), (1, 2)),
            (2, losers(1), (4, 3)),
            (3, losers(2), (3, 2)),
            (4, GrandFinal, (1, 2)),
        ]
    );
    assert_eq!(
        standings(&phase),
        [(1, None), (2, Some(4)), (3, Some(3)), (4, Some(2))]
    );
}

/// The loser bracket knocks out two entrants per round of the winner bracket, which ties the
/// two knocked out in the same round.
#[test]
fn entrants_knocked_out_in_the_same_round_share_their_rank() {
    let mut phase = bracket(DoubleElimination::new(), 8);

    play_out(&mut phase, favourite);

    assert_eq!(
        standings(&phase),
        [
            (1, None),
            (2, Some(6)),
            (3, Some(5)),
            (4, Some(4)),
            (5, Some(3)),
            (5, Some(3)),
            (7, Some(2)),
            (7, Some(2)),
        ]
    );
}

#[test]
fn without_a_reset_the_grand_final_knocks_out_its_loser() {
    let mut phase = bracket(DoubleElimination::new(), 4);

    play_out(&mut phase, underdog_wins_the_grand_final);

    assert_eq!(
        phase.data().matches().len(),
        6,
        "nothing follows the grand final"
    );
    assert_eq!(
        standings(&phase)[..2],
        [(2, Some(4)), (1, None)],
        "the champion of the winner bracket is out after its first loss"
    );
}

#[test]
fn with_a_reset_the_champion_of_the_winner_bracket_needs_to_lose_twice() {
    let mut phase = bracket(DoubleElimination::new().with_grand_final_reset(), 4);

    play_out(&mut phase, underdog_wins_the_grand_final);

    assert_eq!(
        schedule(&phase)[5..],
        [(4, GrandFinal, (1, 2)), (5, GrandFinalReset, (1, 2))]
    );
    assert_eq!(
        standings(&phase)[..2],
        [(1, None), (2, Some(5))],
        "the favourite lost the grand final but won the reset"
    );
}

#[test]
fn a_reset_is_not_played_if_the_champion_of_the_winner_bracket_wins_the_grand_final() {
    let mut phase = bracket(DoubleElimination::new().with_grand_final_reset(), 4);

    play_out(&mut phase, favourite);

    assert_eq!(phase.data().matches().len(), 6);
}

/// With a reset to come, losing the grand final only knocks out the champion of the loser
/// bracket, which is side B.
#[test]
fn the_grand_final_eliminates_according_to_the_reset() {
    for (format, eliminates) in [
        (DoubleElimination::new(), Eliminates::Loser),
        (
            DoubleElimination::new().with_grand_final_reset(),
            Eliminates::Only(Team::B),
        ),
    ] {
        let mut phase = bracket(format, 2);
        let first = *phase.data().matches().keys().next().unwrap();
        phase.report_match_result(first, MatchResult::AWins);

        let grand_final = phase.data().matches().values().last().unwrap();

        assert_eq!(grand_final.annotation(), Some(&GrandFinal));
        assert_eq!(grand_final.annotation(), Some(&eliminates));
    }
}

/// A match of the next round is scheduled as soon as the two matches it follows from are
/// decided, while the rest of the round is still being played.
#[test]
fn a_match_is_scheduled_as_soon_as_both_of_its_entrants_are_known() {
    let mut phase = bracket(DoubleElimination::new(), 8);
    let first_two: Vec<MatchId> = phase.data().matches().keys().take(2).copied().collect();

    for match_id in first_two {
        phase.report_match_result(match_id, MatchResult::AWins);
    }

    assert_eq!(
        schedule(&phase)[4..],
        [
            (
                2,
                Winners {
                    round: 2,
                    rounds: 3
                },
                (1, 4)
            ),
            (
                2,
                Losers {
                    round: 1,
                    rounds: 4
                },
                (8, 5)
            ),
        ]
    );
}

/// Where the bracket is not full, the best seeds advance without a match, and two of them that
/// meet next can play right away.
#[test]
fn the_best_seeds_get_the_byes() {
    let phase = bracket(DoubleElimination::new(), 5);

    let winners = |round| Winners { round, rounds: 3 };
    assert_eq!(
        schedule(&phase),
        [(1, winners(1), (4, 5)), (2, winners(2), (2, 3))]
    );
}

/// The bye of the winner bracket leaves nobody to drop down, so whoever drops down to meet
/// nobody has a bye in the loser bracket as well.
#[test]
fn a_bye_of_the_winner_bracket_leads_to_one_in_the_loser_bracket() {
    let mut phase = bracket(DoubleElimination::new(), 3);

    play_out(&mut phase, favourite);

    let byes = phase
        .data()
        .byes()
        .values()
        .map(|bye| {
            (
                bye.round().get(),
                *bye.annotation::<DoubleEliminationRound>().unwrap(),
                seed_of(&phase, &bye.ranking()),
            )
        })
        .collect_vec();
    assert_eq!(
        byes,
        [
            (
                1,
                Winners {
                    round: 1,
                    rounds: 2
                },
                1
            ),
            (
                2,
                Losers {
                    round: 1,
                    rounds: 2
                },
                3
            ),
        ]
    );
}

/// Whatever the results, every entrant but the champion is knocked out, after its second loss
/// unless it lost a grand final that is not reset, and plays its matches in the order of
/// their rounds.
#[test]
fn every_bracket_size_plays_out_to_one_champion() {
    // an arbitrary but fixed mix of favourites and underdogs winning
    let mixed = |r#match: &Match, seed_a: u32, seed_b: u32| {
        if (seed_a * 7 + seed_b * 13 + r#match.round().get()).is_multiple_of(3) {
            Team::B
        } else {
            Team::A
        }
    };
    for (reset, num_players) in [false, true].into_iter().cartesian_product(2..=20) {
        let format = if reset {
            DoubleElimination::new().with_grand_final_reset()
        } else {
            DoubleElimination::new()
        };
        let mut phase = bracket(format, num_players);

        play_out(&mut phase, mixed);

        let context = format!("{num_players} entrants, reset: {reset}");
        let phase = phase.data();
        let reset_played = phase
            .matches()
            .values()
            .any(|r#match| r#match.annotation() == Some(&GrandFinalReset));
        assert_eq!(
            phase.matches().len() as u32,
            2 * num_players - 2 + u32::from(reset_played),
            "{context}"
        );
        let (still_in, knocked_out): (Vec<_>, Vec<_>) = phase
            .rankings()
            .values()
            .partition(|ranking| ranking.metric::<RoundOfElimination>().unwrap().0.is_none());
        assert_eq!(still_in.len(), 1, "{context}");
        let losses: Vec<u32> = knocked_out
            .iter()
            .map(|ranking| ranking.metric::<WinsDrawsLosses>().unwrap().losses)
            .sorted()
            .collect();
        let out_after_one_loss = losses.iter().filter(|losses| **losses == 1).count();
        assert!(
            losses.iter().all(|losses| (1..=2).contains(losses))
                && out_after_one_loss <= usize::from(!reset),
            "{context}: {losses:?}"
        );
        for ranking_id in phase.rankings().keys() {
            let rounds: Vec<_> = phase
                .matches()
                .values()
                .filter(|r#match| r#match.rankings().contains(ranking_id))
                .map(Match::round)
                .collect();
            assert!(
                rounds.is_sorted() && rounds.iter().all_unique(),
                "{context}"
            );
        }
    }
}

/// The loser bracket takes in the losers of the winner bracket, and the reset follows the
/// grand final only if the champion of the loser bracket wins it.
#[test]
fn the_bracket_tells_where_the_losers_drop_and_when_the_reset_is_played() {
    let phase = bracket(DoubleElimination::new().with_grand_final_reset(), 4);
    let format: &dyn std::any::Any = phase.format();
    let pairings = format
        .downcast_ref::<DoubleElimination>()
        .and_then(DoubleElimination::bracket)
        .expect("a started double elimination has a bracket")
        .pairings();
    let stages = pairings
        .iter()
        .map(|pairing| *pairing.annotations.get::<DoubleEliminationRound>().unwrap())
        .collect_vec();
    let winners = |round| Winners { round, rounds: 2 };
    let losers = |round| Losers { round, rounds: 2 };
    assert_eq!(
        stages,
        [
            winners(1),
            winners(1),
            winners(2),
            losers(1),
            losers(2),
            GrandFinal,
            GrandFinalReset
        ]
    );
    let from = |index: usize| pairings[index].sides.map(|side| side.from);
    let id = |index: usize| pairings[index].id;
    assert_eq!(
        from(3),
        [Some(Advance::LoserOf(id(0))), Some(Advance::LoserOf(id(1)))]
    );
    assert_eq!(
        from(4),
        [
            Some(Advance::WinnerOf(id(3))),
            Some(Advance::LoserOf(id(2)))
        ]
    );
    assert_eq!(pairings[5].advances_only_if_won_by, Some(Team::B));
    assert_eq!(
        from(6),
        [
            Some(Advance::LoserOf(id(5))),
            Some(Advance::WinnerOf(id(5)))
        ]
    );
}

#[test]
fn the_stage_is_serialized_under_its_explicitly_chosen_tag() {
    let phase = bracket(DoubleElimination::new(), 2);
    let first = phase.data().matches().values().next().unwrap();

    let value = serde_json::to_value(first).expect("serializing must not fail");

    assert_eq!(
        value["contest"]["annotations"],
        serde_json::json!([{
            "type": "tournament_engine::double_elimination_round",
            "value": { "Winners": { "round": 1, "rounds": 1 } }
        }])
    );
}
