//! How the field of a phase is dealt to its divisions: the [`Draw`] axis and the draws of this
//! crate.

use dyn_clone::DynClone;
use plugin_factory::{Plugin, axis, register};
use rand::{rngs::StdRng, seq::SliceRandom};
use serde::{Deserialize, Serialize};

#[cfg(doc)]
use crate::{
    Competition,
    plugins::{phase_format::PhaseFormat, seeding::Seeding},
};
use crate::{Entrant, EntrantId, Qualifier, Seed, entities::Entropy, map::IdMap};

/// Deals the field of a phase (see [`Competition::field_of`]) to its divisions when it starts:
/// who plays in which division. The order within a division is the [`Seeding`]'s.
#[axis]
pub trait Draw: Plugin + DynClone {
    /// For each division the positions in `view.field()` of its qualifiers. Every position is in
    /// exactly one division; there are `view.divisions()` of them, none empty unless the field is.
    /// The order within a division does not matter: see [`Seeding`]. A format needs a number of
    /// qualifiers in a division (see [`PhaseFormat::min_entrants`]): there are at least that many
    /// times as many qualifiers as divisions, unless there is one division, so a draw dealing them
    /// evenly gives each enough.
    fn draw(&self, view: &DrawView<'_>) -> Vec<Vec<usize>>;

    /// How the divisions stand to each other in the overall standings of the phase.
    fn order(&self) -> DivisionOrder {
        DivisionOrder::Parallel
    }
}

dyn_clone::clone_trait_object!(Draw);

/// How the divisions of a phase stand to each other, see
/// [`Phase::standings`](crate::Phase::standings).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

/// Deals the field in its order as a snake: the first qualifier to A, the second to B, ..., the
/// qualifier after the last division's to the last division again, then back to A. One division
/// gets the whole field.
#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::snake",
    label = "back and forth",
    description = "the groups get the entrants in turn, in their order, back and forth"
)]
pub struct Snake;

#[register]
impl Draw for Snake {
    fn draw(&self, view: &DrawView<'_>) -> Vec<Vec<usize>> {
        snake(0..view.field().len(), view.divisions())
    }
}

/// Shuffles the field, then deals it in turn: the first to A, the second to B, ...
#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::random",
    label = "at random",
    description = "every entrant to a group by lot"
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

#[cfg(test)]
mod tests;
