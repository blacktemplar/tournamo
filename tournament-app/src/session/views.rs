//! What the screens show of the competition.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    num::NonZeroU32,
};

use plugin_factory::catalog::{AnyAxis, PluginEntry};
use tournament_engine::{
    Annotated as _, Bye, EntrantId, Match, Phase, PhaseData, Ranking, RankingId, TypeMap,
    plugins::{
        match_format::Parts,
        metrics_calculator::{Cell, MetricsCalculator},
        ranker::SliceRanker,
    },
};

use super::Session;
use crate::{
    storage::Progress,
    view::{
        self, ByeRow, Column, Entered, EntrantRow, ListedMatch, MatchRow, Medal, Round,
        RoundSummary, RoundsView, Shown, StageName, StandingCell, StandingRow, Standings,
        TypedEntry,
    },
};

impl Session {
    /// How far the competition is, as its [`Listing`] keeps it.
    ///
    /// [`Listing`]: crate::storage::Listing
    pub fn progress(&self) -> Progress {
        if !self.competition().is_started() {
            return Progress::Setup {
                entrants: self.competition().entrants().len(),
            };
        }
        if let Some(summary) = self.rounds().into_iter().find(|summary| summary.open > 0) {
            return Progress::Playing {
                round: summary.round,
                open: summary.open,
            };
        }
        let winners = self.winners().unwrap_or_default();
        Progress::Won {
            winners: winners
                .into_iter()
                .map(|team| team.into_iter().map(|entrant| entrant.name).collect())
                .collect(),
        }
    }

    /// The teams ranked first once every match scheduled has a result, more than one if they are
    /// tied, by the names of their entrants, each team by its entrants; `None` before the start or
    /// while a match has no result.
    pub fn winners(&self) -> Option<Vec<Vec<EntrantRow>>> {
        let phase = self.started_phase()?;
        let data = phase.data();
        if data
            .matches()
            .values()
            .any(|r#match| r#match.result().is_none())
        {
            return None;
        }
        let standings = phase.standings();
        let mut winners: Vec<_> = standings
            .iter()
            .filter(|standing| standing.rank() == NonZeroU32::MIN)
            .map(|standing| self.team(data, &[standing.ranking().id()]))
            .collect();
        // tied teams are in no order of their own
        winners.sort_by_cached_key(|team| {
            team.iter()
                .map(|entrant| entrant.name.as_deref().map(str::to_lowercase))
                .collect::<Vec<_>>()
        });
        Some(winners)
    }

    /// The rounds the phase has scheduled matches or byes in, in order; none before the start.
    pub fn rounds(&self) -> Vec<RoundSummary> {
        let Some(phase) = self.started_phase() else {
            return Vec::new();
        };
        let data = phase.data();
        let empty = |round| RoundSummary {
            round,
            matches: 0,
            open: 0,
            stage: None,
        };
        let mut rounds = BTreeMap::new();
        // the stage of each round, until a match or a bye of another one makes it `None`
        let mut stages: BTreeMap<NonZeroU32, Option<StageName>> = BTreeMap::new();
        let mut note_stage = |round, annotations| {
            let stage = stage(phase, annotations);
            stages
                .entry(round)
                .and_modify(|kept| {
                    if *kept != stage {
                        *kept = None;
                    }
                })
                .or_insert(stage);
        };
        for r#match in data.matches().values() {
            let round = r#match.round();
            let summary = rounds.entry(round).or_insert_with(|| empty(round));
            summary.matches += 1;
            if !r#match.is_finished() {
                summary.open += 1;
            }
            note_stage(round, r#match.annotations());
        }
        for bye in data.byes().values() {
            let round = bye.round();
            rounds.entry(round).or_insert_with(|| empty(round));
            note_stage(round, bye.annotations());
        }
        rounds
            .into_values()
            .map(|summary| RoundSummary {
                stage: stages.remove(&summary.round).flatten(),
                ..summary
            })
            .collect()
    }

    /// The matches and byes of `round`, or `None` if it has neither, e.g. before the start.
    pub fn round(&self, round: NonZeroU32) -> Option<Round> {
        let phase = self.started_phase()?;
        let data = phase.data();
        let mut matches: Vec<_> = data
            .matches()
            .values()
            .filter(|r#match| r#match.round() == round)
            .map(|r#match| self.match_row(phase, r#match))
            .collect();
        // a bracket numbers its matches before it schedules them, in another order
        matches.sort_by_key(|row| row.number);
        let byes_of_round: Vec<_> = data
            .byes()
            .values()
            .filter(|bye| bye.round() == round)
            .map(|bye| (bye, stage(phase, bye.annotations())))
            .collect();
        let stages: HashSet<_> = matches
            .iter()
            .map(|row| &row.stage)
            .chain(byes_of_round.iter().map(|(_, stage)| stage))
            .collect();
        let several_stages = stages.len() > 1;
        let byes: Vec<_> = byes_of_round
            .into_iter()
            .map(|(bye, stage)| ByeRow {
                entrants: self.team(data, &[bye.ranking()]),
                stage: stage.filter(|_| several_stages),
            })
            .collect();
        (!matches.is_empty() || !byes.is_empty()).then_some(Round {
            round,
            matches,
            byes,
            draws: phase.format().allows_draws(),
        })
    }

    /// The list of rounds showing what the operator `chose`, or the current round, or else the
    /// matches a result typed in `entry` fits, see [`Self::read_entered`]: nothing typed, or only
    /// spaces, shows the list chosen. `None` before the start.
    pub fn rounds_view(&self, chosen: Option<Shown>, entry: &str) -> Option<RoundsView> {
        let rounds = self.rounds();
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
        let entered = (!finished && !entry.is_empty()).then(|| self.read_entered(entry));
        let playable = self.playable();
        let playable_count = playable.len();
        let current_round = self.round(current);
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
                    self.round(round)
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

    /// The matches that can be played now, in the order they were scheduled: those without a
    /// result none of whose entrants still has to play a match of an earlier round. None before
    /// the start.
    pub fn playable(&self) -> Vec<MatchRow> {
        let Some(phase) = self.started_phase() else {
            return Vec::new();
        };
        let data = phase.data();
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
            .map(|r#match| self.match_row(phase, r#match))
            .collect()
    }

    /// The standings of the phase, with a column for every calculator that offers one; `None`
    /// before the start.
    pub fn standings(&self) -> Option<Standings> {
        let phase = self.started_phase()?;
        let calculators = AnyAxis::of::<dyn MetricsCalculator>();
        // the calculators each criterion ranks by, in the order of the criteria
        let ranked_by: Vec<&str> = phase
            .plugins()
            .ranking()
            .iter()
            .filter_map(|criterion| {
                AnyAxis::of::<dyn SliceRanker>().entry(plugin_factory::id_of(&**criterion))
            })
            .flat_map(|criterion| criterion.providers_in(calculators))
            .map(PluginEntry::id)
            .collect();
        let mut columns: Vec<_> = phase
            .plugins()
            .metrics()
            .iter()
            .filter_map(|calculator| {
                Some((plugin_factory::id_of(&**calculator), calculator.column()?))
            })
            .collect();
        // stable, so that the others stay in running order
        columns.sort_by_key(|(calculator, _)| {
            ranked_by
                .iter()
                .position(|ranked| ranked == calculator)
                .unwrap_or(usize::MAX)
        });
        let data = phase.data();
        let finished = data.matches().values().all(Match::is_finished);
        let standings = phase.standings();
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
            rows: standings
                .iter()
                .map(|standing| {
                    let ranking = standing.ranking();
                    StandingRow {
                        rank: standing.rank(),
                        entrants: self.team(data, &[ranking.id()]),
                        played: ranking.played_matches().len(),
                        cells: columns
                            .iter()
                            .map(|(_, column)| {
                                Some(standing_cell(phase, ranking, column.cell(ranking)?))
                            })
                            .collect(),
                        medal: finished.then(|| Medal::of(standing.rank())).flatten(),
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

    pub(super) fn team(&self, phase: &PhaseData, rankings: &[RankingId]) -> Vec<EntrantRow> {
        rankings
            .iter()
            .flat_map(|id| phase.rankings()[id].entrants())
            .map(|id| self.entrant_row(id))
            .collect()
    }

    /// The row of `r#match`, a match of `phase`.
    pub(super) fn match_row(&self, phase: &Phase, r#match: &Match) -> MatchRow {
        let data = phase.data();
        MatchRow {
            id: r#match.id(),
            number: number(r#match),
            round: r#match.round(),
            stage: stage(phase, r#match.annotations()),
            a: self.team(data, r#match.team_a()),
            b: self.team(data, r#match.team_b()),
            result: r#match.result(),
            scores: view::scores(phase, r#match),
            parts_won: view::parts_won(phase, r#match),
            details: details(phase, r#match),
        }
    }
}

/// Whether the match format of `r#match` enters more than who won, see [`MatchRow::details`].
fn details(phase: &Phase, r#match: &Match) -> bool {
    let root = phase.match_format().template(phase.data(), r#match).root;
    !root.fields.is_empty() || !matches!(root.parts, Parts::None)
}

/// The number of `r#match`, as [`MatchRow::number`] shows it.
pub(super) fn number(r#match: &Match) -> usize {
    usize::try_from(r#match.number().get()).expect("a number of a match fits a usize")
}

/// `cell` of `ranking` as the standings show it: a [`Cell::Round`] by the stage of the ranking's
/// match or bye in it, where the format names one.
fn standing_cell(phase: &Phase, ranking: &Ranking, cell: Cell) -> StandingCell {
    let Cell::Round(round) = cell else {
        return StandingCell::Metric(cell);
    };
    let data = phase.data();
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
        .and_then(|annotations| stage(phase, annotations))
    {
        Some(stage) => StandingCell::Stage(stage),
        None => StandingCell::Metric(cell),
    }
}

/// The stage the format of `phase` names a match or a bye with `annotations` by.
pub(super) fn stage(phase: &Phase, annotations: &TypeMap) -> Option<StageName> {
    let format = phase.format();
    Some(StageName {
        format: plugin_factory::id_of(format),
        stage: format.stage(annotations)?,
    })
}
