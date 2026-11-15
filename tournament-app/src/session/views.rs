//! What the screens show of the competition.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    num::NonZeroU32,
};

use plugin_factory::catalog::{AnyAxis, PluginEntry};
use tournament_engine::{
    Annotated as _, Bye, Division, DivisionData, EntrantId, Match, PhaseId, Ranking, RankingId,
    Standing, TypeMap,
    plugins::{
        draw::DivisionOrder,
        match_format::Parts,
        metrics_calculator::{Cell, MetricsCalculator},
        ranker::SliceRanker,
    },
};

use super::Session;
use crate::{
    storage::Progress,
    view::{
        self, ByeRow, Column, DivisionRef, Entered, EntrantRow, ListedMatch, MatchRow, MatchesOf,
        Medal, Round, RoundSummary, RoundsView, Shown, StageName, StandingCell, StandingRow,
        Standings, TypedEntry,
    },
};

impl Session {
    /// How far the competition is, as its [`Listing`] keeps it: playing while a started phase has
    /// a match without a result, the first such phase; won once the ranking is final.
    ///
    /// [`Listing`]: crate::storage::Listing
    pub fn progress(&self) -> Progress {
        if !self.competition().is_started() {
            return Progress::Setup {
                entrants: self.competition().entrants().len(),
            };
        }
        let open = self.competition().phases().values().find_map(|phase| {
            self.rounds(MatchesOf::phase(phase.id()))
                .into_iter()
                .find(|summary| summary.open > 0)
        });
        if let Some(summary) = open {
            return Progress::Playing {
                round: summary.round,
                open: summary.open,
            };
        }
        let Some(winners) = self.competition_winners() else {
            return Progress::Finished;
        };
        Progress::Won {
            winners: winners
                .into_iter()
                .map(|team| team.into_iter().map(|entrant| entrant.name).collect())
                .collect(),
        }
    }

    /// The teams ranked first in the division `division` once every match it scheduled has a
    /// result, more than one if they are tied, by the names of their entrants, each team by its
    /// entrants; `None` before its phase starts or while a match has no result.
    pub fn winners(&self, division: DivisionRef) -> Option<Vec<Vec<EntrantRow>>> {
        let division = self.division(division)?;
        let data = division.data();
        if data
            .matches()
            .values()
            .any(|r#match| r#match.result().is_none())
        {
            return None;
        }
        let standings = division.standings();
        Some(by_names(
            standings
                .iter()
                .filter(|standing| standing.rank() == NonZeroU32::MIN)
                .map(|standing| self.team(data, &[standing.ranking().id()]))
                .collect(),
        ))
    }

    /// The teams placed first in the competition once its ranking is final (see
    /// [`Self::ranking`]), more than one if they are tied, by the names of their entrants;
    /// `None` before.
    pub fn competition_winners(&self) -> Option<Vec<Vec<EntrantRow>>> {
        let ranking = self.ranking();
        ranking.is_final.then(|| {
            by_names(
                ranking
                    .rows
                    .into_iter()
                    .filter(|row| row.place == NonZeroU32::MIN)
                    .map(|row| row.entrants)
                    .collect(),
            )
        })
    }

    /// The rounds the divisions of `of` have scheduled matches or byes in, in order, the same
    /// round of every division as one; none before the start of the phase.
    pub fn rounds(&self, of: MatchesOf) -> Vec<RoundSummary> {
        let divisions = self.divisions_of(of);
        let empty = |round| RoundSummary {
            round,
            matches: 0,
            open: 0,
            stage: None,
        };
        let mut rounds = BTreeMap::new();
        // the stage of each round, until a match or a bye of another one makes it `None`
        let mut stages: BTreeMap<NonZeroU32, Option<StageName>> = BTreeMap::new();
        let mut note_stage = |round, stage| {
            stages
                .entry(round)
                .and_modify(|kept| {
                    if *kept != stage {
                        *kept = None;
                    }
                })
                .or_insert(stage);
        };
        for (_, division) in divisions {
            let data = division.data();
            for r#match in data.matches().values() {
                let round = r#match.round();
                let summary = rounds.entry(round).or_insert_with(|| empty(round));
                summary.matches += 1;
                if !r#match.is_finished() {
                    summary.open += 1;
                }
                note_stage(round, stage(division, r#match.annotations()));
            }
            for bye in data.byes().values() {
                let round = bye.round();
                rounds.entry(round).or_insert_with(|| empty(round));
                note_stage(round, stage(division, bye.annotations()));
            }
        }
        rounds
            .into_values()
            .map(|summary| RoundSummary {
                stage: stages.remove(&summary.round).flatten(),
                ..summary
            })
            .collect()
    }

    /// The matches and byes of `round` of the divisions of `of`, or `None` if they have neither,
    /// e.g. before the start of the phase.
    pub fn round(&self, of: MatchesOf, round: NonZeroU32) -> Option<Round> {
        let divisions = self.divisions_of(of);
        let mut matches: Vec<_> = divisions
            .iter()
            .flat_map(|&(index, division)| {
                division
                    .data()
                    .matches()
                    .values()
                    .filter(|r#match| r#match.round() == round)
                    .map(move |r#match| self.match_row(of.phase, index, division, r#match))
            })
            .collect();
        // a bracket numbers its matches before it schedules them, in another order
        matches.sort_by_key(|row| row.number);
        let byes_of_round: Vec<_> = divisions
            .iter()
            .flat_map(|&(_, division)| {
                division
                    .data()
                    .byes()
                    .values()
                    .filter(|bye| bye.round() == round)
                    .map(move |bye| {
                        (
                            self.team(division.data(), &[bye.ranking()]),
                            stage(division, bye.annotations()),
                        )
                    })
            })
            .collect();
        let stages: HashSet<_> = matches
            .iter()
            .map(|row| &row.stage)
            .chain(byes_of_round.iter().map(|(_, stage)| stage))
            .collect();
        let several_stages = stages.len() > 1;
        let byes: Vec<_> = byes_of_round
            .into_iter()
            .map(|(entrants, stage)| ByeRow {
                entrants,
                stage: stage.filter(|_| several_stages),
            })
            .collect();
        (!matches.is_empty() || !byes.is_empty()).then_some(Round {
            round,
            matches,
            byes,
            draws: divisions
                .iter()
                .any(|(_, division)| division.format().allows_draws()),
        })
    }

    /// The list of rounds of the matches of `of` showing what the operator `chose`, or the current
    /// round, or else the matches a result typed in `entry` fits, see [`Self::read_entered`]:
    /// nothing typed, or only spaces, shows the list chosen. `None` before the start of the
    /// phase.
    pub fn rounds_view(
        &self,
        of: MatchesOf,
        chosen: Option<Shown>,
        entry: &str,
    ) -> Option<RoundsView> {
        let rounds = self.rounds(of);
        let current = rounds
            .iter()
            .find(|summary| summary.open > 0)
            .or(rounds.last())?
            .round;
        let position = |round| rounds.iter().position(|summary| summary.round == round);
        // a round that no longer exists, e.g. after an undo, falls back to the current one
        let shown = chosen
            .filter(|shown| match shown {
                Shown::Playable => true,
                Shown::Round(round) => position(*round).is_some(),
            })
            .unwrap_or(Shown::Round(current));
        let (previous, next) = match shown {
            Shown::Playable => (None, None),
            Shown::Round(round) => {
                let at = |offset| {
                    position(round)
                        .and_then(|position| position.checked_add_signed(offset))
                        .and_then(|position| rounds.get(position))
                        .map(|summary| summary.round)
                };
                (at(-1), at(1))
            }
        };
        let finished = rounds.iter().all(|summary| summary.open == 0);
        let entry = entry.trim();
        let entered = (!finished && !entry.is_empty()).then(|| self.read_entered(of, entry));
        let playable = self.playable(of);
        let playable_count = playable.len();
        let current_round = self.round(of, current);
        let draws = current_round.as_ref().is_some_and(|round| round.draws);
        let scores = current_round
            .as_ref()
            .is_some_and(|round| round.matches.iter().any(|row| row.details));
        let (rows, byes, entry) = match (entered, shown) {
            (Some(Entered { found, report }), _) => (
                found,
                Vec::new(),
                Some(TypedEntry {
                    text: entry.to_owned(),
                    report,
                }),
            ),
            (None, Shown::Playable) => (playable, Vec::new(), None),
            (None, Shown::Round(round)) => {
                let round = if round == current {
                    current_round
                } else {
                    self.round(of, round)
                };
                let (matches, byes) = round
                    .map(|round| (round.matches, round.byes))
                    .unwrap_or_default();
                (matches, byes, None)
            }
        };
        let open: Vec<_> = rows
            .iter()
            .filter(|row| row.result.is_none())
            .map(|row| row.id)
            .collect();
        let next_open = |row: &MatchRow| {
            let position = open.iter().position(|open| *open == row.id)?;
            open[position + 1..]
                .iter()
                .chain(&open[..position])
                .next()
                .copied()
        };
        let with_round = shown == Shown::Playable || entry.is_some();
        let with_stage = rows.iter().any(|row| row.stage.is_some());
        let matches = rows
            .into_iter()
            .map(|row| ListedMatch {
                next_open: next_open(&row),
                row,
            })
            .collect();
        Some(RoundsView {
            rounds,
            current,
            shown,
            previous,
            next,
            playable: playable_count,
            draws,
            scores,
            finished,
            entry,
            matches,
            byes,
            with_round,
            with_stage,
        })
    }

    /// The matches of `of` that can be played now, in the order they were scheduled: those without
    /// a result none of whose entrants still has to play a match of an earlier round. None before
    /// the start of the phase.
    pub fn playable(&self, of: MatchesOf) -> Vec<MatchRow> {
        self.divisions_of(of)
            .into_iter()
            .flat_map(|(index, division)| self.playable_in(of.phase, index, division))
            .collect()
    }

    /// The matches of `division`, the division `index` of the phase `phase`, that can be played
    /// now, see [`Self::playable`].
    fn playable_in(&self, phase: PhaseId, index: usize, division: &Division) -> Vec<MatchRow> {
        let data = division.data();
        let open = || {
            data.matches()
                .values()
                .filter(|r#match| !r#match.is_finished())
        };
        let entrants = |r#match: &Match| -> Vec<EntrantId> {
            r#match
                .team_a()
                .iter()
                .chain(r#match.team_b())
                .flat_map(|ranking| data.rankings()[ranking].entrants())
                .copied()
                .collect()
        };
        // the earliest round each entrant still has to play a match in
        let mut next_round = HashMap::new();
        for r#match in open() {
            for entrant in entrants(r#match) {
                let round = next_round.entry(entrant).or_insert(r#match.round());
                *round = (*round).min(r#match.round());
            }
        }
        open()
            .filter(|r#match| {
                entrants(r#match)
                    .iter()
                    .all(|entrant| next_round[entrant] == r#match.round())
            })
            .map(|r#match| self.match_row(phase, index, division, r#match))
            .collect()
    }

    /// The overall standings of the phase `phase`, of all its divisions, each row with its overall
    /// rank and its division, see [`Phase::standings`](tournament_engine::Phase::standings); those
    /// of its division if it has one. `None` before the start of the phase.
    pub fn overall_standings(&self, phase: PhaseId) -> Option<Standings> {
        let of = self.phase(phase)?;
        let standings = of.standings();
        let rows = standings
            .iter()
            .map(|row| (row.division(), row.in_division(), row.rank()))
            .collect();
        self.standings_of(phase, rows, Columns::Ranking)
    }

    /// The standings of the division `of`, with a column for every calculator that offers one;
    /// `None` before the start of its phase.
    pub fn standings(&self, of: DivisionRef) -> Option<Standings> {
        let division = self.division(of)?;
        let standings = division.standings();
        let rows = standings
            .iter()
            .map(|&standing| (of.division, standing, standing.rank()))
            .collect();
        self.standings_of(of.phase, rows, Columns::Ranking)
    }

    /// For each place (unique rank) in the parallel divisions of the phase `phase` that a cut of
    /// a source of another phase goes through, the rows of that place in their divisions, by
    /// their overall standings, with the columns of the metrics the phase compares across
    /// divisions by: e.g. "the third places compared", where the best two of three go on. Empty
    /// for a phase of one division or of levels, and before its start.
    pub fn compared(&self, phase: PhaseId) -> Vec<view::Compared> {
        let Some(of) = self.phase(phase) else {
            return Vec::new();
        };
        if of.divisions().len() < 2 || of.plugins().draw().order() != DivisionOrder::Parallel {
            return Vec::new();
        }
        let advancing = self.advancing(phase);
        let standings = of.standings();
        let mut by_rank: BTreeMap<NonZeroU32, Vec<_>> = BTreeMap::new();
        for row in standings.iter() {
            by_rank
                .entry(row.in_division().unique_rank())
                .or_default()
                .push((row.division(), row.in_division(), row.rank()));
        }
        by_rank
            .into_iter()
            .filter(|(_, rows)| {
                let first = advancing(rows[0].1.ranking().id()).map(|advance| advance.to);
                rows.iter()
                    .any(|row| advancing(row.1.ranking().id()).map(|advance| advance.to) != first)
            })
            .filter_map(|(rank, rows)| {
                let standings = self.standings_of(phase, rows, Columns::Across)?;
                Some(view::Compared {
                    rank,
                    columns: standings.columns,
                    rows: standings.rows,
                })
            })
            .collect()
    }

    /// The standings of `rows` of the phase `phase`, each its division, its row in the division's
    /// standings and the rank shown, with the `columns` asked for; `None` before the start of the
    /// phase.
    fn standings_of(
        &self,
        phase: PhaseId,
        rows: Vec<(usize, Standing, NonZeroU32)>,
        columns: Columns,
    ) -> Option<Standings> {
        let of = self.phase(phase)?;
        let first = of.divisions().first()?;
        let advancing = self.advancing(phase);
        let first_place = self
            .competition()
            .first_place(&phase)
            .map_or(1, NonZeroU32::get);
        // a medal is one of the competition, by the overall rank
        let overall: HashMap<RankingId, NonZeroU32> = of
            .standings()
            .iter()
            .map(|row| (row.ranking().id(), row.rank()))
            .collect();
        let calculators = AnyAxis::of::<dyn MetricsCalculator>();
        // the calculators each criterion ranks by, in the order of the criteria
        let criteria = match columns {
            Columns::Ranking => first.plugins().ranking(),
            Columns::Across => first.plugins().across(),
        };
        let ranked_by: Vec<&str> = criteria
            .iter()
            .filter_map(|criterion| {
                AnyAxis::of::<dyn SliceRanker>().entry(plugin_factory::id_of(&**criterion))
            })
            .flat_map(|criterion| criterion.providers_in(calculators))
            .map(PluginEntry::id)
            .collect();
        let mut columns: Vec<_> = first
            .plugins()
            .metrics()
            .iter()
            .filter_map(|calculator| {
                let id = plugin_factory::id_of(&**calculator);
                (columns == Columns::Ranking || ranked_by.contains(&id))
                    .then(|| Some((id, calculator.column()?)))
                    .flatten()
            })
            .collect();
        // stable, so that the others stay in running order
        columns.sort_by_key(|(calculator, _)| {
            ranked_by
                .iter()
                .position(|ranked| ranked == calculator)
                .unwrap_or(usize::MAX)
        });
        let finished = of.is_finished();
        Some(Standings {
            columns: columns
                .iter()
                .map(|(calculator, column)| {
                    let entry = calculators.entry(calculator);
                    Column {
                        calculator,
                        header: column.header(),
                        label: entry.map_or(column.header(), PluginEntry::label),
                        description: entry.and_then(PluginEntry::description),
                        ranks: ranked_by.contains(calculator),
                    }
                })
                .collect(),
            rows: rows
                .into_iter()
                .map(|(index, standing, rank)| {
                    let division = &of.divisions()[index];
                    let ranking = standing.ranking();
                    let advances = advancing(ranking.id());
                    let place = overall
                        .get(&ranking.id())
                        .and_then(|overall| NonZeroU32::new(first_place + overall.get() - 1));
                    StandingRow {
                        rank,
                        division: index,
                        entrants: self.team(division.data(), &[ranking.id()]),
                        played: ranking.played_matches().len(),
                        cells: columns
                            .iter()
                            .map(|(_, column)| {
                                Some(standing_cell(division, ranking, column.cell(ranking)?))
                            })
                            .collect(),
                        medal: (finished && advances.is_none())
                            .then(|| place.and_then(Medal::of))
                            .flatten(),
                        advances,
                    }
                })
                .collect(),
        })
    }

    pub(super) fn entrant_row(&self, id: &EntrantId) -> EntrantRow {
        EntrantRow {
            id: *id,
            name: self.people.name(id).map(str::to_owned),
        }
    }

    pub(super) fn team(&self, division: &DivisionData, rankings: &[RankingId]) -> Vec<EntrantRow> {
        rankings
            .iter()
            .flat_map(|id| division.rankings()[id].entrants())
            .map(|id| self.entrant_row(id))
            .collect()
    }

    /// The row of `r#match`, a match of `division`, the division `index` of the phase `phase`.
    pub(super) fn match_row(
        &self,
        phase: PhaseId,
        index: usize,
        division: &Division,
        r#match: &Match,
    ) -> MatchRow {
        let data = division.data();
        let root = division.match_format().template(data, r#match).root;
        MatchRow {
            phase,
            division: index,
            id: r#match.id(),
            number: number(r#match),
            round: r#match.round(),
            stage: stage(division, r#match.annotations()),
            a: self.team(data, r#match.team_a()),
            b: self.team(data, r#match.team_b()),
            result: r#match.result(),
            scores: view::scores(&root, r#match),
            parts_won: view::parts_won(&root, r#match),
            details: !root.fields.is_empty() || !matches!(root.parts, Parts::None),
        }
    }
}

/// Which columns standings have: of every calculator offering one, the metrics of the ranking
/// first, or of the metrics the phase is compared across divisions by alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Columns {
    Ranking,
    Across,
}

/// `teams` in the order of the names of their entrants, which tied teams are in no order of their
/// own.
fn by_names(mut teams: Vec<Vec<EntrantRow>>) -> Vec<Vec<EntrantRow>> {
    teams.sort_by_cached_key(|team| {
        team.iter()
            .map(|entrant| entrant.name.as_deref().map(str::to_lowercase))
            .collect::<Vec<_>>()
    });
    teams
}

/// The number of `r#match`, as [`MatchRow::number`] shows it.
pub(super) fn number(r#match: &Match) -> usize {
    usize::try_from(r#match.number().get()).expect("a number of a match fits a usize")
}

/// `cell` of `ranking` as the standings show it: a [`Cell::Round`] by the stage of the ranking's
/// match or bye in it, where the format names one.
fn standing_cell(division: &Division, ranking: &Ranking, cell: Cell) -> StandingCell {
    let Cell::Round(round) = cell else {
        return StandingCell::Metric(cell);
    };
    let data = division.data();
    let matches = ranking
        .played_matches()
        .iter()
        .filter_map(|id| data.matches().get(id))
        .filter(|r#match| r#match.round() == round)
        .map(Match::annotations);
    let byes = data
        .byes()
        .values()
        .filter(|bye| bye.ranking() == ranking.id() && bye.round() == round)
        .map(Bye::annotations);
    match matches
        .chain(byes)
        .next()
        .and_then(|annotations| stage(division, annotations))
    {
        Some(stage) => StandingCell::Stage(stage),
        None => StandingCell::Metric(cell),
    }
}

/// The stage the format of `division` names a match or a bye with `annotations` by.
pub(super) fn stage(division: &Division, annotations: &TypeMap) -> Option<StageName> {
    let format = division.format();
    Some(StageName {
        format: plugin_factory::id_of(format),
        stage: format.stage(annotations)?,
    })
}
