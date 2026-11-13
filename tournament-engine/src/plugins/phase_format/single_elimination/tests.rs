use itertools::Itertools;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::{
    entities::{Annotated, Match, MatchResult, Phase, PhaseBuilder, RankingId},
    plugins::phase_format::{Advance, Seat},
};

#[test]
fn knockout_rounds_are_named_by_their_fraction_of_the_final() {
    let names = [2, 4, 8, 16, 32, 64, 128, 1024]
        .map(|entrants| KnockoutRound::RoundOf(entrants).to_string());

    assert_eq!(
        names,
        [
            "Final",
            "Semifinals",
            "Quarterfinals",
            "Round of 16",
            "Round of 32",
            "Round of 64",
            "Round of 128",
            "Round of 1024"
        ]
    );
    assert_eq!(KnockoutRound::ThirdPlace.to_string(), "Third place");
}

#[test]
fn a_stage_is_keyed_by_its_fraction_of_the_final() {
    let stages = [
        KnockoutRound::RoundOf(2),
        KnockoutRound::RoundOf(4),
        KnockoutRound::RoundOf(8),
        KnockoutRound::RoundOf(16),
        KnockoutRound::ThirdPlace,
    ]
    .map(Stage::from);

    assert_eq!(
        stages
            .each_ref()
            .map(|stage| (stage.key(), stage.numbers())),
        [
            ("final", &[][..]),
            ("semifinals", &[]),
            ("quarterfinals", &[]),
            ("fraction-finals", &[("fraction", 8)]),
            ("third-place", &[]),
        ]
    );
    assert_eq!(stages[3].to_string(), "Round of 16");
}

/// A knockout between `num_players` entrants with its first round already scheduled.
fn knockout(format: SingleElimination, num_players: u32) -> Phase {
    let mut phase = PhaseBuilder::new()
        .format(format)
        .add_players(num_players)
        .build();
    phase.create_new_matches();
    phase
}

/// The matches of `round`, each as the pair of initial unique ranks it is played between, in
/// ascending order so that the pairings of a round can be compared as a whole.
fn pairings(phase: &Phase, round: u32) -> Vec<(u32, u32)> {
    phase
        .data()
        .matches()
        .values()
        .filter(|r#match| r#match.round().get() == round)
        .map(|r#match| pairing_of(phase, r#match))
        .sorted()
        .collect()
}

/// The initial unique ranks of the two sides of `match`.
fn pairing_of(phase: &Phase, r#match: &Match) -> (u32, u32) {
    let rank_of = |id: &RankingId| phase.data().rankings()[id].initial_unique_rank().get();
    (rank_of(&r#match.team_a()[0]), rank_of(&r#match.team_b()[0]))
}

/// Reports every match of `round` as won by the side that entered the bracket higher, i.e.
/// lets the favourites advance, and schedules whatever follows from that.
fn play_round(phase: &mut Phase, round: u32) {
    let match_ids = phase
        .data()
        .matches()
        .values()
        .filter(|r#match| r#match.round().get() == round)
        .map(Match::id)
        .collect_vec();
    assert!(!match_ids.is_empty(), "round {round} has no matches");
    for match_id in match_ids {
        phase.report_match_result(match_id, MatchResult::AWins);
    }
    phase.create_new_matches();
}

#[test]
fn a_knockout_ends_with_the_final_unless_it_plays_for_third_place() {
    let mut phase = knockout(SingleElimination::new(), 4);

    assert_eq!(pairings(&phase, 1), [(1, 4), (2, 3)]);
    play_round(&mut phase, 1);

    assert_eq!(pairings(&phase, 2), [(1, 2)], "the final, and nothing else");
    play_round(&mut phase, 2);
    assert_eq!(phase.data().matches().len(), 3);
}

#[test]
fn the_match_for_third_place_is_scheduled_alongside_the_final() {
    let mut phase = knockout(SingleElimination::new().with_third_place_match(), 4);

    play_round(&mut phase, 1);

    assert_eq!(
        pairings(&phase, 2),
        [(1, 2), (3, 4)],
        "the two losers of the semifinals play in the round of the final"
    );
    play_round(&mut phase, 2);
    assert_eq!(
        phase.data().matches().len(),
        4,
        "the bracket is over afterwards"
    );
}

/// The match is played between whoever lost the semifinals, not between whoever entered the
/// bracket third and fourth, which is what a bracket the favourites lose shows.
#[test]
fn the_losers_of_the_semifinals_play_for_third_place() {
    let mut phase = knockout(SingleElimination::new().with_third_place_match(), 4);
    for match_id in phase.data().matches().values().map(Match::id).collect_vec() {
        // the lower ranked side wins both semifinals, so 1 and 2 are the ones knocked out
        phase.report_match_result(match_id, MatchResult::BWins);
    }

    phase.create_new_matches();

    assert_eq!(
        pairings(&phase, 2),
        [(2, 1), (4, 3)],
        "4 and 3 advanced to the final, so 2 and 1 are the ones playing for third place"
    );
}

/// A bracket of three gives one entrant a bye into the final, so only one of the three gets
/// knocked out before it and there is nobody for it to play against.
#[test]
fn three_entrants_do_not_play_for_third_place() {
    let mut phase = knockout(SingleElimination::new().with_third_place_match(), 3);

    assert_eq!(pairings(&phase, 1), [(2, 3)]);
    play_round(&mut phase, 1);

    assert_eq!(pairings(&phase, 2), [(1, 2)]);
    play_round(&mut phase, 2);
    assert_eq!(phase.data().matches().len(), 2);
}

/// The stages of the matches of `round`, in the order the pairings of [`pairings`] list them.
fn stages(phase: &Phase, round: u32) -> Vec<KnockoutRound> {
    let rank_of = |id: &RankingId| phase.data().rankings()[id].initial_unique_rank().get();
    phase
        .data()
        .matches()
        .values()
        .filter(|r#match| r#match.round().get() == round)
        .sorted_by_key(|r#match| (rank_of(&r#match.team_a()[0]), rank_of(&r#match.team_b()[0])))
        .map(|r#match| *r#match.annotation::<KnockoutRound>().unwrap())
        .collect()
}

/// The first round is named after the full bracket it fills, byes included, and the final
/// shares its round with the match for third place but not its stage.
#[test]
fn every_match_is_annotated_with_its_stage() {
    let mut phase = knockout(SingleElimination::new().with_third_place_match(), 6);
    assert_eq!(stages(&phase, 1), [KnockoutRound::RoundOf(8); 2]);

    play_round(&mut phase, 1);
    assert_eq!(stages(&phase, 2), [KnockoutRound::RoundOf(4); 2]);

    play_round(&mut phase, 2);
    assert_eq!(
        stages(&phase, 3),
        [KnockoutRound::RoundOf(2), KnockoutRound::ThirdPlace]
    );
}

/// The tag is part of the serialized form of every snapshot ever taken, so a change to it is a
/// breaking one and has to be deliberate.
#[test]
fn a_stage_is_serialized_under_its_explicitly_chosen_tag() {
    let phase = knockout(SingleElimination::new(), 2);
    let the_final = phase.data().matches().values().next().unwrap();

    let value = serde_json::to_value(the_final).expect("serializing must not fail");

    assert_eq!(
        value["contest"]["annotations"],
        serde_json::json!([{
            "type": "tournament_engine::knockout_round",
            "value": { "RoundOf": 2 }
        }])
    );
}

/// A match of the next round is scheduled as soon as the two matches it follows from are
/// decided, while the rest of the round is still being played.
#[test]
fn a_match_is_scheduled_as_soon_as_both_of_its_entrants_are_known() {
    let mut phase = knockout(SingleElimination::new(), 7);
    assert_eq!(pairings(&phase, 1), [(2, 7), (3, 6), (4, 5)]);
    let four_against_five = phase
        .data()
        .matches()
        .values()
        .find(|r#match| pairing_of(&phase, r#match) == (4, 5))
        .map(Match::id)
        .unwrap();

    phase.report_match_result(four_against_five, MatchResult::AWins);

    assert_eq!(pairings(&phase, 2), [(1, 4)], "1 had a bye and meets 4 now");
}

/// The best seeds advance without a match, each with a bye in the round it skips, annotated
/// with the stage of that round.
#[test]
fn the_best_seeds_get_the_byes() {
    let phase = knockout(SingleElimination::new(), 6);
    let phase = phase.data();

    let byes = phase
        .byes()
        .values()
        .map(|bye| {
            (
                phase.rankings()[&bye.ranking()].initial_unique_rank().get(),
                bye.round().get(),
                *bye.annotation::<KnockoutRound>().unwrap(),
            )
        })
        .sorted_by_key(|(seed, ..)| *seed)
        .collect_vec();
    assert_eq!(
        byes,
        [
            (1, 1, KnockoutRound::RoundOf(8)),
            (2, 1, KnockoutRound::RoundOf(8))
        ]
    );
}

/// Where two entrants with byes meet next, they play right away.
#[test]
fn two_byes_meeting_next_play_right_away() {
    let phase = knockout(SingleElimination::new(), 5);

    assert_eq!(pairings(&phase, 1), [(4, 5)]);
    assert_eq!(pairings(&phase, 2), [(2, 3)]);
}

/// The bracket of `phase`, read the way a consumer does, by downcasting its format.
fn bracket_of(phase: &Phase) -> &Bracket {
    let format: &dyn std::any::Any = phase.format();
    format
        .downcast_ref::<SingleElimination>()
        .and_then(SingleElimination::bracket)
        .expect("a started knockout has a bracket")
}

/// The final and the match for third place are in the bracket from the start, with the
/// pairings their sides come from, and take their entrants as those are decided.
#[test]
fn the_bracket_tells_who_meets_whom_before_it_is_scheduled() {
    let mut phase = knockout(SingleElimination::new().with_third_place_match(), 4);
    let pairings = bracket_of(&phase).pairings();
    let [semifinal_a, semifinal_b, the_final, third_place] = &pairings[..] else {
        panic!("expected two semifinals, the final and the match for third place");
    };

    assert!(semifinal_a.r#match.is_some() && semifinal_b.r#match.is_some());
    assert_eq!(the_final.r#match, None);
    assert_eq!(
        the_final.sides.map(|side| side.from),
        [
            Some(Advance::WinnerOf(semifinal_a.id)),
            Some(Advance::WinnerOf(semifinal_b.id))
        ]
    );
    assert_eq!(the_final.sides.map(|side| side.seat), [None, None]);
    assert_eq!(
        third_place.sides.map(|side| side.from),
        [
            Some(Advance::LoserOf(semifinal_b.id)),
            Some(Advance::LoserOf(semifinal_a.id))
        ]
    );
    assert_eq!(
        the_final.annotations.get::<KnockoutRound>(),
        Some(&KnockoutRound::RoundOf(2))
    );

    let winner = phase.data().matches()[&semifinal_a.r#match.unwrap()].team_a()[0];
    play_round(&mut phase, 1);
    let the_final = bracket_of(&phase).pairings()[2];
    assert_eq!(the_final.sides[0].seat, Some(Seat::Ranking(winner)));
    assert!(the_final.r#match.is_some());
    assert_eq!(
        the_final.annotations.get::<KnockoutRound>(),
        Some(&KnockoutRound::RoundOf(2)),
        "a scheduled pairing keeps its annotations"
    );
}

/// Two entrants play nothing but the final, so the option changes nothing at all.
#[test]
fn two_entrants_only_play_the_final() {
    let mut phase = knockout(SingleElimination::new().with_third_place_match(), 2);

    assert_eq!(pairings(&phase, 1), [(1, 2)]);
    play_round(&mut phase, 1);

    assert_eq!(phase.data().matches().len(), 1);
}

/// Only the semifinals decide who plays for third place, so the earlier rounds of a larger
/// bracket are scheduled exactly as they are without the option, byes included.
#[test]
fn a_larger_bracket_only_gains_the_one_match() {
    let mut phase = knockout(SingleElimination::new().with_third_place_match(), 6);

    assert_eq!(pairings(&phase, 1), [(3, 6), (4, 5)]);
    play_round(&mut phase, 1);

    assert_eq!(pairings(&phase, 2), [(1, 4), (2, 3)]);
    play_round(&mut phase, 2);

    assert_eq!(pairings(&phase, 3), [(1, 2), (3, 4)]);
    play_round(&mut phase, 3);
    assert_eq!(phase.data().matches().len(), 6);
}

/// The number of each match, by its pairing in the bracket, whether scheduled or not.
fn numbers(phase: &Phase) -> Vec<Option<u32>> {
    bracket_of(phase)
        .pairings()
        .iter()
        .map(|pairing| pairing.number.map(std::num::NonZeroU32::get))
        .collect()
}

/// The matches are numbered by their place in the bracket from the start, so a semifinal keeps
/// its number whichever is scheduled first; the pairings against a bye are no matches.
#[test]
fn the_matches_are_numbered_by_the_bracket_whatever_is_played_first() {
    let mut phase = knockout(SingleElimination::new().with_third_place_match(), 6);
    // the first round: two byes, two matches; the semifinals, the final, the third place
    assert_eq!(
        numbers(&phase),
        [
            None,
            Some(1),
            None,
            Some(2),
            Some(3),
            Some(4),
            Some(5),
            Some(6)
        ]
    );

    // the second quarterfinal first, which lets the second semifinal be scheduled
    let second = bracket_of(&phase).pairings()[3].r#match.unwrap();
    phase.report_match_result(second, MatchResult::AWins);
    phase.create_new_matches();

    let second_semifinal = bracket_of(&phase).pairings()[5].r#match.unwrap();
    assert_eq!(phase.data().matches()[&second_semifinal].number().get(), 4);
    let scheduled: Vec<_> = phase
        .data()
        .matches()
        .values()
        .map(|r#match| r#match.number().get())
        .collect();
    assert_eq!(
        scheduled,
        [1, 2, 4],
        "numbered by the bracket, not as scheduled"
    );
}
