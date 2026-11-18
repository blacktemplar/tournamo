//! Who plays together in a phase: the [`Teams`] axis and the teams of this crate.

use std::num::NonZeroU32;

use dyn_clone::DynClone;
use plugin_factory::{Plugin, axis, compatibility::Declaration, register};
use rand::{rngs::StdRng, seq::SliceRandom};
use serde::{Deserialize, Serialize};
use vec1::{Vec1, vec1};

#[cfg(doc)]
use crate::{
    AddPhase, Competition, StartCompetition, StartPhase,
    plugins::{draw::Draw, facts::EntrantsPerQualifier},
};
use crate::{
    Entrant, EntrantId, Qualifier, Seed,
    entities::Entropy,
    map::IdMap,
    plugins::facts::{self, EntrantsPerRanking},
};

/// Forms the teams of a phase from its field (see [`Competition::field_of`]) when it starts: the
/// entrants each of its rankings stands for, in an order the [`Draw`] deals them in. A team may
/// put entrants of several qualifiers together, or take the entrants of one apart.
///
/// A teams plugin provides [`EntrantsPerRanking`] to its phase, usually derived from the
/// [`EntrantsPerQualifier`] of its context, see
/// [`Declaration::provide_from`](plugin_factory::compatibility::Declaration::provide_from).
#[axis]
pub trait Teams: Plugin + DynClone {
    /// The teams, in the order of the new field, each its entrants. Every entrant of
    /// `view.field()` is in exactly one team, of [`Self::entrants_per_team`] entrants.
    fn form(&self, view: &TeamsView<'_>) -> Vec<Vec1<EntrantId>>;

    /// Whether a field of `qualifiers` can be formed into teams; [`StartCompetition`] and
    /// [`StartPhase`] refuse a start otherwise.
    fn fits(&self, qualifiers: usize) -> bool {
        let _ = qualifiers;
        true
    }

    /// How many entrants each team stands for, of qualifiers standing for `per_qualifier`
    /// entrants each (`None`: for different numbers). `None` if the teams would not all be of one
    /// size, which [`AddPhase`] refuses.
    fn entrants_per_team(&self, per_qualifier: Option<NonZeroU32>) -> Option<NonZeroU32>;
}

dyn_clone::clone_trait_object!(Teams);

/// What [`Teams`] sees of the phase it forms the teams of.
#[derive(Debug, Clone, Copy)]
pub struct TeamsView<'a> {
    field: &'a [Qualifier],
    entrants: &'a IdMap<Entrant>,
    seed: Seed,
}

impl<'a> TeamsView<'a> {
    pub(crate) fn new(field: &'a [Qualifier], entrants: &'a IdMap<Entrant>, seed: Seed) -> Self {
        Self {
            field,
            entrants,
            seed,
        }
    }

    /// The field of the phase: its sources in order, each best first.
    pub fn field(&self) -> &'a [Qualifier] {
        self.field
    }

    /// The entrant `id` of the competition, with the annotations a consumer gave it.
    ///
    /// # Panics
    /// Panics if `id` is not an entrant of the competition, which no entrant of the field is.
    pub fn entrant(&self, id: &EntrantId) -> &'a Entrant {
        &self.entrants[id]
    }

    /// The random stream of the teams, derived from the seed of the phase. Every call starts the
    /// same stream again, so a teams plugin takes everything it needs from a single one.
    pub fn rng(&self) -> StdRng {
        self.seed.rng(Entropy::Teams)
    }
}

/// Every qualifier is a team, in the order of the field.
#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::keep",
    label = "unchanged",
    description = "everyone plays as they come in: on their own from the registration, in their \
                   team from a phase played in teams",
    declare_static = Self::declare_static
)]
pub struct Keep;

impl Keep {
    fn declare_static(declaration: &mut Declaration) {
        declaration.provide_from(|coming_in: &facts::EntrantsPerQualifier| {
            EntrantsPerRanking(coming_in.0)
        });
    }
}

#[register]
impl Teams for Keep {
    fn form(&self, view: &TeamsView<'_>) -> Vec<Vec1<EntrantId>> {
        view.field()
            .iter()
            .map(|qualifier| qualifier.entrants.clone())
            .collect()
    }

    fn entrants_per_team(&self, per_qualifier: Option<NonZeroU32>) -> Option<NonZeroU32> {
        per_qualifier
    }
}

/// What a plugin putting two single entrants together declares: it requires single entrants
/// coming in and provides rankings of two.
fn declare_pairs(declaration: &mut Declaration) {
    declaration
        .require_that::<facts::EntrantsPerQualifier>(
            "the entrants per qualifier",
            "single entrants coming in",
            |coming_in| coming_in.0 == NonZeroU32::MIN,
        )
        .provide(EntrantsPerRanking(NonZeroU32::new(2).expect("2 is not 0")));
}

/// How many entrants a pair of qualifiers of `per_qualifier` entrants each stands for.
fn pairs_of(per_qualifier: Option<NonZeroU32>) -> Option<NonZeroU32> {
    per_qualifier.map(|entrants| entrants.saturating_mul(NonZeroU32::new(2).expect("2 is not 0")))
}

/// The teams of `pairs`, pairs of positions in the field of single entrants: ordered by the sum
/// of their positions, ties by the better position, each listing the entrant of the better
/// position first.
fn teams_of(
    field: &[Qualifier],
    pairs: impl IntoIterator<Item = (usize, usize)>,
) -> Vec<Vec1<EntrantId>> {
    let mut pairs: Vec<_> = pairs
        .into_iter()
        .map(|(first, second)| (first.min(second), first.max(second)))
        .collect();
    pairs.sort_by_key(|&(better, worse)| (better + worse, better));
    pairs
        .into_iter()
        .map(|(better, worse)| {
            vec1![
                *field[better].entrants.first(),
                *field[worse].entrants.first()
            ]
        })
        .collect()
}

/// Draws the entrants into pairs by lot: shuffles the field, then pairs neighbours. Fits an even
/// field of single entrants.
#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::random_pairs",
    label = "drawn pairs",
    description = "the entrants are drawn into pairs by lot, which play together throughout",
    declare_static = declare_pairs
)]
pub struct RandomPairs;

#[register]
impl Teams for RandomPairs {
    fn form(&self, view: &TeamsView<'_>) -> Vec<Vec1<EntrantId>> {
        let mut order: Vec<_> = (0..view.field().len()).collect();
        order.shuffle(&mut view.rng());
        teams_of(
            view.field(),
            order
                .as_chunks::<2>()
                .0
                .iter()
                .map(|&[first, second]| (first, second)),
        )
    }

    fn fits(&self, qualifiers: usize) -> bool {
        qualifiers.is_multiple_of(2)
    }

    fn entrants_per_team(&self, per_qualifier: Option<NonZeroU32>) -> Option<NonZeroU32> {
        pairs_of(per_qualifier)
    }
}

/// Pairs the best of the field with the worst, the second with the second last, and so on. Fits
/// an even field of single entrants.
#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::best_with_worst",
    label = "best with worst",
    description = "pairs the first with the last, the second with the second last, and so on: from \
                   the registration in the order of the ratings, by lot where there are none, \
                   from a phase by its places",
    declare_static = declare_pairs
)]
pub struct BestWithWorst;

#[register]
impl Teams for BestWithWorst {
    fn form(&self, view: &TeamsView<'_>) -> Vec<Vec1<EntrantId>> {
        let count = view.field().len();
        teams_of(
            view.field(),
            (0..count / 2).map(|better| (better, count - 1 - better)),
        )
    }

    fn fits(&self, qualifiers: usize) -> bool {
        qualifiers.is_multiple_of(2)
    }

    fn entrants_per_team(&self, per_qualifier: Option<NonZeroU32>) -> Option<NonZeroU32> {
        pairs_of(per_qualifier)
    }
}

/// Every entrant plays on their own: the entrants of each qualifier, in the order of the field,
/// those of one qualifier in the order it lists them.
#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::split",
    label = "everyone on their own",
    description = "teams of a phase played in teams are split up, so their members play on their \
                   own; the same as unchanged for the registration",
    declare_static = Self::declare_static
)]
pub struct Split;

impl Split {
    fn declare_static(declaration: &mut Declaration) {
        declaration.provide(EntrantsPerRanking::SINGLE);
    }
}

#[register]
impl Teams for Split {
    fn form(&self, view: &TeamsView<'_>) -> Vec<Vec1<EntrantId>> {
        view.field()
            .iter()
            .flat_map(|qualifier| qualifier.entrants.iter().map(|entrant| vec1![*entrant]))
            .collect()
    }

    fn entrants_per_team(&self, _: Option<NonZeroU32>) -> Option<NonZeroU32> {
        Some(NonZeroU32::MIN)
    }
}

#[cfg(test)]
mod tests;
