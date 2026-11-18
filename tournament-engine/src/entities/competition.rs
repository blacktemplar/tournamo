use std::num::NonZeroU32;

use itertools::Itertools;
use plugin_factory::{compatibility::Declaration, draft::Draft};
use rand::{RngExt, seq::SliceRandom};
use serde::{Deserialize, Serialize};
use vec1::{Vec1, vec1};

use super::seed::Entropy;
#[cfg(doc)]
use crate::plugins::teams::Teams;
use crate::{
    Columns, Entrant, EntrantId, MatchId, Origin, Part, Phase, PhaseId, Qualifier, Ranking,
    RankingId, Seed, Source, SourceOf,
    map::{IdMap, Map},
    plugins::{PhasePlugins, teams::TeamsView},
};

/// A competition: its entrants, its phases, and the random seed everything random in it derives
/// from.
#[derive(Default, Serialize, Deserialize)]
pub struct Competition {
    entrants: IdMap<Entrant>,
    phases: IdMap<Phase>,
    started: bool,
    seed: Seed,
    /// The entrants by lot, best first; empty until the competition starts.
    lots: Vec<EntrantId>,
    /// The entrants in the order of the registration, best first; empty until the competition
    /// starts.
    registration: Vec<EntrantId>,
    /// The highest number of a match so far, or kept for the matches of a division, 0 for none.
    highest_number: u32,
}

/// Where an entrant is placed in the competition, see [`Competition::ranking`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Placed {
    pub entrant: EntrantId,
    /// One more than the rows placed before the entrant's, a team one row; shared by entrants
    /// placed equal.
    pub place: NonZeroU32,
    /// The latest started phase the entrant is in, which places them; for an entrant of the
    /// registration waiting for a phase not started yet, that phase.
    pub phase: PhaseId,
    /// The ranking of the entrant in that phase, `None` while it has not started.
    pub ranking: Option<RankingId>,
}

impl Competition {
    pub fn is_started(&self) -> bool {
        self.started
    }

    pub fn entrants(&self) -> &IdMap<Entrant> {
        &self.entrants
    }

    pub(crate) fn add_entrant(&mut self, entrant: Entrant) {
        self.entrants.insert(entrant);
    }

    pub(crate) fn remove_entrant(&mut self, entrant_id: &EntrantId) {
        self.entrants.remove(entrant_id);
    }

    pub(crate) fn set_seed(&mut self, seed: Seed) {
        self.seed = seed;
    }

    pub fn seed(&self) -> Seed {
        self.seed
    }

    /// Every entrant, ordered by lot, drawn when the competition starts; empty before. The lot
    /// breaks every tie nothing else breaks, see [`Ranking::lot`](crate::Ranking::lot).
    pub fn lots(&self) -> &[EntrantId] {
        &self.lots
    }

    /// The registration, what [`SourceOf::Registration`] takes places of: every entrant, in the
    /// order the start gave (see [`StartCompetition`](crate::StartCompetition)), its tiers in
    /// order, each by lot, then everyone else by lot. Empty before the start; without an order
    /// given it is [`Self::lots`].
    pub fn registration(&self) -> &[EntrantId] {
        &self.registration
    }

    /// The lot of `entrant`, i.e. their place in [`Self::lots`]; `None` before the start.
    pub fn lot(&self, entrant: &EntrantId) -> Option<NonZeroU32> {
        self.lots
            .iter()
            .position(|e| e == entrant)
            .map(|index| NonZeroU32::new(index as u32 + 1).expect("one more than an index"))
    }

    pub(crate) fn add_phase(
        &mut self,
        id: PhaseId,
        name: Option<String>,
        sources: Vec1<Source>,
        plugins: PhasePlugins,
        columns: Columns,
    ) {
        let seed = Seed::new(self.seed.rng(id).random());
        self.phases
            .insert(Phase::new(id, name, sources, plugins, columns, seed));
    }

    /// Declares the facts that hold for a phase of this competition taking its entrants from
    /// `sources` before any of its plugins declare theirs, i.e. the context of its
    /// [`PhasePlugins`] for a [`Draft`] and for [`plugin_factory::compatibility`], see
    /// [`PhasePlugins::draft`].
    pub(crate) fn declare_phase_context(&self, sources: &[Source], declaration: &mut Declaration) {
        PhasePlugins::declare_context(self.entrants_per_qualifier(sources), declaration);
    }

    /// A draft of the plugins of a phase of this competition taking its entrants from `sources`,
    /// with nothing answered yet: [`PhasePlugins::draft`] of how many entrants each of their
    /// qualifiers stands for ([`Self::entrants_per_qualifier`]). Building [`PhasePlugins`] from it
    /// is the way to make them, see their example.
    pub fn phase_draft(&self, sources: &[Source]) -> Draft<PhasePlugins> {
        PhasePlugins::draft(self.entrants_per_qualifier(sources))
    }

    /// How many entrants each qualifier of a field taken from `sources` stands for: one for the
    /// registration, for a phase as many as each of its rankings, see
    /// [`Self::entrants_per_ranking`]. `None` if the sources send qualifiers of different sizes,
    /// take places of a phase this competition does not have, or are none.
    pub fn entrants_per_qualifier(&self, sources: &[Source]) -> Option<NonZeroU32> {
        entrants_per_qualifier(sources, |phase| self.entrants_per_ranking(phase))
    }

    /// How many entrants each ranking of the phase `phase_id` stands for: what its teams make of
    /// the qualifiers coming in, see [`Teams::entrants_per_team`]. `None` if there is no such
    /// phase or its teams are not all of one size.
    pub fn entrants_per_ranking(&self, phase_id: &PhaseId) -> Option<NonZeroU32> {
        let phase = self.phase(phase_id)?;
        phase
            .plugins()
            .teams()
            .entrants_per_team(self.entrants_per_qualifier(phase.sources()))
    }

    /// The phases, in the order they were added in; a phase takes entrants only from phases
    /// before it.
    pub fn phases(&self) -> &IdMap<Phase> {
        &self.phases
    }

    pub fn phase(&self, phase_id: &PhaseId) -> Option<&Phase> {
        self.phases.get(phase_id)
    }

    /// The first phase of this competition, once one is added.
    pub fn only_phase(&self) -> Option<&Phase> {
        self.phases.values().next()
    }

    /// Draws the lots, orders the registration by the tiers of `order` (see
    /// [`Self::registration`]) and starts every phase fed by the registration alone.
    pub(crate) fn start(&mut self, order: &[Vec1<EntrantId>]) {
        let mut lots: Vec<_> = self.entrants().keys().copied().collect();
        // Note that keys() is not guaranteed to give a stable ordering. To make sure ordering is
        // stable we sort before shuffling
        lots.sort_by_key(|e| e.uuid());
        lots.shuffle(&mut self.seed.rng(Entropy::CompetitionStart));
        let lot_of: Map<EntrantId, usize> = lots.iter().copied().zip(0..).collect();
        // the tier of every entrant named by the order, the rest after the last tier
        let tier_of: Map<EntrantId, usize> = order
            .iter()
            .zip(0..)
            .flat_map(|(tier, index)| tier.iter().map(move |entrant| (*entrant, index)))
            .collect();
        let mut registration = lots.clone();
        registration.sort_by_key(|entrant| {
            (
                tier_of.get(entrant).copied().unwrap_or(order.len()),
                lot_of[entrant],
            )
        });
        self.lots = lots;
        self.registration = registration;
        self.started = true;
        let starting: Vec<_> = self
            .phases
            .values()
            .filter(|phase| phase.is_fed_by_registration_alone())
            .map(Phase::id)
            .collect();
        for phase_id in starting {
            self.start_phase(phase_id);
        }
    }

    /// Starts the phase `phase_id` with its teams, see [`Self::teams_of`].
    ///
    /// # Panics
    /// Panics if there is no such phase.
    pub(crate) fn start_phase(&mut self, phase_id: PhaseId) {
        let field = self
            .teams_of(&phase_id)
            .into_iter()
            .map(|qualifier| {
                let lot = qualifier
                    .entrants
                    .iter()
                    .map(|entrant| self.lot(entrant).expect("every entrant has a lot"))
                    .min()
                    .expect("a qualifier has entrants");
                (qualifier, lot)
            })
            .collect();
        self.phases
            .get_mut(&phase_id)
            .expect("the phase exists")
            .start(field, &self.entrants, &mut self.highest_number);
    }

    /// How many qualifiers the field of the phase `phase_id` has as things stand, see
    /// [`Self::field_of`], counting the whole registration before the start too. 0 if there is no
    /// such phase.
    pub fn field_len(&self, phase_id: &PhaseId) -> usize {
        let Some(phase) = self.phase(phase_id) else {
            return 0;
        };
        phase
            .sources()
            .iter()
            .map(|source| {
                let available = match source.of() {
                    SourceOf::Registration => self.entrants.len(),
                    SourceOf::Phase(from) => self
                        .phase(&from)
                        .map_or(0, |from| from.standings().iter().count()),
                };
                source.count(available)
            })
            .sum()
    }

    /// The field of the phase `phase_id` formed into its teams, what its draw deals, see
    /// [`Teams`]: each team a qualifier of its entrants, with the origins of the qualifiers of the
    /// field they come from. Empty if there is no such phase.
    pub fn teams_of(&self, phase_id: &PhaseId) -> Vec<Qualifier> {
        let Some(phase) = self.phase(phase_id) else {
            return Vec::new();
        };
        let field = self.field_of(phase_id);
        let teams = phase.plugins().teams();
        let formed = teams.form(&TeamsView::new(&field, &self.entrants, phase.seed()));
        let position_of: Map<EntrantId, usize> = field
            .iter()
            .enumerate()
            .flat_map(|(position, qualifier)| {
                qualifier
                    .entrants
                    .iter()
                    .map(move |entrant| (*entrant, position))
            })
            .collect();
        debug_assert!(
            formed
                .iter()
                .flatten()
                .map(|entrant| entrant.uuid())
                .sorted()
                .eq(position_of.keys().map(|entrant| entrant.uuid()).sorted())
                && self.entrants_per_ranking(phase_id).is_none_or(|size| {
                    formed.iter().all(|team| team.len() == size.get() as usize)
                }),
            "teams take every entrant of the field once, each of the size they stand for"
        );
        formed
            .into_iter()
            .map(|entrants| {
                let positions: Vec<_> = entrants
                    .iter()
                    .map(|entrant| position_of[entrant])
                    .sorted()
                    .dedup()
                    .collect();
                let origins = positions
                    .into_iter()
                    .flat_map(|position| field[position].origins.iter().copied())
                    .dedup()
                    .collect::<Vec<_>>();
                Qualifier {
                    entrants,
                    origins: Vec1::try_from_vec(origins).expect("an entrant comes from a row"),
                }
            })
            .collect()
    }

    /// The field of the phase `phase_id`, what it starts with as things stand: its sources in
    /// order, each the rows of its range best first, as many as there are. A phase that has not
    /// started has no rows. Empty if there is no such phase.
    pub fn field_of(&self, phase_id: &PhaseId) -> Vec<Qualifier> {
        let Some(phase) = self.phase(phase_id) else {
            return Vec::new();
        };
        let mut field = Vec::new();
        for source in phase.sources() {
            match source.of() {
                SourceOf::Registration => {
                    field.extend(
                        self.registration
                            .iter()
                            .zip(NonZeroU32::MIN..)
                            .filter(|&(_, place)| source.takes(place))
                            .map(|(entrant, place)| {
                                Qualifier::new(vec1![*entrant], Origin::Registration { place })
                            }),
                    );
                }
                SourceOf::Phase(from) => {
                    let Some(from) = self.phase(&from) else {
                        continue;
                    };
                    field.extend(
                        from.standings()
                            .iter()
                            .filter(|standing| source.takes(standing.unique_rank()))
                            .map(|standing| {
                                Qualifier::new(
                                    standing.ranking().entrants().clone(),
                                    Origin::Phase {
                                        phase: from.id(),
                                        division: standing.division(),
                                        rank: standing.in_division().rank(),
                                        unique_rank: standing.in_division().unique_rank(),
                                        overall_rank: standing.rank(),
                                        overall_unique_rank: standing.unique_rank(),
                                    },
                                )
                            }),
                    );
                }
            }
        }
        field
    }

    /// Finishes the match `match_id` of the phase `phase_id` with what was `entered` for it.
    ///
    /// # Panics
    /// Panics if there is no such phase, or as [`Phase::report_match`] does.
    pub(crate) fn report_match(&mut self, phase_id: PhaseId, match_id: MatchId, entered: Part) {
        self.phases
            .get_mut(&phase_id)
            .expect("the phase exists")
            .report_match(match_id, entered, &mut self.highest_number);
    }

    /// The slot the overall standings of the phase `phase_id` start at, see [`Self::slots`];
    /// `None` if there is no such phase.
    fn first_slot(&self, phase_id: &PhaseId) -> Option<NonZeroU32> {
        let phase = self.phase(phase_id)?;
        phase
            .sources()
            .iter()
            .map(|source| match source.of() {
                SourceOf::Registration => Some(source.from()),
                SourceOf::Phase(from) => self
                    .first_slot(&from)?
                    .checked_add(self.entrants_before(&from, source.from())),
            })
            .min()
            .flatten()
    }

    /// The number of entrants in the rows of the overall standings of the phase `phase_id` before
    /// the row of `unique_rank`; each row the phase does not have (yet) counts as many as a
    /// ranking of it stands for, see [`Self::entrants_per_ranking`], or else as one.
    fn entrants_before(&self, phase_id: &PhaseId, unique_rank: NonZeroU32) -> u32 {
        let rows = unique_rank.get() - 1;
        let Some(phase) = self.phase(phase_id) else {
            return rows;
        };
        let standings = phase.standings();
        let known = standings.iter().take(rows as usize);
        let missing = rows - known.len() as u32;
        let entrants: u32 = known
            .map(|standing| standing.ranking().entrants().len() as u32)
            .sum();
        let per_ranking = self
            .entrants_per_ranking(phase_id)
            .map_or(1, NonZeroU32::get);
        entrants + missing * per_ranking
    }

    /// The ranking, slot, unique slot and taker (the phase taking it on, if any) of each row of
    /// the overall standings of the phase `phase_id`, best first. Slots count entrants, so that
    /// rows of phases of teams of different sizes compare: a row's slot is the slot its phase
    /// starts at plus the entrants of the rows before it, where a cut goes through rows sharing a
    /// rank those before the first row of its side of the cut, its unique slot those before it.
    fn slots(&self, phase_id: &PhaseId) -> Vec<(&Ranking, u32, u32, Option<PhaseId>)> {
        let (Some(phase), Some(start)) = (self.phase(phase_id), self.first_slot(phase_id)) else {
            return Vec::new();
        };
        let start = start.get();
        // the phase taking each unique rank on, if any
        let taker = |unique_rank: NonZeroU32| {
            self.phases.values().find_map(|taker| {
                taker
                    .sources()
                    .iter()
                    .any(|source| {
                        source.of() == SourceOf::Phase(*phase_id) && source.takes(unique_rank)
                    })
                    .then(|| taker.id())
            })
        };
        let standings = phase.standings();
        // the entrants in the rows before each row
        let before: Vec<u32> = standings
            .iter()
            .scan(0, |entrants, standing| {
                let before = *entrants;
                *entrants += standing.ranking().entrants().len() as u32;
                Some(before)
            })
            .collect();
        // the entrants before the first row of each rank and taker; until the phase is finished,
        // a cut through a rank is not decided, so its rows stay together
        let finished = phase.is_finished();
        let side = |taker: Option<PhaseId>| taker.filter(|_| finished);
        let mut first_of: Map<(NonZeroU32, Option<PhaseId>), u32> = Map::default();
        for (standing, &before) in standings.iter().zip(&before) {
            first_of
                .entry((standing.rank(), side(taker(standing.unique_rank()))))
                .or_insert(before);
        }
        standings
            .iter()
            .zip(before)
            .map(|(standing, before)| {
                let taker = taker(standing.unique_rank());
                let first = first_of[&(standing.rank(), side(taker))];
                (standing.ranking(), start + first, start + before, taker)
            })
            .collect()
    }

    /// The place in the competition of each ranking the phase `phase_id` places, i.e. of each row
    /// of its overall standings not gone on to a started phase, see [`Self::ranking`].
    pub fn places(&self, phase_id: &PhaseId) -> Map<RankingId, NonZeroU32> {
        self.ranking()
            .into_iter()
            .filter(|placed| placed.phase == *phase_id)
            .filter_map(|placed| Some((placed.ranking?, placed.place)))
            .collect()
    }

    /// The place in the competition every entrant of a started phase has as things stand, best
    /// first. An entrant is placed by the latest started phase they are in, by their row there;
    /// the members of a team share its row. A row's place is one more than the rows ranked before
    /// it, rows of one rank of a phase sharing the place of the first; where the cut of a source
    /// goes through rows sharing a rank of a finished phase, each side of the cut shares the place
    /// of its first row, so that the rows the lot left behind are placed after those it took on,
    /// while until the phase is finished they all share it. A phase's rows
    /// come where those of its sources it took on were: a phase starts where the first of them
    /// was, a source of the registration at its first place. A row going on to a phase not
    /// started yet counts as the rows that phase will form of its entrants, but at least as one:
    /// a pair to be split up counts as two, so that the places after it are those they will be,
    /// while entrants to be paired keep a row each until they are.
    ///
    /// An entrant of the registration that a phase not started yet takes is placed by that
    /// phase, without a ranking, a row of their own, sharing the place where their range of the
    /// registration starts with the others of it. Final once every phase is finished. Entrants of
    /// the registration in no phase are not placed, each keeping a place free; entrants of places
    /// of a phase that no phase takes are placed by the phase they stopped in.
    pub fn ranking(&self) -> Vec<Placed> {
        // (slot, index of the phase, unique slot, rows counted, placed) of every entrant placed
        // so far, see `slots`
        let mut placed: Map<EntrantId, (u32, usize, u32, u32, Placed)> = Map::default();
        for (index, phase) in self.phases.values().enumerate() {
            for (ranking, slot, unique, taker) in self.slots(&phase.id()) {
                // a taker placing the row has started, and replaces it below
                let rows = taker.map_or(1, |taker| {
                    let per_ranking = self.entrants_per_ranking(&taker).map_or(1, NonZeroU32::get);
                    (ranking.entrants().len() as u32 / per_ranking).max(1)
                });
                for entrant in ranking.entrants() {
                    let entry = Placed {
                        entrant: *entrant,
                        place: NonZeroU32::MIN,
                        phase: phase.id(),
                        ranking: Some(ranking.id()),
                    };
                    placed.insert(*entrant, (slot, index, unique, rows, entry));
                }
            }
        }
        // the entrants of the registration a phase not started yet takes, a row each, sharing
        // the place their range of the registration starts at
        for (index, phase) in self.phases.values().enumerate() {
            if phase.is_started() {
                continue;
            }
            for source in phase.sources() {
                if source.of() != SourceOf::Registration {
                    continue;
                }
                for (entrant, place) in self.registration.iter().zip(NonZeroU32::MIN..) {
                    if !source.takes(place) || placed.contains_key(entrant) {
                        continue;
                    }
                    let entry = Placed {
                        entrant: *entrant,
                        place: NonZeroU32::MIN,
                        phase: phase.id(),
                        ranking: None,
                    };
                    let slot = source.from().get();
                    placed.insert(*entrant, (slot, index, place.get(), 1, entry));
                }
            }
        }
        let mut placed: Vec<_> = placed.into_values().collect();
        placed.sort_by_key(|&(slot, index, unique, ..)| (slot, index, unique));
        // number the rows: rows sharing a slot in a phase share a place, and each slot left free
        // before a row (an entrant of the registration in no started phase) counts as a row
        let mut next_slot = 1;
        let mut rows_before = 0;
        let mut last_group = None;
        let mut last_row: Option<(usize, Result<RankingId, EntrantId>)> = None;
        let mut place = NonZeroU32::MIN;
        for (slot, index, _, rows, placed) in &mut placed {
            if last_group != Some((*slot, *index)) {
                last_group = Some((*slot, *index));
                rows_before += slot.saturating_sub(next_slot);
                next_slot = next_slot.max(*slot);
                place = NonZeroU32::MIN.saturating_add(rows_before);
            }
            // a row of a ranking, or of an entrant waiting for a phase
            let row = (*index, placed.ranking.ok_or(placed.entrant));
            if last_row != Some(row) {
                last_row = Some(row);
                rows_before += *rows;
            }
            next_slot += 1;
            placed.place = place;
        }
        placed.into_iter().map(|(.., placed)| placed).collect()
    }
}

/// How many entrants each qualifier of a field taken from `sources` stands for, each ranking of a
/// phase standing for `per_ranking` of its id, see [`Competition::entrants_per_qualifier`].
pub(crate) fn entrants_per_qualifier(
    sources: &[Source],
    per_ranking: impl Fn(&PhaseId) -> Option<NonZeroU32>,
) -> Option<NonZeroU32> {
    let mut sizes = sources.iter().map(|source| match source.of() {
        SourceOf::Registration => Some(NonZeroU32::MIN),
        SourceOf::Phase(phase) => per_ranking(&phase),
    });
    let first = sizes.next()??;
    sizes.all(|size| size == Some(first)).then_some(first)
}
