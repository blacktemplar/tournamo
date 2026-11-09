//! The cross table of a phase whose matches are played by fixed teams, e.g. a round robin.

use std::collections::HashMap;

use tournament_engine::{MatchResult, RankingId, Score, Sides};

use super::{Session, bracket::plays_bracket};
use crate::view::{self, CrossMatch, CrossRow, CrossTable, Outcome};

/// What a draw adds to the total of a row.
const HALF: Score = Score::new(0.5);

impl Session {
    /// The cross table of the phase, if each of its matches is played by two teams of one
    /// ranking each and its format plays no bracket (see [`Self::bracket`]), e.g. a round robin
    /// or random rounds; `None` before the start, and for teams drawn anew each round, as in a
    /// Monster DYP.
    pub fn cross_table(&self) -> Option<CrossTable> {
        let phase = self.started_phase()?;
        if plays_bracket(phase) {
            return None;
        }
        let data = phase.data();
        let mut pairs: Vec<(RankingId, RankingId, &_)> = Vec::new();
        for r#match in data.matches().values() {
            let ([a], [b]) = (r#match.team_a().as_slice(), r#match.team_b().as_slice()) else {
                return None;
            };
            pairs.push((*a, *b, r#match));
        }
        // in the order of the standings
        let ranked: Vec<_> = data.standings().collect();
        let index: HashMap<RankingId, usize> = ranked
            .iter()
            .enumerate()
            .map(|(index, ranking)| (ranking.id(), index))
            .collect();
        let mut rows: Vec<CrossRow> = ranked
            .iter()
            .map(|ranking| CrossRow {
                entrants: self.team(data, &[ranking.id()]),
                cells: vec![Vec::new(); ranked.len()],
                total: Score::ZERO,
            })
            .collect();
        for (a, b, r#match) in pairs {
            let row = self.match_row(phase, r#match);
            let outcomes = match row.result {
                None => [None; 2],
                Some(MatchResult::AWins) => [Some(Outcome::Win), Some(Outcome::Loss)],
                Some(MatchResult::BWins) => [Some(Outcome::Loss), Some(Outcome::Win)],
                Some(MatchResult::Draw) => [Some(Outcome::Draw); 2],
            };
            let score = view::top_score(phase, r#match);
            let scores = [score, score.map(|score| Sides::new(score.b, score.a))];
            let (a, b) = (index[&a], index[&b]);
            for (((team, against), outcome), score) in
                [(a, b), (b, a)].into_iter().zip(outcomes).zip(scores)
            {
                rows[team].total += match outcome {
                    Some(Outcome::Win) => Score::ONE,
                    Some(Outcome::Draw) => HALF,
                    Some(Outcome::Loss) | None => Score::ZERO,
                };
                rows[team].cells[against].push(CrossMatch {
                    row: row.clone(),
                    outcome,
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
