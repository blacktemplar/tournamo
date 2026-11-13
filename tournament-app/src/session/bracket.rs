//! The bracket of a phase whose format plays one, e.g. a knockout.

use std::collections::{HashMap, HashSet};

use tournament_engine::{
    Phase, Sides, Team, TypeMap,
    plugins::phase_format::{
        Advance, BracketPairing as Pairing, BracketSide, DoubleEliminationRound, KnockoutRound,
        PairingId, Seat,
    },
};

use super::{Session, views::stage};
use crate::view::{
    Bracket, BracketColumn, BracketEntry, BracketLine, BracketPairing, BracketPart, BracketSection,
    BracketSource, Unplayed,
};

/// What a pairing is laid out with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Part(BracketPart),
    /// The stage below the last column of the section before, apart from it.
    Below,
}

/// Where a pairing is drawn: its section, its column in it, and its place in the column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Place {
    section: usize,
    column: usize,
    index: usize,
}

impl Session {
    /// The bracket of the phase, if its format plays one, e.g. a single or a double elimination;
    /// `None` before the start.
    pub fn bracket(&self) -> Option<Bracket> {
        let phase = self.started_phase()?;
        let bracket = phase.format().bracket()?;
        let pairings = bracket.pairings();
        let by_id: HashMap<PairingId, &Pairing> = pairings
            .iter()
            .map(|pairing| (pairing.id, pairing))
            .collect();

        // the sections in the order their first pairings were added, each with its columns by
        // stage, in the same order
        let mut layout: Vec<(Section, Vec<(_, Vec<PairingId>)>)> = Vec::new();
        let mut places = HashMap::new();
        for pairing in &pairings {
            let part = section(pairing.annotations);
            let section = layout
                .iter()
                .position(|(other, _)| *other == part)
                .unwrap_or_else(|| {
                    layout.push((part, Vec::new()));
                    layout.len() - 1
                });
            let columns = &mut layout[section].1;
            let stage = stage(phase, pairing.annotations);
            let column = columns
                .iter()
                .position(|(other, _)| *other == stage)
                .unwrap_or_else(|| {
                    columns.push((stage, Vec::new()));
                    columns.len() - 1
                });
            let ids = &mut columns[column].1;
            places.insert(
                pairing.id,
                Place {
                    section,
                    column,
                    index: ids.len(),
                },
            );
            ids.push(pairing.id);
        }

        let data = phase.data();
        // the sides only a bye can take
        let byes: HashMap<PairingId, Sides<bool>> = pairings
            .iter()
            .map(|pairing| (pairing.id, pairing.sides.map(|side| side.bye)))
            .collect();
        let only_byes = |id: &PairingId| byes[id].into_iter().all(|bye| bye);
        let a_bye = |id: &PairingId| byes[id].into_iter().any(|bye| bye);
        let unplayed: HashMap<PairingId, Unplayed> = pairings
            .iter()
            .filter_map(|pairing| {
                if only_byes(&pairing.id) {
                    return Some((pairing.id, Unplayed::Byes));
                }
                // waiting for a pairing whose result moved nobody on
                let decided_without_it =
                    pairing.sides.into_iter().any(|side| {
                        let Some(Advance::WinnerOf(from) | Advance::LoserOf(from)) = side.from
                        else {
                            return false;
                        };
                        let from = by_id[&from];
                        from.advances_only_if_won_by.is_some()
                            && from
                                .r#match
                                .is_some_and(|id| data.matches()[&id].result().is_some())
                    }) && pairing.sides.into_iter().all(|side| side.seat.is_none());
                decided_without_it.then_some((pairing.id, Unplayed::NotNeeded))
            })
            .collect();

        // the place each pairing's winner is drawn at next, if that is in the next column; none
        // from or to a pairing never played
        let mut next: HashMap<PairingId, Place> = HashMap::new();
        for pairing in &pairings {
            if unplayed.contains_key(&pairing.id) {
                continue;
            }
            let place = places[&pairing.id];
            for side in pairing.sides {
                if let Some(Advance::WinnerOf(from)) = side.from
                    && !unplayed.contains_key(&from)
                    && places[&from].section == place.section
                    && places[&from].column + 1 == place.column
                {
                    next.insert(from, place);
                }
            }
        }

        // Folding a column leaves out its pairings decided by a bye, so it can be folded where its
        // other pairings all lead to the next column, in whose rows they are drawn. Nothing may
        // lead into it, whose lines would expect its pairings in rows of their own.
        let by_bye = |id: &PairingId| by_id[id].r#match.is_none() && a_bye(id);
        let mut folded = HashSet::new();
        for (section, (_, columns)) in layout.iter().enumerate() {
            for (column, (_, ids)) in columns.iter().enumerate() {
                let led_into = next
                    .values()
                    .any(|to| to.section == section && to.column == column);
                if column + 1 < columns.len()
                    && !led_into
                    && ids.iter().any(by_bye)
                    && ids.iter().all(|id| by_bye(id) || next.contains_key(id))
                {
                    folded.insert((section, column));
                    next.retain(|from, _| !(ids.contains(from) && by_bye(from)));
                }
            }
        }
        let left_out = |id: &PairingId| {
            let place = places[id];
            folded.contains(&(place.section, place.column)) && by_bye(id)
        };
        // the place of each pairing drawn among those drawn in its column, from 0, and how many
        // those are
        let mut drawn: HashMap<PairingId, (usize, usize)> = HashMap::new();
        for (_, columns) in &layout {
            for (_, ids) in columns {
                let shown: Vec<_> = ids.iter().filter(|id| !left_out(id)).collect();
                for (index, id) in shown.iter().enumerate() {
                    drawn.insert(**id, (index, shown.len()));
                }
            }
        }

        let number = |pairing: &Pairing| {
            pairing.number.map(|number| {
                usize::try_from(number.get()).expect("a number of a match fits a usize")
            })
        };
        let source = |id: PairingId| {
            let pairing = by_id[&id];
            let (index, count) = drawn[&id];
            BracketSource {
                number: number(pairing),
                stage: stage(phase, pairing.annotations),
                position: (count > 1).then_some(index + 1),
            }
        };
        // who takes `side`, where the winner of a pairing against a bye is whoever plays the bye
        let entry = |mut side: BracketSide| loop {
            return match (side.seat, side.from) {
                (Some(Seat::Ranking(ranking)), _) => {
                    BracketEntry::Team(self.team(data, &[ranking]))
                }
                (Some(Seat::Bye), _) => BracketEntry::Bye,
                (None, Some(Advance::WinnerOf(from) | Advance::LoserOf(from)))
                    if only_byes(&from) =>
                {
                    BracketEntry::Bye
                }
                (None, Some(Advance::LoserOf(from))) if a_bye(&from) => BracketEntry::Bye,
                (None, Some(Advance::WinnerOf(from))) if a_bye(&from) => {
                    let playing = if byes[&from].a { Team::B } else { Team::A };
                    side = *by_id[&from].sides.of(playing);
                    continue;
                }
                (None, Some(Advance::WinnerOf(from))) => BracketEntry::WinnerOf(source(from)),
                (None, Some(Advance::LoserOf(from))) => BracketEntry::LoserOf(source(from)),
                (None, None) => unreachable!("a side the bracket was built with is taken"),
            };
        };
        let row = |pairing: &Pairing| {
            let sides = pairing.sides.map(entry);
            let place = places[&pairing.id];
            let after_bye = Sides::from_fn(|side| {
                let from_left_out = matches!(
                    pairing.sides.of(side).from,
                    Some(Advance::WinnerOf(from)) if left_out(&from)
                );
                from_left_out && *sides.of(side) != BracketEntry::Bye
            });
            let line_out = next.get(&pairing.id).map(|to| {
                // the places of the pairings whose winners meet in `to`, this one's included
                let joining: Vec<_> = next
                    .iter()
                    .filter(|(_, other)| *other == to)
                    .map(|(from, _)| places[from].index)
                    .collect();
                if joining.len() == 1 {
                    BracketLine::Straight
                } else if joining.iter().all(|other| place.index <= *other) {
                    BracketLine::Down
                } else {
                    BracketLine::Up
                }
            });
            let from = || pairing.sides.into_iter().filter_map(|side| side.from);
            let unplayed = unplayed.get(&pairing.id).copied();
            BracketPairing {
                number: number(pairing),
                r#match: pairing.r#match.map(|id| {
                    let r#match = data
                        .matches()
                        .get(&id)
                        .expect("the bracket only holds matches of its phase");
                    self.match_row(phase, r#match)
                }),
                row: if folded.contains(&(place.section, place.column)) {
                    next[&pairing.id].index
                } else {
                    place.index
                },
                sides: sides.into(),
                after_bye: after_bye.into(),
                line_out,
                line_in: next.values().any(|to| *to == place),
                // until it is scheduled or known not to be played
                only_if_needed: pairing.r#match.is_none()
                    && unplayed.is_none()
                    && from().any(|advance| {
                        let (Advance::WinnerOf(from) | Advance::LoserOf(from)) = advance;
                        by_id[&from].advances_only_if_won_by.is_some()
                    }),
                unplayed,
            }
        };
        let mut sections: Vec<BracketSection> = Vec::new();
        for (index, (section, columns)) in layout.iter().enumerate() {
            let mut columns = columns.iter().enumerate().map(|(column, (stage, ids))| {
                let rows = if folded.contains(&(index, column)) {
                    layout[index].1[column + 1].1.len()
                } else {
                    ids.len()
                };
                BracketColumn {
                    stage: stage.clone(),
                    rows,
                    pairings: ids
                        .iter()
                        .filter(|id| !left_out(id))
                        .map(|id| row(by_id[id]))
                        .collect(),
                    below: None,
                }
            });
            match section {
                // a column of byes only is left out whole, its stage being played by nobody
                Section::Part(part) => sections.push(BracketSection {
                    part: *part,
                    columns: columns
                        .filter(|column| !column.pairings.is_empty())
                        .collect(),
                }),
                Section::Below => {
                    let last = sections
                        .last_mut()
                        .and_then(|section| section.columns.last_mut())
                        .expect("a stage below another follows it");
                    last.below = columns.next().map(Box::new);
                }
            }
        }
        Some(Bracket { sections })
    }
}

/// Whether the format of `phase`, which has started, plays a bracket, see [`Session::bracket`].
pub(super) fn plays_bracket(phase: &Phase) -> bool {
    phase.format().bracket().is_some()
}

/// The section a pairing annotated with `annotations` is drawn in, by the stages of the knockouts
/// of the engine: the match for third place below the final, a double elimination by the bracket
/// of its stage. Any other is a knockout.
fn section(annotations: &TypeMap) -> Section {
    if let Some(round) = annotations.get::<DoubleEliminationRound>() {
        return Section::Part(match round {
            DoubleEliminationRound::Winners { .. } => BracketPart::Winners,
            DoubleEliminationRound::Losers { .. } => BracketPart::Losers,
            DoubleEliminationRound::GrandFinal | DoubleEliminationRound::GrandFinalReset => {
                BracketPart::GrandFinal
            }
        });
    }
    match annotations.get::<KnockoutRound>() {
        Some(KnockoutRound::ThirdPlace) => Section::Below,
        Some(KnockoutRound::RoundOf(_)) | None => Section::Part(BracketPart::Knockout),
    }
}
