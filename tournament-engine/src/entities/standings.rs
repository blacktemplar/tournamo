//! Where the rankings of a division stand, see [`Standings`], and those of a phase, see
//! [`PhaseStandings`].

use std::num::NonZeroU32;

use itertools::Itertools;

use crate::entities::Ranking;

/// The standings of a division: its rankings best first, each with its ranks. They borrow the
/// division, so it cannot change while they are read: standings are never stale.
#[derive(Debug, Clone)]
pub struct Standings<'a> {
    rows: Vec<Standing<'a>>,
}

impl<'a> Standings<'a> {
    /// The standings of `ranked`, every ranking of a division with its rank as
    /// [`HierarchicalRanker::rank`] ranks them: ordered by rank, ties by their lot.
    ///
    /// [`HierarchicalRanker::rank`]: crate::plugins::ranker::HierarchicalRanker::rank
    pub(crate) fn new(mut ranked: Vec<(&'a Ranking, NonZeroU32)>) -> Self {
        ranked.sort_by_key(|&(ranking, rank)| (rank, ranking.lot()));
        debug_assert!(ranks_are_valid(ranked.iter().map(|&(_, rank)| rank)));
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

    /// Whether there are no rankings.
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
    /// [`rank`](Self::rank) with ties broken by their [`Ranking::lot`].
    pub fn unique_rank(&self) -> NonZeroU32 {
        self.unique_rank
    }
}

/// The overall standings of a phase: the rows of the standings of all its divisions, best first,
/// each with its overall rank, see [`Phase::standings`](crate::Phase::standings).
#[derive(Debug, Clone)]
pub struct PhaseStandings<'a> {
    rows: Vec<PhaseStanding<'a>>,
}

impl<'a> PhaseStandings<'a> {
    /// The standings of `rows`, each the row of a division's standings, its division and overall
    /// rank, ordered by overall rank and ties by their lot.
    pub(crate) fn new(mut rows: Vec<(Standing<'a>, usize, NonZeroU32)>) -> Self {
        rows.sort_by_key(|&(standing, _, rank)| (rank, standing.ranking().lot()));
        debug_assert!(ranks_are_valid(rows.iter().map(|&(_, _, rank)| rank)));
        let rows = rows
            .into_iter()
            .zip(NonZeroU32::MIN..)
            .map(|((standing, division, rank), unique_rank)| PhaseStanding {
                standing,
                division,
                rank,
                unique_rank,
            })
            .collect();
        Self { rows }
    }

    /// Best first, i.e. by [`PhaseStanding::unique_rank`].
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &PhaseStanding<'a>> {
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

/// A ranking in the overall standings of a phase: its row in its division's standings, and where
/// it stands overall.
#[derive(Debug, Clone, Copy)]
pub struct PhaseStanding<'a> {
    standing: Standing<'a>,
    division: usize,
    rank: NonZeroU32,
    unique_rank: NonZeroU32,
}

impl<'a> PhaseStanding<'a> {
    /// The row of the ranking in the standings of its division.
    pub fn in_division(&self) -> Standing<'a> {
        self.standing
    }

    /// The ranking: its id, entrants, initial rank, played matches and metrics.
    pub fn ranking(&self) -> &'a Ranking {
        self.standing.ranking()
    }

    /// The index of the division of the ranking.
    pub fn division(&self) -> usize {
        self.division
    }

    /// The overall rank of the ranking, shared by rankings that are tied, as
    /// [`Standing::rank`].
    pub fn rank(&self) -> NonZeroU32 {
        self.rank
    }

    /// The overall unique rank of the ranking, i.e. its place in the overall standings: its
    /// [`rank`](Self::rank) with ties broken by their [`Ranking::lot`].
    pub fn unique_rank(&self) -> NonZeroU32 {
        self.unique_rank
    }
}

/// Whether `ranks`, in order, satisfy the invariant of the ranks of
/// [`HierarchicalRanker::rank`](crate::plugins::ranker::HierarchicalRanker::rank): the first is 1,
/// every other one that of its predecessor or its index plus one.
fn ranks_are_valid(ranks: impl Iterator<Item = NonZeroU32>) -> bool {
    let ranks: Vec<_> = ranks.collect();
    ranks.first().is_none_or(|&rank| rank == NonZeroU32::MIN)
        && ranks
            .iter()
            .enumerate()
            .array_windows()
            .all(|[(_, r1), (i2, r2)]| r1 == r2 || r2.get() == i2 as u32 + 1)
}
