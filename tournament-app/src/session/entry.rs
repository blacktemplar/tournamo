//! Reading and reporting a result typed as one line, see [`Session::read_entered`].

use tournament_engine::{EnteredValue, MatchResult, Phase, Sides, Team};

use super::Session;
use crate::{
    error::{ReportEnteredError, ReportError},
    result_form::read_score,
    view::{Entered, EntrantRow, MatchRow, NameSearch, Reported},
};

impl Session {
    /// Reports the result `entry` says, see [`Self::read_entered`].
    ///
    /// # Errors
    /// Returns an error if [`Self::read_entered`] reports none, or the result is not reported, see
    /// [`Self::report`].
    pub fn report_entered(&mut self, entry: &str) -> Result<Reported, ReportEnteredError> {
        let row = self.read_entered(entry).report?;
        let result = row.result.expect("an entry says a result");
        let reported = if row.scores.is_empty() {
            self.report(row.id, result)
        } else {
            let mut form = self.result_form(row.id)?;
            let entered = form.enter_scores(&row.scores);
            debug_assert!(entered, "the scores of an entry fit its match");
            form.choose(&[], Some(result));
            self.report_form(&form)
        };
        reported.map_err(|error| match error {
            ReportError::AlreadyFinished => ReportEnteredError::AlreadyFinished(row.number),
            error => error.into(),
        })
    }

    /// The matches `entry` fits, and the result it says, which [`Self::report_entered`] would
    /// report. Spaces around it do not count. `entry` is either
    ///
    /// - the number of a match, as [`MatchRow::number`], and its result, separated by whitespace
    ///   or a `-`: `1` for a win of side A, `2` for one of side B, and `X` (or `x`) for a draw,
    ///   which needs no separator, e.g. `"3 1"`, `"3-1"`, `"12 X"` or `"12x"`; or its scores,
    ///   side A's first, followed by its result only where they do not say it, each apart by
    ///   whitespace, e.g. `"3 21-17"`, `"3 21-17 19-21 21-15"` or `"3 21-17 1"`. It fits that
    ///   match, also with a result already or without a result entered (`"3"`); or
    /// - part of the name of the winner, found as [`NameSearch`] finds it, optionally followed by
    ///   the number of the match and by `X` for a draw, in either order, and then by the scores,
    ///   the named side's first, each apart by whitespace, e.g. `"ali"`, `"ali X"`, `"ali 5"`,
    ///   `"ali 5 X"` or `"ali 21-17"`. It fits the open matches with an entrant of that name, and
    ///   says a result only if they are one side of one match (one match for a draw, on one side
    ///   of it with scores), whichever of them is playable now. A number, an `X` or scores at the
    ///   end are read as a part of the name as long as a name contains them, e.g. `"Malcolm X"` is
    ///   Malcolm X's win, and `"Malcolm X X"` a draw of his match.
    ///
    /// A score is one [`read_score`] reads, e.g. `21-17`, `21–17` or `9.5-10`: that of the match,
    /// or else one of each of its parts in order, e.g. its games, as [`ResultForm::enter_scores`]
    /// enters them. The result entered must be the one they say, if they say one: a result
    /// against them is entered in a [`ResultForm`].
    ///
    /// The result is an error if the competition has not started, `entry` is empty, it is the
    /// number of a match alone, there is no match of the number entered, it has a result already,
    /// its format has no draws while a draw is entered or scored, the name entered fits no open
    /// match, both sides of one, or several matches, the match does not take the scores entered,
    /// they say no result while none is entered, or another than the one entered.
    ///
    /// [`ResultForm`]: crate::result_form::ResultForm
    /// [`ResultForm::enter_scores`]: crate::result_form::ResultForm::enter_scores
    pub fn read_entered(&self, entry: &str) -> Entered {
        let nothing = |error: ReportEnteredError| Entered {
            found: Vec::new(),
            report: Err(error),
        };
        let Some(phase) = self.started_phase() else {
            return nothing(ReportError::NotStarted.into());
        };
        let entry = entry.trim();
        if entry.is_empty() {
            return nothing(ReportEnteredError::Unreadable);
        }
        let Some(NumberedEntry {
            number,
            scores,
            result,
        }) = read_entry(entry)
        else {
            let mut open: Vec<_> = phase
                .data()
                .matches()
                .values()
                .filter(|r#match| !r#match.is_finished())
                .map(|r#match| self.match_row(phase, r#match))
                .collect();
            open.sort_by_key(|row| row.number);
            let Named { found, says } = read_named(&open, entry, phase.format().allows_draws());
            let report = says.and_then(|Says { row, scores }| {
                let result = row.result;
                self.with_scores(phase, row, &scores, result)
            });
            return Entered { found, report };
        };
        let Some(r#match) = phase
            .data()
            .matches()
            .values()
            .find(|r#match| super::views::number(r#match) == number)
        else {
            return nothing(ReportEnteredError::NoMatch(number));
        };
        let row = self.match_row(phase, r#match);
        let report = match result {
            _ if r#match.is_finished() => Err(ReportEnteredError::AlreadyFinished(number)),
            None if scores.is_empty() => Err(ReportEnteredError::MissingResult(number)),
            result => self.with_scores(phase, row.clone(), &scores, result),
        };
        Entered {
            found: vec![row],
            report,
        }
    }

    /// `row`, an open match of `phase`, with the result `scores` (side A's first) and `result`
    /// say, see [`Self::read_entered`].
    fn with_scores(
        &self,
        phase: &Phase,
        row: MatchRow,
        scores: &[Sides<EnteredValue>],
        result: Option<MatchResult>,
    ) -> Result<MatchRow, ReportEnteredError> {
        let scores: Vec<_> = scores
            .iter()
            .map(|score| Sides::new(Some(score.a), Some(score.b)))
            .collect();
        let result = if scores.is_empty() {
            result
        } else {
            let mut form = self.result_form(row.id)?;
            if !form.enter_scores(&scores) {
                return Err(ReportEnteredError::ScoresNotTaken(row.number));
            }
            match (result, form.result()) {
                (Some(entered), Some(suggested)) if entered != suggested => {
                    return Err(ReportEnteredError::AgainstScores(row.number));
                }
                (None, None) => return Err(ReportEnteredError::Undecided(row.number)),
                (entered, suggested) => entered.or(suggested),
            }
        };
        let result = result.expect("a result entered or suggested");
        if result == MatchResult::Draw && !phase.format().allows_draws() {
            return Err(ReportError::DrawNotAllowed.into());
        }
        Ok(MatchRow {
            result: Some(result),
            scores,
            ..row
        })
    }
}

/// What an entry of part of a name fits and says, see [`read_named`].
#[derive(Debug)]
struct Named {
    /// The matches it fits, by their numbers.
    found: Vec<MatchRow>,
    says: Result<Says, ReportEnteredError>,
}

/// The match an entry says the result of, with that result, and the scores entered, which are yet
/// to be entered into the match.
#[derive(Debug)]
struct Says {
    row: MatchRow,
    /// Side A's first.
    scores: Vec<Sides<EnteredValue>>,
}

/// What `entry`, part of a name and perhaps the number of a match and a draw, fits of the open
/// matches `open`, in the order of their numbers, and says, in a phase that takes draws if `draws`,
/// see [`Session::read_entered`].
fn read_named(open: &[MatchRow], entry: &str, draws: bool) -> Named {
    let readings = NamedEntry::readings(entry);
    // the longest name that fits
    let Some((reading, fitting)) = readings.iter().find_map(|reading| {
        let fitting = reading.fitting(open);
        (!fitting.is_empty()).then_some((reading, fitting))
    }) else {
        // a name followed by a number, which fits only without it
        let error = readings
            .iter()
            .find_map(|reading| {
                let number = reading.number?;
                let anywhere = NamedEntry {
                    number: None,
                    ..reading.clone()
                };
                (!anywhere.fitting(open).is_empty()).then(|| ReportEnteredError::NotInMatch {
                    name: reading.name.to_owned(),
                    number,
                })
            })
            .unwrap_or_else(|| ReportEnteredError::NoNameFound(entry.to_owned()));
        return Named {
            found: Vec::new(),
            says: Err(error),
        };
    };
    let mut found: Vec<MatchRow> = Vec::new();
    for (row, ..) in &fitting {
        if found.last().is_none_or(|last| last.id != row.id) {
            found.push((*row).clone());
        }
    }
    let says = if reading.draw && !draws {
        Err(ReportError::DrawNotAllowed.into())
    } else {
        match (fitting.as_slice(), found.as_slice()) {
            ([(row, result, side)], _) => {
                let row = MatchRow {
                    result: Some(*result),
                    ..(*row).clone()
                };
                match (reading.scores.as_slice(), side) {
                    ([], _) => Ok(Says {
                        row,
                        scores: Vec::new(),
                    }),
                    // scores of a draw whose both sides have the name are no one's
                    (_, None) => Err(ReportEnteredError::BothSides {
                        name: reading.name.to_owned(),
                        number: row.number,
                    }),
                    (scores, Some(team)) => Ok(Says {
                        row,
                        scores: scores
                            .iter()
                            .map(|score| match team {
                                Team::A => *score,
                                Team::B => Sides::new(score.b, score.a),
                            })
                            .collect(),
                    }),
                }
            }
            (_, [row]) => Err(ReportEnteredError::BothSides {
                name: reading.name.to_owned(),
                number: row.number,
            }),
            (_, rows) => Err(ReportEnteredError::SeveralMatches {
                name: reading.name.to_owned(),
                matches: rows.len(),
                number: rows[0].number,
            }),
        }
    };
    Named { found, says }
}

/// What an entry of the number of a match says, see [`Session::read_entered`].
#[derive(Debug, PartialEq)]
struct NumberedEntry {
    number: usize,
    /// Side A's first.
    scores: Vec<Sides<EnteredValue>>,
    result: Option<MatchResult>,
}

/// What `entry` says, if it starts with the number of a match, see [`Session::read_entered`].
fn read_entry(entry: &str) -> Option<NumberedEntry> {
    let entry = entry.trim();
    let digits = entry
        .find(|char: char| !char.is_ascii_digit())
        .unwrap_or(entry.len());
    let (number, rest) = entry.split_at(digits);
    let number = number.parse().ok()?;
    let numbered = |scores, result| {
        Some(NumberedEntry {
            number,
            scores,
            result,
        })
    };
    if rest.is_empty() {
        return numbered(Vec::new(), None);
    }
    let result = rest.trim_start_matches(|char: char| char.is_whitespace() || char == '-');
    // a win needs a separator, or its side would be read as a digit of the number
    if let Some(result) = read_result(result, result.len() < rest.len()) {
        return numbered(Vec::new(), Some(result));
    }
    // scores apart by whitespace, perhaps followed by the result
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let mut words: Vec<_> = rest.split_whitespace().collect();
    let result = words.last().and_then(|last| read_result(last, true));
    if result.is_some() {
        words.pop();
    }
    let scores = words
        .into_iter()
        .map(read_score)
        .collect::<Option<Vec<_>>>()?;
    numbered(scores, result)
}

/// The result `text` says: `1`, `2` (if `separated` from the number of its match) or `X`.
fn read_result(text: &str, separated: bool) -> Option<MatchResult> {
    match text {
        "1" if separated => Some(MatchResult::AWins),
        "2" if separated => Some(MatchResult::BWins),
        "X" | "x" => Some(MatchResult::Draw),
        _ => None,
    }
}

/// A way to read an entry of part of a name, see [`Session::read_entered`].
#[derive(Debug, Clone)]
struct NamedEntry<'a> {
    name: &'a str,
    /// The number of the match it is restricted to.
    number: Option<usize>,
    draw: bool,
    /// The named side's first.
    scores: Vec<Sides<EnteredValue>>,
}

impl<'a> NamedEntry<'a> {
    /// The ways to read `entry`, the longest name first: all of it, then without the last word if
    /// that is a number or an `X`, then without the last two if they are one of each; then the
    /// same without the scores at its end, if any.
    fn readings(entry: &'a str) -> Vec<Self> {
        let mut readings = Self::without_number(entry, Vec::new());
        let mut name = entry;
        let mut scores = Vec::new();
        while let Some((before, last)) = name.rsplit_once(char::is_whitespace)
            && let Some(score) = read_score(last)
        {
            scores.insert(0, score);
            name = before.trim_end();
        }
        if !scores.is_empty() {
            readings.extend(Self::without_number(name, scores));
        }
        readings
    }

    /// The ways to read `entry`, followed by `scores`, the longest name first: all of it, then
    /// without the last word if that is a number or an `X`, then without the last two if they are
    /// one of each.
    fn without_number(entry: &'a str, scores: Vec<Sides<EnteredValue>>) -> Vec<Self> {
        let mut reading = Self {
            name: entry,
            number: None,
            draw: false,
            scores,
        };
        let mut readings = vec![reading.clone()];
        for _ in 0..2 {
            let Some((before, last)) = reading.name.rsplit_once(char::is_whitespace) else {
                break;
            };
            if reading.number.is_none()
                && let Ok(number) = last.parse()
                && last.chars().all(|char| char.is_ascii_digit())
            {
                reading.number = Some(number);
            } else if !reading.draw && (last == "X" || last == "x") {
                reading.draw = true;
            } else {
                break;
            }
            reading.name = before.trim_end();
            readings.push(reading.clone());
        }
        readings
    }

    /// The open matches of `open` this reading fits, with the result it says of each and the side
    /// named: a win of each side with an entrant of its name, or a draw of a match with one, whose
    /// side is `None` if both sides have one.
    fn fitting<'r>(&self, open: &'r [MatchRow]) -> Vec<(&'r MatchRow, MatchResult, Option<Team>)> {
        let search = NameSearch::new(self.name);
        let finds = |side: &[EntrantRow]| side.iter().any(|entrant| search.finds(entrant));
        open.iter()
            .filter(|row| self.number.is_none_or(|number| row.number == number))
            .flat_map(|row| {
                let (a, b) = (finds(&row.a), finds(&row.b));
                let results = if self.draw {
                    let side = match (a, b) {
                        (true, true) => Some(None),
                        (true, false) => Some(Some(Team::A)),
                        (false, true) => Some(Some(Team::B)),
                        (false, false) => None,
                    };
                    side.map(|side| (MatchResult::Draw, side))
                        .into_iter()
                        .collect()
                } else {
                    [
                        (a, MatchResult::AWins, Team::A),
                        (b, MatchResult::BWins, Team::B),
                    ]
                    .into_iter()
                    .filter(|(found, ..)| *found)
                    .map(|(_, result, team)| (result, Some(team)))
                    .collect::<Vec<_>>()
                };
                results
                    .into_iter()
                    .map(move |(result, side)| (row, result, side))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use tournament_engine::{EntrantId, uuid::Uuid};
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::*;

    fn count(a: u16, b: u16) -> Sides<EnteredValue> {
        Sides::new(EnteredValue::Count(a), EnteredValue::Count(b))
    }

    #[test]
    fn a_number_is_followed_by_its_result_or_scores() {
        let numbered = |number, scores, result| {
            Some(NumberedEntry {
                number,
                scores,
                result,
            })
        };
        assert_eq!(
            read_entry("3 1"),
            numbered(3, vec![], Some(MatchResult::AWins))
        );
        assert_eq!(
            read_entry("3-2"),
            numbered(3, vec![], Some(MatchResult::BWins))
        );
        assert_eq!(
            read_entry("12x"),
            numbered(12, vec![], Some(MatchResult::Draw))
        );
        assert_eq!(
            read_entry(" 12 X "),
            numbered(12, vec![], Some(MatchResult::Draw))
        );
        assert_eq!(read_entry("3"), numbered(3, vec![], None));
        // a win needs a separator
        assert_eq!(read_entry("31"), numbered(31, vec![], None));
        assert_eq!(
            read_entry("3 21-17 19-21 1"),
            numbered(
                3,
                vec![count(21, 17), count(19, 21)],
                Some(MatchResult::AWins)
            )
        );
        assert_eq!(
            read_entry("3 21-17"),
            numbered(3, vec![count(21, 17)], None)
        );
    }

    #[test]
    fn an_entry_not_starting_with_a_number_is_none() {
        assert_eq!(read_entry("ali"), None);
        assert_eq!(read_entry("3ali"), None);
        assert_eq!(read_entry("3 ali"), None);
    }

    /// The open match numbered `number`, of Alice and Bob.
    fn open_match(number: usize) -> MatchRow {
        let id = serde_json::from_value(serde_json::json!(Uuid::from_u128(number as u128)))
            .expect("a match id is a UUID");
        MatchRow {
            id,
            number,
            round: NonZeroU32::MIN,
            stage: None,
            a: team(&["Alice"]),
            b: team(&["Bob"]),
            result: None,
            scores: Vec::new(),
            parts_won: None,
            details: false,
        }
    }

    fn team(names: &[&str]) -> Vec<EntrantRow> {
        names
            .iter()
            .map(|name| EntrantRow {
                id: EntrantId::new(Uuid::new_v5(&Uuid::nil(), name.as_bytes())),
                name: Some((*name).to_owned()),
            })
            .collect()
    }

    /// The match `named` says the result of, its number, and the scores entered.
    fn says(named: Named) -> (usize, Option<MatchResult>, Vec<Sides<EnteredValue>>) {
        let Says { row, scores } = named.says.expect("the entry says a result");
        (row.number, row.result, scores)
    }

    fn found(named: &Named) -> Vec<usize> {
        named.found.iter().map(|row| row.number).collect()
    }

    #[test]
    fn a_name_says_the_win_of_its_side() {
        let open = [
            open_match(1),
            MatchRow {
                a: team(&["Carol"]),
                b: team(&["Dave"]),
                ..open_match(2)
            },
        ];
        let named = read_named(&open, "dav", true);
        assert_eq!(found(&named), [2]);
        assert_eq!(says(named), (2, Some(MatchResult::BWins), vec![]));
    }

    #[test]
    fn the_scores_of_a_name_are_its_sides_first() {
        let named = read_named(&[open_match(1)], "bob 21-17 15-21", true);
        assert_eq!(
            says(named),
            (
                1,
                Some(MatchResult::BWins),
                vec![count(17, 21), count(21, 15)]
            )
        );
    }

    #[test]
    fn a_number_or_x_ends_a_name_only_if_no_name_has_it() {
        let open = [MatchRow {
            a: team(&["Malcolm X"]),
            ..open_match(1)
        }];
        assert_eq!(
            says(read_named(&open, "Malcolm X", true)).1,
            Some(MatchResult::AWins)
        );
        assert_eq!(
            says(read_named(&open, "Malcolm X X", true)).1,
            Some(MatchResult::Draw)
        );
        assert_eq!(
            says(read_named(&open, "malcolm 1", true)).1,
            Some(MatchResult::AWins)
        );
    }

    #[test]
    fn a_name_in_several_matches_needs_a_number() {
        let open = [open_match(1), open_match(2)];
        let named = read_named(&open, "ali", true);
        assert_eq!(found(&named), [1, 2]);
        assert!(matches!(
            named.says,
            Err(ReportEnteredError::SeveralMatches {
                matches: 2,
                number: 1,
                ..
            })
        ));
        assert_eq!(says(read_named(&open, "ali 2", true)).0, 2);
        assert!(matches!(
            read_named(&open, "ali 3", true).says,
            Err(ReportEnteredError::NotInMatch { number: 3, .. })
        ));
    }

    #[test]
    fn a_name_of_both_sides_names_no_winner() {
        let open = [MatchRow {
            a: team(&["Ann"]),
            b: team(&["Andy"]),
            ..open_match(1)
        }];
        let both = |entry| {
            matches!(
                read_named(&open, entry, true).says,
                Err(ReportEnteredError::BothSides { number: 1, .. })
            )
        };
        assert!(both("an"));
        // a draw needs no winner, but its scores need a side
        assert_eq!(
            says(read_named(&open, "an x", true)).1,
            Some(MatchResult::Draw)
        );
        assert!(both("an x 1-1"));
    }

    #[test]
    fn a_draw_is_refused_where_there_are_none() {
        let named = read_named(&[open_match(1)], "ali x", false);
        assert_eq!(found(&named), [1]);
        assert!(matches!(
            named.says,
            Err(ReportEnteredError::Report(ReportError::DrawNotAllowed))
        ));
    }

    #[test]
    fn a_name_no_open_match_has_is_not_found() {
        let named = read_named(&[open_match(1)], "carol", true);
        assert!(named.found.is_empty());
        assert!(
            matches!(named.says, Err(ReportEnteredError::NoNameFound(name)) if name == "carol")
        );
    }
}
