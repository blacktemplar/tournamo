//! The commands an operator types at the prompt, and how a line of input becomes one.

use std::{
    num::{NonZeroU32, NonZeroUsize},
    str::FromStr,
};

use clap::{Parser, ValueEnum};
use thiserror::Error;
use tournament_app::{people::Rating, result_form::read_score};
use tournament_engine::{EnteredValue, Sides};

/// A single command typed at the prompt.
///
/// The doc comment of every variant is the help text the prompt shows for it. This one is kept out
/// of the help.
#[derive(Debug, Clone, PartialEq, Parser)]
#[command(multicall = true, about = None, long_about = None)]
pub enum Command {
    /// List the entrants of the competition
    Entrants,
    /// Add one or more entrants, each given by the name it is shown and addressed by
    #[command(visible_alias = "add")]
    AddEntrant {
        #[arg(required = true)]
        names: Vec<String>,
        /// The rating of every entrant added, a number, higher is better: the start orders the
        /// registration by rating
        #[arg(long, value_parser = rating)]
        rating: Option<Rating>,
    },
    /// Rate an entrant, or remove its rating if none is given; the start orders the registration
    /// by rating, higher first, those of equal ratings and without one by lot
    Rate {
        name: String,
        /// A number, e.g. `1500`
        #[arg(value_parser = rating)]
        rating: Option<Rating>,
    },
    /// Remove an entrant from the competition
    #[command(visible_alias = "remove")]
    RemoveEntrant { name: String },
    /// Add a phase the competition is played in, configuring it at the prompt unless a format is
    /// given
    AddPhase {
        /// The format, by its label with hyphens for spaces (e.g. `round-robin`) or by its id;
        /// everything else is what the format recommends
        format: Option<String>,
        /// Answers to the fields of the format, e.g. `rounds=7`, or of the phase, e.g.
        /// `divisions=3` or `draw=in-blocks` (a plugin by its label with hyphens or its id); a
        /// field not answered here is answered as it offers
        #[arg(requires = "format", value_name = "FIELD=VALUE")]
        answers: Vec<String>,
        /// The name of the phase, e.g. "Group phase"
        #[arg(long)]
        name: Option<String>,
        /// Where the phase takes entrants from, in order: the places of `registration` or of the
        /// phase of a number, e.g. `1:1-8` or `1:9-` (to the last place) or `1` (every place);
        /// by default the whole registration for the first phase, else every place of the phase
        /// before
        #[arg(long, value_name = "SOURCE")]
        from: Vec<SourceArg>,
    },
    /// Change the configuration of a phase at the prompt, starting out at its current one; the
    /// results reported so far stay as long as the changed phase schedules their matches
    EditPhase {
        /// The number of the phase, by default the only running one
        #[arg(long)]
        phase: Option<NonZeroUsize>,
    },
    /// Start the competition, which schedules the first matches of the phases taking their
    /// entrants from the registration
    Start,
    /// Start a phase taking entrants from other phases, once those are finished
    StartPhase {
        /// The number of the phase, as `phases` lists it
        phase: NonZeroUsize,
    },
    /// List the phases, with where they take their entrants from and how far they are
    Phases,
    /// List the matches of a phase scheduled so far, with their number and result
    Matches {
        /// Only list the matches that have no result yet
        #[arg(long)]
        open: bool,
        /// Only list the matches of this round
        #[arg(long)]
        round: Option<NonZeroU32>,
        /// The number of the phase, by default the only running one
        #[arg(long)]
        phase: Option<NonZeroUsize>,
        /// Only list the matches of this division of the phase, by its letter, e.g. `b`
        #[arg(long, value_parser = division)]
        division: Option<usize>,
    },
    /// Report the result of a match
    Report {
        /// The number of the phase, by default the only running one
        #[arg(long)]
        phase: Option<NonZeroUsize>,
        /// The number of the match, as listed by `matches`
        number: NonZeroUsize,
        /// The result, `a`, `b` or `draw` (in any case, so that the sides can be named the way
        /// `matches` shows them); or the scores, e.g. `21-17 19-21 21-15` (or `21:17`, or `21–17`
        /// as `matches` shows them), followed by the result only where it is not the one they give
        #[arg(required = true, num_args = 1..)]
        entry: Vec<Reported>,
    },
    /// Undo the result reported for a match, which may be any reported match rather than only the
    /// one reported last
    UndoReport {
        /// The number of the phase, by default the only running one
        #[arg(long)]
        phase: Option<NonZeroUsize>,
        /// The number of the match, as listed by `matches`
        number: NonZeroUsize,
    },
    /// Show the current standings of a phase, or the ranking of the competition
    Ranking {
        /// The number of the phase, by default the only running one
        #[arg(long, conflicts_with = "competition")]
        phase: Option<NonZeroUsize>,
        /// Show the standings of this division of the phase, by its letter, e.g. `b`, instead of
        /// the overall standings
        #[arg(long, value_parser = division, conflicts_with = "competition")]
        division: Option<usize>,
        /// Show every entrant's place in the competition instead
        #[arg(long)]
        competition: bool,
    },
    /// Undo the last change
    Undo,
    /// Redo the last undone change
    Redo,
    /// Save the competition, under the name it was loaded or last saved under unless one is given
    Save { name: Option<String> },
    /// Exit; refuses if there are unsaved changes, unless forced
    #[command(visible_alias = "quit")]
    Exit {
        /// Exit even if there are unsaved changes, discarding them
        #[arg(long)]
        force: bool,
    },
}

/// The rating `text` says, see [`Rating::parse`].
fn rating(text: &str) -> Result<Rating, String> {
    Rating::parse(text).ok_or_else(|| format!("“{text}” is no rating; write a number, e.g. `1500`"))
}

/// The index of the division of the letter `letter`, as `--division` takes it: `a` (or `A`) for
/// the first, `z` for the 26th, `aa` for the 27th.
fn division(letter: &str) -> Result<usize, String> {
    let mut index = 0usize;
    if letter.is_empty() {
        return Err("a division is named by its letter, e.g. `b`".to_owned());
    }
    for char in letter.chars() {
        let char = char.to_ascii_uppercase();
        if !char.is_ascii_uppercase() {
            return Err("a division is named by its letter, e.g. `b`".to_owned());
        }
        index = index
            .checked_mul(26)
            .and_then(|index| index.checked_add(usize::from(char as u8 - b'A') + 1))
            .ok_or("no such division")?;
    }
    Ok(index - 1)
}

/// Where a phase takes entrants from, as `add-phase --from` takes it: the places of the
/// registration or of a phase by its number, e.g. `registration:1-8`, `1:9-` or `2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceArg {
    /// The number of the phase, `None` for the registration.
    pub phase: Option<NonZeroUsize>,
    pub from: NonZeroU32,
    /// To the last place if `None`.
    pub to: Option<NonZeroU32>,
}

impl FromStr for SourceArg {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let wrong = || {
            format!(
                "“{text}” names no places; write e.g. `registration:1-8`, `1:9-` (to the last) \
                 or `1` (every place of phase 1)"
            )
        };
        let (of, range) = text.split_once(':').unwrap_or((text, ""));
        let phase = if of.eq_ignore_ascii_case("registration") {
            None
        } else {
            Some(of.parse().map_err(|_| wrong())?)
        };
        let (from, to) = match range.split_once('-') {
            _ if range.is_empty() => (NonZeroU32::MIN, None),
            Some((from, "")) => (from.parse().map_err(|_| wrong())?, None),
            Some((from, to)) => (
                from.parse().map_err(|_| wrong())?,
                Some(to.parse().map_err(|_| wrong())?),
            ),
            None => {
                let place = range.parse().map_err(|_| wrong())?;
                (place, Some(place))
            }
        };
        Ok(Self { phase, from, to })
    }
}

/// What an operator reports of a match: its result, or a score.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reported {
    Result(ReportedResult),
    /// The score of each side, the first team's first, as [`read_score`] reads it: of the match,
    /// or of its next part, e.g. a game.
    Score(Sides<EnteredValue>),
}

impl FromStr for Reported {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if let Ok(result) = ReportedResult::from_str(text, true) {
            return Ok(Self::Result(result));
        }
        read_score(text).map(Self::Score).ok_or_else(|| {
            format!("“{text}” is no result (a, b or draw) and no score (e.g. 21-17)")
        })
    }
}

/// The result of a match, as reported by an operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ReportedResult {
    /// The first team won
    A,
    /// The second team won
    B,
    /// The match ended in a draw
    Draw,
}

/// Why a line of input is not a command.
#[derive(Debug, Error)]
pub enum ParseError {
    #[error("the line has an unclosed quote or a trailing backslash")]
    Unquote,
    /// Covers both a malformed command and an explicit request for help, which clap also reports
    /// as an error. [`clap::Error::print`] shows either of them correctly.
    #[error(transparent)]
    Clap(#[from] clap::Error),
}

impl Command {
    /// Parses a line typed at the prompt, splitting it into words the way a POSIX shell would, so
    /// that a name containing a space can be quoted.
    ///
    /// Returns `None` for a line without any words, which is no command but not an error either.
    ///
    /// # Errors
    /// Returns an error if the line cannot be split into words or the words are not a valid
    /// command, which includes a request for help.
    ///
    /// # Examples
    /// ```
    /// use tournament_cli::command::Command;
    ///
    /// assert_eq!(
    ///     Command::parse_line(r#"add-entrant "Ada Lovelace""#)?,
    ///     Some(Command::AddEntrant {
    ///         names: vec!["Ada Lovelace".to_owned()],
    ///         rating: None,
    ///     })
    /// );
    /// assert_eq!(Command::parse_line("   ")?, None);
    /// # Ok::<(), tournament_cli::command::ParseError>(())
    /// ```
    pub fn parse_line(line: &str) -> Result<Option<Self>, ParseError> {
        let words = shlex::split(line).ok_or(ParseError::Unquote)?;
        if words.is_empty() {
            return Ok(None);
        }
        Ok(Some(Self::try_parse_from(words)?))
    }
}

#[cfg(test)]
mod tests {
    use clap::{CommandFactory, error::ErrorKind};
    use tournament_engine::Score;
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::*;

    /// The score of `a` to `b`.
    fn counts(a: u16, b: u16) -> Reported {
        Reported::Score(Sides::new(EnteredValue::Count(a), EnteredValue::Count(b)))
    }

    #[track_caller]
    fn parse(line: &str) -> Command {
        Command::parse_line(line)
            .expect("line must parse")
            .expect("line must hold a command")
    }

    #[test]
    fn the_command_definition_is_consistent() {
        Command::command().debug_assert();
    }

    #[test]
    fn several_entrants_can_be_added_at_once() {
        assert_eq!(
            parse("add Alice 'Bob B.'"),
            Command::AddEntrant {
                names: vec!["Alice".to_owned(), "Bob B.".to_owned()],
                rating: None,
            }
        );
    }

    #[test]
    fn an_entrant_is_added_and_rated_with_a_number() {
        assert_eq!(
            parse("add Alice --rating 1500"),
            Command::AddEntrant {
                names: vec!["Alice".to_owned()],
                rating: Rating::parse("1500"),
            }
        );
        assert_eq!(
            parse("rate Alice 7,5"),
            Command::Rate {
                name: "Alice".to_owned(),
                rating: Rating::parse("7.5"),
            }
        );
        assert_eq!(
            parse("rate Alice"),
            Command::Rate {
                name: "Alice".to_owned(),
                rating: None,
            }
        );
        assert!(Command::parse_line("rate Alice strong").is_err());
    }

    #[test]
    fn adding_no_entrant_is_rejected() {
        assert!(Command::parse_line("add-entrant").is_err());
    }

    #[test]
    fn a_format_takes_answers_to_its_fields() {
        assert_eq!(
            parse("add-phase random-rounds rounds=5"),
            Command::AddPhase {
                format: Some("random-rounds".to_owned()),
                answers: vec!["rounds=5".to_owned()],
                name: None,
                from: Vec::new(),
            }
        );
    }

    /// A phase is configured at the prompt rather than given by its format, which is what the
    /// absent format stands for.
    #[test]
    fn add_phase_takes_no_format_at_all() {
        assert_eq!(
            parse("add-phase"),
            Command::AddPhase {
                format: None,
                answers: Vec::new(),
                name: None,
                from: Vec::new(),
            }
        );
    }

    #[test]
    fn matches_can_be_narrowed_down_by_both_filters() {
        assert_eq!(
            parse("matches --open --round 2"),
            Command::Matches {
                open: true,
                round: NonZeroU32::new(2),
                phase: None,
                division: None,
            }
        );
        assert_eq!(
            parse("matches"),
            Command::Matches {
                open: false,
                round: None,
                phase: None,
                division: None,
            }
        );
    }

    #[test]
    fn a_division_is_named_by_its_letter() {
        assert_eq!(
            parse("matches --division b"),
            Command::Matches {
                open: false,
                round: None,
                phase: None,
                division: Some(1),
            }
        );
        assert_eq!(
            parse("ranking --division AA"),
            Command::Ranking {
                phase: None,
                division: Some(26),
                competition: false,
            }
        );
        assert!(Command::parse_line("ranking --division 2").is_err());
    }

    #[test]
    fn rounds_start_at_one() {
        assert!(Command::parse_line("matches --round 0").is_err());
    }

    #[test]
    fn a_result_is_reported_by_match_number() {
        assert_eq!(
            parse("report 3 draw"),
            Command::Report {
                phase: None,
                number: NonZeroUsize::new(3).unwrap(),
                entry: vec![Reported::Result(ReportedResult::Draw)]
            }
        );
    }

    #[test]
    fn score_counts_fit_in_u16() {
        assert_eq!(Reported::from_str("65535-0").unwrap(), counts(u16::MAX, 0));
        assert!(Reported::from_str("65536-0").is_err());
        assert!(Reported::from_str("4294967295-0").is_err());
    }

    /// An en dash joins the numbers of a score as `matches` shows them, so that one shown can be
    /// pasted.
    #[test]
    fn scores_are_reported_with_a_hyphen_a_colon_or_an_en_dash_and_a_result_after_them() {
        assert_eq!(
            parse("report 2 21-17 19:21 21–15 a"),
            Command::Report {
                phase: None,
                number: NonZeroUsize::new(2).unwrap(),
                entry: vec![
                    counts(21, 17),
                    counts(19, 21),
                    counts(21, 15),
                    Reported::Result(ReportedResult::A),
                ]
            }
        );
    }

    #[test]
    fn a_score_may_be_a_decimal_with_a_point_or_a_comma() {
        let decimal = EnteredValue::Decimal(Score::new(9.5));

        for score in ["9.5-10", "9,5–10"] {
            assert_eq!(
                Reported::from_str(score).unwrap(),
                Reported::Score(Sides::new(decimal, EnteredValue::Count(10))),
                "{score}"
            );
        }
    }

    #[test]
    fn a_result_can_be_reported_in_the_case_the_sides_are_shown_in() {
        assert_eq!(
            parse("report 1 A"),
            Command::Report {
                phase: None,
                number: NonZeroUsize::new(1).unwrap(),
                entry: vec![Reported::Result(ReportedResult::A)]
            }
        );
        assert_eq!(
            parse("report 1 B"),
            Command::Report {
                phase: None,
                number: NonZeroUsize::new(1).unwrap(),
                entry: vec![Reported::Result(ReportedResult::B)]
            }
        );
        assert_eq!(
            parse("report 1 Draw"),
            Command::Report {
                phase: None,
                number: NonZeroUsize::new(1).unwrap(),
                entry: vec![Reported::Result(ReportedResult::Draw)]
            }
        );
    }

    #[test]
    fn match_numbers_start_at_one() {
        assert!(Command::parse_line("report 0 a").is_err());
        assert!(Command::parse_line("undo-report 0").is_err());
    }

    #[test]
    fn a_reported_result_is_undone_by_match_number() {
        assert_eq!(
            parse("undo-report 3"),
            Command::UndoReport {
                phase: None,
                number: NonZeroUsize::new(3).unwrap()
            }
        );
    }

    #[test]
    fn a_phase_takes_entrants_from_places_of_the_registration_or_of_phases() {
        let place = |place| NonZeroU32::new(place).unwrap();
        let phase = NonZeroUsize::new(1);

        assert_eq!(
            parse("add-phase single-elimination --name Final --from 1:1-8 --from registration:9-"),
            Command::AddPhase {
                format: Some("single-elimination".to_owned()),
                answers: Vec::new(),
                name: Some("Final".to_owned()),
                from: vec![
                    SourceArg {
                        phase,
                        from: place(1),
                        to: Some(place(8)),
                    },
                    SourceArg {
                        phase: None,
                        from: place(9),
                        to: None,
                    },
                ],
            }
        );
        assert_eq!(
            SourceArg::from_str("1").unwrap(),
            SourceArg {
                phase,
                from: place(1),
                to: None,
            }
        );
        assert!(SourceArg::from_str("one:1-8").is_err());
        assert!(SourceArg::from_str("1:0-8").is_err());
    }

    #[test]
    fn a_result_is_reported_in_the_phase_named() {
        assert_eq!(
            parse("report --phase 2 3 a"),
            Command::Report {
                phase: NonZeroUsize::new(2),
                number: NonZeroUsize::new(3).unwrap(),
                entry: vec![Reported::Result(ReportedResult::A)]
            }
        );
        assert_eq!(
            parse("ranking --competition"),
            Command::Ranking {
                phase: None,
                division: None,
                competition: true
            }
        );
        assert!(Command::parse_line("ranking --competition --phase 1").is_err());
    }

    #[test]
    fn help_is_reported_as_a_clap_error_to_be_printed() {
        let Err(ParseError::Clap(error)) = Command::parse_line("help") else {
            panic!("help must be reported through clap");
        };

        assert_eq!(error.kind(), ErrorKind::DisplayHelp);
    }

    #[test]
    fn an_unclosed_quote_is_rejected() {
        assert!(matches!(
            Command::parse_line("add 'Alice"),
            Err(ParseError::Unquote)
        ));
    }
}
