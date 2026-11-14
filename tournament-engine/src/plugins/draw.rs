//! How the field of a phase is dealt to its divisions: the [`Draw`] axis and the draws of this
//! crate.

use std::num::NonZeroU32;

use dyn_clone::DynClone;
use plugin_factory::{Plugin, axis, register};
use rand::{rngs::StdRng, seq::SliceRandom};
use serde::{Deserialize, Serialize};

#[cfg(doc)]
use crate::{Competition, plugins::phase_format::PhaseFormat};
use crate::{
    Entrant, EntrantId, Origin, PhaseId, Qualifier, Seed, entities::Entropy, map::IdMap,
    plugins::phase_format::seeding_order,
};

/// Deals the field of a phase (see [`Competition::field_of`]) to its divisions when it starts,
/// each division's qualifiers in seeding order: the order its format seeds them by, as a knockout
/// seeds its bracket.
#[axis]
pub trait Draw: Plugin + DynClone {
    /// For each division the positions in `view.field()` of its qualifiers, in seeding order.
    /// Every position is in exactly one division; there are `view.divisions()` of them, none
    /// empty unless the field is. A format needs a number of qualifiers in a division (see
    /// [`PhaseFormat::min_entrants`]): there are at least that many times as many qualifiers as
    /// divisions, unless there is one division, so a draw dealing them evenly gives each enough.
    fn draw(&self, view: &DrawView<'_>) -> Vec<Vec<usize>>;

    /// How the divisions stand to each other in the overall standings of the phase.
    fn order(&self) -> DivisionOrder {
        DivisionOrder::Parallel
    }
}

dyn_clone::clone_trait_object!(Draw);

/// How the divisions of a phase stand to each other, see
/// [`Phase::standings`](crate::Phase::standings).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DivisionOrder {
    /// No division is better than another, as groups are: the overall standings take the best of
    /// every division first, then the second best, and so on.
    Parallel,
    /// Everyone of a division stands above everyone of the divisions after it, as in knockouts
    /// A/B/C: the overall standings are those of the divisions one after the other.
    Levels,
}

/// What a [`Draw`] sees of the phase it deals.
#[derive(Debug, Clone, Copy)]
pub struct DrawView<'a> {
    field: &'a [Qualifier],
    divisions: usize,
    entrants: &'a IdMap<Entrant>,
    seed: Seed,
}

impl<'a> DrawView<'a> {
    pub(crate) fn new(
        field: &'a [Qualifier],
        divisions: usize,
        entrants: &'a IdMap<Entrant>,
        seed: Seed,
    ) -> Self {
        Self {
            field,
            divisions,
            entrants,
            seed,
        }
    }

    /// The field of the phase: its sources in order, each best first.
    pub fn field(&self) -> &'a [Qualifier] {
        self.field
    }

    /// How many divisions the field is dealt to: as many as the phase is configured with, but no
    /// more than leave each the qualifiers its format needs, and at least one.
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

    /// The random stream of the draw, derived from the seed of the phase. Every call starts the
    /// same stream again, so a draw takes everything it needs from a single one.
    pub fn rng(&self) -> StdRng {
        self.seed.rng(Entropy::Draw)
    }
}

/// `order`, positions of the field, dealt to `divisions` divisions as a snake: the first
/// position to the first division, the second to the second, ..., then back from the last
/// division, so that the best of each round of dealing alternate between the ends.
fn snake(order: impl IntoIterator<Item = usize>, divisions: usize) -> Vec<Vec<usize>> {
    let mut dealt = vec![Vec::new(); divisions];
    for (index, position) in order.into_iter().enumerate() {
        let (round, at) = (index / divisions, index % divisions);
        let division = if round % 2 == 0 {
            at
        } else {
            divisions - 1 - at
        };
        dealt[division].push(position);
    }
    dealt
}

/// Keeps the order of the field: one division gets it as it is, several are dealt it as a snake,
/// the first qualifier to A, the second to B, ..., the qualifier after the last division's to the
/// last division again, then back to A.
#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::in_order",
    label = "in order",
    description = "keeps the order of the field; several groups get it in turn, back and forth"
)]
pub struct InOrder;

#[register]
impl Draw for InOrder {
    fn draw(&self, view: &DrawView<'_>) -> Vec<Vec<usize>> {
        snake(0..view.field().len(), view.divisions())
    }
}

/// Shuffles the field, then deals it in turn: the first to A, the second to B, ...
#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::random",
    label = "at random",
    description = "every qualifier to a group and a seed by lot"
)]
pub struct Random;

#[register]
impl Draw for Random {
    fn draw(&self, view: &DrawView<'_>) -> Vec<Vec<usize>> {
        let mut order: Vec<_> = (0..view.field().len()).collect();
        order.shuffle(&mut view.rng());
        let mut dealt = vec![Vec::new(); view.divisions()];
        for (index, position) in order.into_iter().enumerate() {
            dealt[index % view.divisions()].push(position);
        }
        dealt
    }
}

/// Deals the field in blocks of consecutive places, as even in size as they can be: the best to
/// A, the next to B, and so on; where the field does not divide evenly, the first divisions get
/// one more. The divisions are levels: everyone of A is placed above everyone of B.
#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::blocks",
    label = "in blocks",
    description = "the best into the first division, the next into the second, and so on; \
                   every division plays for places of its own"
)]
pub struct Blocks;

#[register]
impl Draw for Blocks {
    fn draw(&self, view: &DrawView<'_>) -> Vec<Vec<usize>> {
        let (size, larger) = (
            view.field().len() / view.divisions(),
            view.field().len() % view.divisions(),
        );
        let mut next = 0;
        (0..view.divisions())
            .map(|division| {
                let end = next + size + usize::from(division < larger);
                let block = (next..end).collect();
                next = end;
                block
            })
            .collect()
    }

    fn order(&self) -> DivisionOrder {
        DivisionOrder::Levels
    }
}

/// Seeds a knockout fed by groups so that entrants of one group meet as late as it can be
/// arranged.
///
/// The field is ordered by rank in the group (by unique rank in the division the qualifier comes
/// from): every group's best first, then every group's second, and so on, a qualifier of the
/// registration counting as the best of a group of its own; qualifiers of one rank keep the order
/// of the field. Within each rank the order is then changed as little as it takes, so that in the
/// bracket the order seeds (best seed against worst, as [`SingleElimination`] seeds) no two
/// qualifiers of one group meet in the first round, and, where every group sends at least two,
/// the best two of every group are in opposite halves of the bracket, so they can only meet in
/// the final. Where that cannot be arranged, it keeps only the first; where neither, the order by
/// rank in the group.
///
/// Meant for one division; several are dealt the order as a snake, as [`InOrder`] deals them.
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
impl Draw for GroupWinnersApart {
    fn draw(&self, view: &DrawView<'_>) -> Vec<Vec<usize>> {
        let seeds = KeptApart::new(view.field());
        let order = seeds
            .search(true)
            .or_else(|| seeds.search(false))
            .unwrap_or(seeds.by_rank);
        snake(order, view.divisions())
    }
}

/// The search of [`GroupWinnersApart`]: positions of the field, by rank in their group.
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
    fn new(field: &[Qualifier]) -> Self {
        let (groups, ranks): (Vec<_>, Vec<_>) = field
            .iter()
            .map(|qualifier| match qualifier.origin {
                Origin::Registration { .. } => (None, NonZeroU32::MIN),
                Origin::Phase {
                    phase,
                    division,
                    unique_rank,
                    ..
                } => (Some((phase, division)), unique_rank),
            })
            .unzip();
        let mut by_rank: Vec<_> = (0..field.len()).collect();
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
        let mut partner = vec![None; field.len()];
        let every_group_sends_two = members.len() > 1
            && groups.iter().all(Option::is_some)
            && members.iter().all(|(_, positions)| positions.len() > 1);
        if every_group_sends_two {
            for (_, positions) in &members {
                partner[positions[0]] = Some(positions[1]);
                partner[positions[1]] = Some(positions[0]);
            }
        }
        let size = field.len().next_power_of_two();
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
