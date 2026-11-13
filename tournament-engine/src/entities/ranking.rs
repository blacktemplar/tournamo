use std::num::NonZeroU32;

use vec1::Vec1;

#[cfg(doc)]
use crate::Standings;
#[cfg(test)]
use crate::type_map::TypeMapValue;
use crate::{EntrantId, MatchId, RankingId, map::IdAble, type_map::TypeMap};

/// The entrants ranked together in a phase, with the matches they played and their metrics. Where
/// they stand is in the [`Standings`] of the phase.
#[derive(Debug)]
pub struct Ranking {
    pub(super) id: RankingId,
    pub(super) entrants: Vec1<EntrantId>,
    pub(super) initial_unique_rank: NonZeroU32,
    pub(super) played_matches: Vec<MatchId>,
    pub(super) metrics: TypeMap,
}

impl IdAble for Ranking {
    type Id = RankingId;

    fn id(&self) -> Self::Id {
        self.id
    }
}

impl Ranking {
    pub fn new(entrants: Vec1<EntrantId>, initial_unique_rank: NonZeroU32) -> Self {
        let id = RankingId::derive(&entrants);
        Self {
            id,
            entrants,
            initial_unique_rank,
            played_matches: Vec::new(),
            metrics: TypeMap::new(),
        }
    }

    pub fn id(&self) -> RankingId {
        self.id
    }

    pub fn entrants(&self) -> &Vec1<EntrantId> {
        &self.entrants
    }

    /// The place of the ranking when the phase started, unique within the phase. It breaks the
    /// ties of the [`Standings`].
    pub fn initial_unique_rank(&self) -> NonZeroU32 {
        self.initial_unique_rank
    }

    /// The matches of this ranking that have a result, in the order the results were reported in.
    ///
    /// A match is in here already while the metrics calculators update after its result.
    pub fn played_matches(&self) -> &[MatchId] {
        &self.played_matches
    }

    pub fn metric<T: 'static>(&self) -> Option<&T> {
        self.metrics.get::<T>()
    }

    pub fn metrics(&self) -> &TypeMap {
        &self.metrics
    }

    /// The ranking with `metric` set, for tests of rankers that do not need a whole phase.
    #[cfg(test)]
    pub(crate) fn with_metric(mut self, metric: impl TypeMapValue) -> Self {
        self.metrics.insert(metric);
        self
    }
}
