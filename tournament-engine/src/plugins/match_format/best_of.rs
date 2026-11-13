//! The match format of a match played in games, the first side winning most of them winning the
//! match, e.g. a best of three games of badminton.

use plugin_factory::{Plugin, compatibility::Declaration, register};
use serde::{Deserialize, Serialize};

use crate::{
    entities::{
        Match, MatchResult, PhaseData,
        details::{Part, Sides},
    },
    plugins::{
        Phrase,
        facts::MatchTally,
        match_format::{
            GamesWon, Level, MatchFormat, Parts, PointsScored, Template,
            scored::{result_of, score, score_field, suggest_by_score},
        },
    },
    type_map::TypeMap,
};

/// The match format of a match played in up to `games` games, each decided by its score: the side
/// winning most of them wins the match.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::best_of",
    label = "best of games",
    description = "the side winning most games wins, each game decided by its score",
    declare_static = Self::declare_static
)]
pub struct BestOf {
    // a result is entered from the fewest games that may decide the match, more than half of
    // them, so far more than are played (a snooker final has 35 frames) would hold the page up
    #[plugin(label = "games", min = 1, max = 99, default = 3)]
    games: u32,
    #[serde(default)]
    #[plugin(
        label = "winning score of a game",
        description = "what the winner of a game usually has, or more, then winning by one or \
                       two; any other score is pointed out, as it may be a typo",
        min = 1,
        default = None
    )]
    winning_score: Option<u16>,
}

impl BestOf {
    /// A match format of up to `games` games, pointing out no score.
    ///
    /// Requires the `test-util` feature.
    #[cfg(feature = "test-util")]
    pub const fn new(games: u32) -> Self {
        Self {
            games,
            winning_score: None,
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
            fields: vec![score_field(self.winning_score)],
            ..Level::new(Phrase::new("game", "game"))
        }
    }

    /// The level of the match, played in games.
    fn match_level(&self) -> Level {
        Level {
            parts: Parts::Same {
                level: Box::new(self.game()),
                min: self.needed(),
                max: self.games,
            },
            ..Level::new(Phrase::new("match", "match"))
        }
    }

    /// The games a side needs to win the match: more than half of them.
    const fn needed(&self) -> u32 {
        self.games / 2 + 1
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
impl MatchFormat for BestOf {
    fn template(&self, _: &PhaseData, _: &Match) -> Template {
        Template {
            root: self.match_level(),
        }
    }

    /// The games each side won, and the points it scored in all of them.
    fn tally(&self, _: &PhaseData, r#match: &Match) -> TypeMap {
        let games = r#match.contest().parts();
        let mut points = Sides::new(0u32, 0u32);
        for score in games.iter().filter_map(score) {
            points.a = points.a.saturating_add(score.a);
            points.b = points.b.saturating_add(score.b);
        }
        let mut tallies = TypeMap::new();
        tallies.insert(GamesWon(Self::won(games)));
        tallies.insert(PointsScored(points));
        tallies
    }

    /// A game, the level without parts, by its score; the match for the side that won most
    /// games, once one has won more than the other side can still win or every game is decided.
    fn suggest(&self, level: &Level, part: &Part) -> Option<MatchResult> {
        if matches!(level.parts, Parts::None) {
            return suggest_by_score(part);
        }
        let won = Self::won(part.parts());
        let decided = part
            .parts()
            .iter()
            .filter(|game| game.decision().is_some())
            .count();
        let remaining = self.games.saturating_sub(decided as u32);
        if won.a.abs_diff(won.b) > remaining || remaining == 0 {
            Some(result_of(won))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests;
