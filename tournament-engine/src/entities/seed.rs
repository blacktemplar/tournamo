use std::hash::Hash;

use rand::rngs::StdRng;
use rand_seeder::Seeder;
use serde::{Deserialize, Serialize};

/// The source of all randomness of a competition, or of one of its phases.
///
/// Every random stream is derived from a seed and some entropy, so replaying the same transitions
/// gives the same results.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Seed([u8; 12]);

impl Seed {
    pub const fn new(bytes: [u8; 12]) -> Self {
        Self(bytes)
    }

    /// The random stream of this seed mixed with `entropy`: different values give independent
    /// streams, the same value always gives the same one.
    pub(super) fn rng(self, entropy: impl Hash) -> StdRng {
        Seeder::from((self.0, entropy)).into_rng()
    }
}

/// What the random streams of a competition are told apart by, see [`Seed::rng`].
// the discriminants are hashed, so reordering the variants changes every random stream
#[derive(Debug, PartialEq, Eq, Hash)]
pub(super) enum Entropy {
    CompetitionStart,
    PhaseFormat,
}
