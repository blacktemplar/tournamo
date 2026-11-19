//! The cross table of a phase whose matches are played by fixed teams, e.g. a round robin.

use std::collections::HashMap;

use tournament_engine::{
    Match, MatchResult, RankingId, Score, Sides,
    plugins::metrics_calculator::{Points, PointsCalculator, WinsDrawsLosses},
};

use super::{Session, bracket::plays_bracket};
use crate::view::{self, Chart, CrossMatch, CrossRow, CrossTable, DivisionRef, Outcome};

impl Session {
    /// What the division `of` shows of all its matches at once: its [`bracket`](Self::bracket),
    /// else its [`cross table`](Self::cross_table), if it has either; `None` before the start of
    /// its phase. It builds neither.
    pub fn chart(&self, of: DivisionRef) -> Option<Chart> {
        let division = self.division(of)?;
        if plays_bracket(division) {
            Some(Chart::Bracket)
        } else if division
            .data()
            .matches()
            .values()
            .all(|r#match| pair(r#match).is_some())
        {
            Some(Chart::CrossTable)
        } else {
            None
        }
    }

    /// The cross table of the division `of`, if each of its matches is played by two teams of one
    /// ranking each and its format plays no bracket (see [`Self::bracket`]), e.g. a round robin
    /// or random rounds; `None` before the start of its phase, and for teams drawn anew each
    /// round, as in a Monster DYP.
    pub fn cross_table(&self, of: DivisionRef) -> Option<CrossTable> {
        let division = self.division(of)?;
        if plays_bracket(division) {
            return None;
        }
        let data = division.data();
        let mut pairs: Vec<(RankingId, RankingId, &_)> = Vec::new();
        for r#match in data.matches().values() {
            let (a, b) = pair(r#match)?;
            pairs.push((a, b, r#match));
        }
        // in the order of the standings
        let ranked: Vec<_> = division
            .standings()
            .iter()
            .map(|standing| standing.ranking())
            .collect();
        let index: HashMap<RankingId, usize> = ranked
            .iter()
            .enumerate()
            .map(|(index, ranking)| (ranking.id(), index))
            .collect();
        // what a result is worth, if the phase counts points
        let points = division.plugins().metrics().iter().find_map(|calculator| {
            plugin_factory::downcast_ref::<PointsCalculator, _>(&**calculator)
        });
        let byes = |ranking: &tournament_engine::Ranking| {
            ranking
                .metric::<WinsDrawsLosses>()
                .map_or(0, |record| record.byes)
        };
        // the points of byes, where a bye scored any
        let bye_points = points
            .map(PointsCalculator::points_for_bye)
            .filter(|points| !points.is_zero() && ranked.iter().any(|ranking| byes(ranking) > 0));
        let mut rows: Vec<CrossRow> = ranked
            .iter()
            .map(|ranking| CrossRow {
                entrants: self.team(data, &[ranking.id()]),
                cells: vec![Vec::new(); ranked.len()],
                byes: bye_points.map(|points| points * byes(ranking).into()),
                total: points.map(|_| {
                    ranking
                        .metric::<Points>()
                        .map_or(Score::ZERO, Points::total)
                }),
            })
            .collect();
        for (a, b, r#match) in pairs {
            let row = self.match_row(of.phase, of.division, division, r#match);
            let outcomes = match row.result {
                None => [None; 2],
                Some(MatchResult::AWins) => [Some(Outcome::Win), Some(Outcome::Loss)],
                Some(MatchResult::BWins) => [Some(Outcome::Loss), Some(Outcome::Win)],
                Some(MatchResult::Draw) => [Some(Outcome::Draw); 2],
            };
            let score = view::top_score(division, r#match);
            let scores = [score, score.map(|score| Sides::new(score.b, score.a))];
            let (a, b) = (index[&a], index[&b]);
            for (((team, against), outcome), score) in
                [(a, b), (b, a)].into_iter().zip(outcomes).zip(scores)
            {
                rows[team].cells[against].push(CrossMatch {
                    row: row.clone(),
                    outcome,
                    points: points.zip(outcome).map(|(points, outcome)| match outcome {
                        Outcome::Win => points.points_for_win(),
                        Outcome::Draw => points.points_for_draw(),
                        Outcome::Loss => Score::ZERO,
                    }),
                    score,
                });
            }
        }
        for row in &mut rows {
            for cell in &mut row.cells {
                cell.sort_by_key(|played| played.row.number);
            }
        }
        Some(CrossTable { rows })
    }
}

/// The ranking of each side of `r#match`, if each side is a team of one ranking.
fn pair(r#match: &Match) -> Option<(RankingId, RankingId)> {
    match (r#match.team_a().as_slice(), r#match.team_b().as_slice()) {
        ([a], [b]) => Some((*a, *b)),
        _ => None,
    }
}
