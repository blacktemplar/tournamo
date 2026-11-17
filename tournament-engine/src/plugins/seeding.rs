//! The order a division starts with: the [`Seeding`] axis and the seedings of this crate.

use std::num::NonZeroU32;

use dyn_clone::DynClone;
use plugin_factory::{Plugin, axis, register};
use rand::{rngs::StdRng, seq::SliceRandom};
use serde::{Deserialize, Serialize};

#[cfg(doc)]
use crate::plugins::{draw::Draw, phase_format::PhaseFormat};
use crate::{
    Entrant, EntrantId, Origin, PhaseId, Qualifier, Seed, entities::Entropy, map::IdMap,
    plugins::phase_format::seeding_order,
};

/// Orders the qualifiers a [`Draw`] dealt to a division when its phase starts: the order its
/// format seeds them by, as a knockout seeds its bracket (see [`PhaseFormat::reads_seeding`]).
#[axis]
pub trait Seeding: Plugin + DynClone {
    /// The qualifiers of the division in seeding order, best seed first: a permutation of
    /// `view.dealt()`, positions in `view.field()`.
    fn seed(&self, view: &SeedingView<'_>) -> Vec<usize>;
}

dyn_clone::clone_trait_object!(Seeding);

/// What a [`Seeding`] sees of the division it seeds.
#[derive(Debug, Clone, Copy)]
pub struct SeedingView<'a> {
    field: &'a [Qualifier],
    dealt: &'a [usize],
    division: usize,
    divisions: usize,
    entrants: &'a IdMap<Entrant>,
    seed: Seed,
}

impl<'a> SeedingView<'a> {
    /// The view of division `division` of `divisions`, dealt the positions `dealt` of `field`, in
    /// field order, `seed` being the division's.
    pub(crate) fn new(
        field: &'a [Qualifier],
        dealt: &'a [usize],
        (division, divisions): (usize, usize),
        entrants: &'a IdMap<Entrant>,
        seed: Seed,
    ) -> Self {
        Self {
            field,
            dealt,
            division,
            divisions,
            entrants,
            seed,
        }
    }

    /// The field of the phase: its sources in order, each best first.
    pub fn field(&self) -> &'a [Qualifier] {
        self.field
    }

    /// The positions in [`field`](Self::field) of the qualifiers dealt to this division, in the
    /// order of the field.
    pub fn dealt(&self) -> &'a [usize] {
        self.dealt
    }

    /// The index of this division in its phase: 0 for the first.
    pub fn division(&self) -> usize {
        self.division
    }

    /// How many divisions the phase is played in.
    pub fn divisions(&self) -> usize {
        self.divisions
    }

    /// The entrant `id` of the competition, with the annotations a consumer gave it.
    ///
    /// # Panics
    /// Panics if `id` is not an entrant of the competition, which no entrant of the field is.
    pub fn entrant(&self, id: &EntrantId) -> &'a Entrant {
        &self.entrants[id]
    }

    /// The random stream of the seeding, derived from the seed of the division. Every call starts
    /// the same stream again, so a seeding takes everything it needs from a single one.
    pub fn rng(&self) -> StdRng {
        self.seed.rng(Entropy::Seeding)
    }
}

/// Keeps the order the qualifiers have in the field: by their places in the phases they come
/// from, those of the registration by rating, the rest by lot.
#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::seeded_in_order",
    label = "by rank",
    description = "the entrants are seeded by their places in the phases before, or else by \
                   rating; those without a rating by lot"
)]
pub struct SeededInOrder;

#[register]
impl Seeding for SeededInOrder {
    fn seed(&self, view: &SeedingView<'_>) -> Vec<usize> {
        view.dealt().to_vec()
    }
}

/// Seeds the qualifiers in an order drawn by lot.
#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::seeded_at_random",
    label = "at random",
    description = "the entrants are seeded by lot"
)]
pub struct SeededAtRandom;

#[register]
impl Seeding for SeededAtRandom {
    fn seed(&self, view: &SeedingView<'_>) -> Vec<usize> {
        let mut order = view.dealt().to_vec();
        order.shuffle(&mut view.rng());
        order
    }
}

/// Seeds a knockout fed by groups so that entrants of one group meet as late as it can be
/// arranged.
///
/// The qualifiers of the division are ordered by rank in the group (by unique rank in the division
/// the qualifier comes from): every group's best first, then every group's second, and so on, a
/// qualifier of the registration counting as the best of a group of its own, and a team of entrants
/// of several groups (see [`Qualifier::origins`]) as a group of its own, of the best rank among
/// them; qualifiers of one rank keep the order of the field. Within each rank the order is then
/// changed as little as it takes, so that in the bracket the order seeds (best seed against worst,
/// as [`SingleElimination`] seeds) no two qualifiers of one group meet in the first round, and,
/// where every group sends at least two, the best two of every group are in opposite halves of the
/// bracket, so they can only meet in the final. Where that cannot be arranged, it keeps only the
/// first; where neither, the order by rank in the group.
///
/// [`SingleElimination`]: crate::plugins::phase_format::SingleElimination
#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::group_winners_apart",
    label = "groups kept apart",
    description = "for a knockout of the best of several groups: entrants of one group meet as \
                   late as possible"
)]
pub struct GroupWinnersApart;

#[register]
impl Seeding for GroupWinnersApart {
    fn seed(&self, view: &SeedingView<'_>) -> Vec<usize> {
        let field = view.field();
        let seeds = KeptApart::new(view.dealt().iter().map(|&position| &field[position]));
        let order = seeds
            .search(true)
            .or_else(|| seeds.search(false))
            .unwrap_or(seeds.by_rank);
        order.into_iter().map(|index| view.dealt()[index]).collect()
    }
}

/// The search of [`GroupWinnersApart`]: indices of the qualifiers it seeds, by rank in their
/// group.
struct KeptApart {
    /// The group of each position, `None` for a group of its own.
    groups: Vec<Option<(PhaseId, usize)>>,
    /// The rank in the group of each position.
    ranks: Vec<NonZeroU32>,
    /// The positions ordered by rank in the group, then by position.
    by_rank: Vec<usize>,
    /// For each position, the other of the best two of its group if it is one of them and every
    /// group sends two.
    partner: Vec<Option<usize>>,
    /// The size of the bracket: a power of two.
    size: usize,
    /// For each seed, whether it is in the top half of the bracket.
    top_half: Vec<bool>,
}

/// How many seeds the search of [`KeptApart`] tries before it gives up.
const SEARCH_BUDGET: usize = 100_000;

impl KeptApart {
    fn new<'a>(qualifiers: impl ExactSizeIterator<Item = &'a Qualifier>) -> Self {
        let count = qualifiers.len();
        let (groups, ranks): (Vec<_>, Vec<_>) = qualifiers
            .map(|qualifier| {
                let group_and_rank = |origin: &Origin| match *origin {
                    Origin::Registration { .. } => (None, NonZeroU32::MIN),
                    Origin::Phase {
                        phase,
                        division,
                        unique_rank,
                        ..
                    } => (Some((phase, division)), unique_rank),
                };
                match qualifier.origins.as_slice() {
                    [origin] => group_and_rank(origin),
                    // a team of several groups, by the best rank of its members
                    origins => (
                        None,
                        origins
                            .iter()
                            .map(|origin| group_and_rank(origin).1)
                            .min()
                            .expect("a qualifier has an origin"),
                    ),
                }
            })
            .unzip();
        let mut by_rank: Vec<_> = (0..count).collect();
        by_rank.sort_by_key(|&position| ranks[position]);
        // the positions of every group, best first
        let mut members: Vec<((PhaseId, usize), Vec<usize>)> = Vec::new();
        for &position in &by_rank {
            let Some(group) = groups[position] else {
                continue;
            };
            match members.iter_mut().find(|(of, _)| *of == group) {
                Some((_, positions)) => positions.push(position),
                None => members.push((group, vec![position])),
            }
        }
        let mut partner = vec![None; count];
        let every_group_sends_two = members.len() > 1
            && groups.iter().all(Option::is_some)
            && members.iter().all(|(_, positions)| positions.len() > 1);
        if every_group_sends_two {
            for (_, positions) in &members {
                partner[positions[0]] = Some(positions[1]);
                partner[positions[1]] = Some(positions[0]);
            }
        }
        let size = count.next_power_of_two();
        let mut top_half = vec![false; size];
        for (index, seed) in seeding_order(size.trailing_zeros()).into_iter().enumerate() {
            top_half[seed] = index < size / 2;
        }
        Self {
            groups,
            ranks,
            by_rank,
            partner,
            size,
            top_half,
        }
    }

    /// The first order, changing the order by rank as little as it takes, in which no two of a
    /// group meet in the first round and, if `halves`, the best two of each group are in opposite
    /// halves; `None` if there is none or the search runs out of its budget.
    fn search(&self, halves: bool) -> Option<Vec<usize>> {
        let mut order = Vec::with_capacity(self.by_rank.len());
        let mut placed = vec![None; self.by_rank.len()];
        let mut budget = SEARCH_BUDGET;
        self.place(&mut order, &mut placed, halves, &mut budget)
            .then_some(order)
    }

    /// Places the next seed and every one after it, depth first; whether it succeeded.
    fn place(
        &self,
        order: &mut Vec<usize>,
        placed: &mut [Option<usize>],
        halves: bool,
        budget: &mut usize,
    ) -> bool {
        let seed = order.len();
        let Some(&next) = self.by_rank.get(seed) else {
            return true;
        };
        let rank = self.ranks[next];
        // the positions of the rank this seed is for, best first
        let candidates: Vec<_> = self
            .by_rank
            .iter()
            .copied()
            .filter(|&position| self.ranks[position] == rank && placed[position].is_none())
            .collect();
        for position in candidates {
            if *budget == 0 {
                return false;
            }
            *budget -= 1;
            if !self.fits(order, placed, position, halves) {
                continue;
            }
            order.push(position);
            placed[position] = Some(seed);
            if self.place(order, placed, halves, budget) {
                return true;
            }
            placed[position] = None;
            order.pop();
        }
        false
    }

    /// Whether `position` can take the next seed after `order`.
    fn fits(
        &self,
        order: &[usize],
        placed: &[Option<usize>],
        position: usize,
        halves: bool,
    ) -> bool {
        let seed = order.len();
        let opponent = self.size - 1 - seed;
        let group = self.groups[position];
        let meets_own_group =
            opponent < seed && group.is_some() && self.groups[order[opponent]] == group;
        let same_half_as_partner = halves
            && self.partner[position]
                .and_then(|partner| placed[partner])
                .is_some_and(|partner_seed| self.top_half[partner_seed] == self.top_half[seed]);
        !meets_own_group && !same_half_as_partner
    }
}

#[cfg(test)]
mod tests;
