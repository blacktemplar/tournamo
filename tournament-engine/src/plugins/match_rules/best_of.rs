//! The rules of a match played in games, the first side winning most of them winning the match,
//! e.g. a best of three games of badminton.

use plugin_factory::{Plugin, compatibility::Declaration, register};
use serde::{Deserialize, Serialize};

use crate::{
    entities::{
        competition::{Match, MatchResult, PhaseData},
        details::{Part, Sides},
    },
    plugins::{
        Phrase,
        facts::MatchTally,
        match_rules::{
            GamesWon, Level, MatchRules, Parts, PointsScored, Template,
            scored::{result_of, score, score_field, suggest_by_score},
        },
    },
    type_map::TypeMap,
};

/// The rules of a match played in up to `games` games, each decided by its score: the side winning
/// most of them wins the match.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::best_of",
    label = "best of games",
    description = "the side winning most games wins, each game decided by its score",
    declare_static = Self::declare_static
)]
pub struct BestOf {
    #[plugin(label = "games", min = 1, default = 3)]
    games: u32,
    #[serde(default)]
    #[plugin(
        label = "usual top score of a game",
        description = "a higher score is pointed out, as it may be a typo",
        default = None
    )]
    usual_top_score: Option<u32>,
}

impl BestOf {
    /// Rules of up to `games` games, pointing out no score.
    ///
    /// Requires the `test-util` feature.
    #[cfg(feature = "test-util")]
    pub const fn new(games: u32) -> Self {
        Self {
            games,
            usual_top_score: None,
        }
    }

    fn declare_static(declaration: &mut Declaration) {
        declaration
            .provide(MatchTally::<GamesWon>::new())
            .provide(MatchTally::<PointsScored>::new());
    }

    /// The level of a game.
    fn game(&self) -> Level {
        Level {
            fields: vec![score_field(self.usual_top_score)],
            ..Level::new(Phrase::new("game", "game"))
        }
    }

    /// The games each side won of `games`.
    fn won(games: &[Part]) -> Sides<u32> {
        let mut won = Sides::new(0, 0);
        for game in games {
            match game.decision().map(|decision| decision.result()) {
                Some(MatchResult::AWins) => won.a += 1,
                Some(MatchResult::BWins) => won.b += 1,
                _ => {}
            }
        }
        won
    }
}

#[register]
impl MatchRules for BestOf {
    fn template(&self, _: &PhaseData, _: &Match) -> Template {
        Template {
            root: Level {
                parts: Parts::Same {
                    level: Box::new(self.game()),
                    max: self.games,
                },
                ..Level::new(Phrase::new("match", "match"))
            },
        }
    }

    /// The games each side won, and the points it scored in all of them.
    fn tally(&self, _: &PhaseData, r#match: &Match) -> TypeMap {
        let games = r#match.contest().parts();
        let mut points = Sides::new(0, 0);
        for score in games.iter().filter_map(score) {
            points.a += score.a;
            points.b += score.b;
        }
        let mut tallies = TypeMap::new();
        tallies.insert(GamesWon(Self::won(games)));
        tallies.insert(PointsScored(points));
        tallies
    }

    /// A game by its score; the match for the side that won most games, once one has won more
    /// than half of them or every game is decided.
    fn suggest(&self, level: &Level, part: &Part) -> Option<MatchResult> {
        if level.name.key() == "game" {
            return suggest_by_score(part);
        }
        let won = Self::won(part.parts());
        let needed = self.games / 2 + 1;
        if won.a >= needed || won.b >= needed || won.a + won.b >= self.games {
            Some(result_of(won))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::*;
    use crate::{
        entities::{
            competition::{Phase, PhaseBuilder},
            details::{Decision, EnteredValue, Values},
        },
        plugins::{
            match_rules::scored::SCORE,
            metrics_calculator::test_utils::{match_id, phase_with},
        },
    };

    /// A game `a` to `b`, decided by its score.
    fn game(a: u32, b: u32) -> Part {
        let score = Sides::new(Some(EnteredValue::Count(a)), Some(EnteredValue::Count(b)));
        Part::new()
            .with_decision(result_of(Sides::new(a, b)))
            .with_value(SCORE, score)
    }

    fn phase(rules: BestOf) -> Phase {
        phase_with(
            PhaseBuilder::new().results(rules).add_players(2),
            vec![(1, vec![0], vec![1])],
        )
    }

    #[test]
    fn a_match_tallies_the_games_won_and_the_points_of_all_games() {
        let mut phase = phase(BestOf::new(3));
        let id = match_id(&phase, 1, &[0], &[1]);
        let games = vec![game(21, 17), game(19, 21), game(21, 15)];

        phase.report_match(id, Decision::new(MatchResult::AWins), Values::new(), games);

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
    fn a_match_is_suggested_won_by_the_side_winning_most_games() {
        let rules = BestOf::new(3);
        let level = Level::new(Phrase::new("match", "match"));
        let suggest = |games: Vec<Part>| {
            let part = games.into_iter().fold(Part::new(), Part::with_part);
            rules.suggest(&level, &part)
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
    fn an_even_number_of_games_can_end_in_a_draw() {
        let rules = BestOf::new(2);
        let level = Level::new(Phrase::new("match", "match"));
        let part = Part::new().with_part(game(21, 3)).with_part(game(3, 21));

        assert_eq!(rules.suggest(&level, &part), Some(MatchResult::Draw));
    }

    #[test]
    fn a_game_is_suggested_won_by_its_score() {
        let suggested = BestOf::new(3).suggest(&BestOf::new(3).game(), &game(21, 23));

        assert_eq!(suggested, Some(MatchResult::BWins));
    }

    /// The tags are part of every log and snapshot written, see `transition_log_serialization.rs`.
    #[test]
    fn the_rules_and_their_tally_are_serialized_under_their_tags() {
        let rules: Box<dyn MatchRules> = Box::new(BestOf::new(3));
        let mut tallies = TypeMap::new();
        tallies.insert(GamesWon::default());

        assert_eq!(
            serde_json::to_value(&rules).unwrap()["type"],
            "tournament_engine::best_of"
        );
        assert_eq!(
            serde_json::to_value(&tallies).unwrap()[0]["type"],
            "tournament_engine::games_won"
        );
    }
}
