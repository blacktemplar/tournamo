//! A format can pair by the standings of its phase, as a Swiss system does: the standings it asks
//! for count every result reported so far, and replaying the log pairs the same.

use std::num::NonZeroU32;

use itertools::Itertools;
use serde::{Deserialize, Serialize};
use tournament_engine::{
    AddEntrant, AddPhase, CompetitionStateMachine, Entrant, EntrantId, Match, MatchResult,
    NewMatch, PhaseId, PhasePluginsBuilder, PhaseView, RankingId, ReportMatchResult,
    StartCompetition,
    plugin_factory::{Plugin, register},
    plugins::{
        metrics_calculator::{MoreWins, WinsDrawsLossesCalculator},
        phase_format::{PhaseFormat, Schedule},
    },
    uuid::Uuid,
};
use vec1::vec1;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

/// Plays two rounds, each once the round before is played, pairing the best of the standings with
/// the second best, the third with the fourth, and so on.
#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(id = "pairing_by_standings_test::by_standings", label = "by standings")]
struct ByStandings;

#[register]
impl PhaseFormat for ByStandings {
    fn create_new_matches(&mut self, phase: &PhaseView<'_>) -> Schedule {
        let matches = phase.data().matches();
        if matches.values().any(|r#match| !r#match.is_finished()) {
            return Schedule::default();
        }
        let round = matches
            .values()
            .map(Match::round)
            .max()
            .map_or(NonZeroU32::MIN, |round| round.saturating_add(1));
        if round.get() > 2 {
            return Schedule::default();
        }
        phase
            .standings()
            .iter()
            .map(|standing| standing.ranking().id())
            .tuples()
            .map(|(better, worse)| NewMatch::new(vec1![better], vec1![worse], round))
            .collect_vec()
            .into()
    }
}

fn phase_id() -> PhaseId {
    PhaseId::new(Uuid::from_u128(0xfa5e))
}

/// Every match of the phase as its round and the ranking of each side, in the order they were
/// scheduled in.
fn matches(sm: &CompetitionStateMachine) -> Vec<(u32, RankingId, RankingId)> {
    sm.current().phases()[&phase_id()]
        .data()
        .matches()
        .values()
        .map(|r#match| {
            (
                r#match.round().get(),
                r#match.team_a()[0],
                r#match.team_b()[0],
            )
        })
        .collect()
}

#[test]
fn a_format_pairs_by_the_standings_and_a_replay_pairs_the_same() {
    let mut sm = CompetitionStateMachine::default();
    for entrant in 0..4 {
        let entrant = Entrant::new(EntrantId::new(Uuid::from_u128(entrant)));
        sm.apply_new(AddEntrant::new(entrant).into()).unwrap();
    }
    let plugins = PhasePluginsBuilder::new()
        .format(ByStandings)
        .add_ranking(MoreWins)
        .add_metric(WinsDrawsLossesCalculator)
        .build()
        .unwrap();
    sm.apply_new(AddPhase::new(phase_id(), plugins).into())
        .unwrap();
    sm.apply_new(StartCompetition.into()).unwrap();
    let data = sm.current().phases()[&phase_id()].data();
    let rankings: Vec<_> = data.rankings().keys().copied().collect();
    let [first, second, third, fourth] = rankings[..] else {
        unreachable!("four entrants are four rankings")
    };
    let round_one: Vec<_> = data.matches().values().map(Match::id).collect();
    // nobody has won yet, so the standings are in the initial order
    assert_eq!(matches(&sm), [(1, first, second), (1, third, fourth)]);

    for (r#match, result) in round_one
        .into_iter()
        .zip([MatchResult::AWins, MatchResult::BWins])
    {
        sm.apply_new(ReportMatchResult::new(phase_id(), r#match, result).into())
            .unwrap();
    }

    // the two winners meet, as do the two losers
    assert_eq!(
        matches(&sm),
        [
            (1, first, second),
            (1, third, fourth),
            (2, first, fourth),
            (2, second, third),
        ]
    );
    let json = serde_json::to_string(&sm).unwrap();
    let replayed: CompetitionStateMachine = serde_json::from_str(&json).unwrap();
    assert_eq!(matches(&replayed), matches(&sm));
}
