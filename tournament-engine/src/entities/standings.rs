//! Where the rankings of a phase stand, see [`Standings`].

use std::num::NonZeroU32;

use itertools::Itertools;

use crate::entities::Ranking;

/// The standings of a phase: its rankings best first, each with its ranks. They borrow the phase,
/// so it cannot change while they are read: standings are never stale.
#[derive(Debug, Clone)]
pub struct Standings<'a> {
    rows: Vec<Standing<'a>>,
}

impl<'a> Standings<'a> {
    /// The standings of `ranked`, every ranking of a phase with its rank as
    /// [`HierarchicalRanker::rank`] ranks them: ordered by rank, ties by their initial unique rank.
    ///
    /// [`HierarchicalRanker::rank`]: crate::plugins::ranker::HierarchicalRanker::rank
    pub(crate) fn new(mut ranked: Vec<(&'a Ranking, NonZeroU32)>) -> Self {
        ranked.sort_by_key(|&(ranking, rank)| (rank, ranking.initial_unique_rank()));
        // the invariant of the ranks, see `HierarchicalRanker::rank`
        debug_assert!(
            ranked
                .first()
                .is_none_or(|&(_, rank)| rank == NonZeroU32::MIN)
                && ranked
                    .iter()
                    .enumerate()
                    .array_windows()
                    .all(|[(_, (_, r1)), (i2, (_, r2))]| r1 == r2 || r2.get() == i2 as u32 + 1)
        );
        let rows = ranked
            .into_iter()
            .zip(NonZeroU32::MIN..)
            .map(|((ranking, rank), unique_rank)| Standing {
                ranking,
                rank,
                unique_rank,
            })
            .collect();
        Self { rows }
    }

    /// Best first, i.e. by [`Standing::unique_rank`].
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &Standing<'a>> {
        self.rows.iter()
    }

    /// The number of rankings.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether there are no rankings, as in a phase that has not started.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

/// A ranking in the standings: the ranking itself and where it stands.
#[derive(Debug, Clone, Copy)]
pub struct Standing<'a> {
    ranking: &'a Ranking,
    rank: NonZeroU32,
    unique_rank: NonZeroU32,
}

impl<'a> Standing<'a> {
    /// The ranking: its id, entrants, initial rank, played matches and metrics.
    pub fn ranking(&self) -> &'a Ranking {
        self.ranking
    }

    /// The rank of the ranking: 1 for the best, the same for rankings that are tied, and after a
    /// tie the rank that skips the tied ones, e.g. 1, 2, 2, 4.
    pub fn rank(&self) -> NonZeroU32 {
        self.rank
    }

    /// The unique rank of the ranking, i.e. its place in the standings, which is its
    /// [`rank`](Self::rank) with ties broken by their [`Ranking::initial_unique_rank`].
    pub fn unique_rank(&self) -> NonZeroU32 {
        self.unique_rank
    }
}
