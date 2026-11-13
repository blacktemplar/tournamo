use std::num::NonZeroU32;

use serde::{Deserialize, Serialize};

use crate::{Annotated, ByeId, RankingId, map::IdAble, type_map::TypeMap};
#[cfg(doc)]
use crate::{
    Match,
    plugins::{metrics_calculator::MetricsCalculator, phase_format::PhaseFormat},
};

/// A round of a phase in which a ranking has no match to play, as decided by the
/// [`PhaseFormat`] that schedules it.
///
/// A bye has no result to report. It counts once its ranking [reaches](Self::is_reached) it, as
/// if it were a match reported as soon as it can be played: then the metrics of its ranking are
/// updated, see [`MetricsCalculator::update_after_bye`]. Like a match it holds `annotations`, given
/// by the format when creating it with [`Annotated::with_annotation`].
#[derive(Debug, Serialize, Deserialize)]
pub struct Bye {
    id: ByeId,
    ranking: RankingId,
    round: NonZeroU32,
    pub(super) reached: bool,
    annotations: TypeMap,
}

impl IdAble for Bye {
    type Id = ByeId;

    fn id(&self) -> Self::Id {
        self.id
    }
}

impl Bye {
    pub fn new(ranking: RankingId, round: NonZeroU32) -> Self {
        let id = ByeId::derive(ranking, round);
        Self {
            id,
            ranking,
            round,
            reached: false,
            annotations: TypeMap::new(),
        }
    }

    pub fn id(&self) -> ByeId {
        self.id
    }

    /// Whether the ranking has reached this bye, i.e. had no open match in an earlier round of the
    /// phase at some point since it was scheduled. Only a reached bye counts for the metrics, and
    /// it stays reached.
    pub fn is_reached(&self) -> bool {
        self.reached
    }

    /// The ranking that has no match to play.
    pub fn ranking(&self) -> RankingId {
        self.ranking
    }

    /// The round the ranking has no match in. Like a match, a bye takes up the one slot its ranking
    /// has in that round, see [`Match::round`].
    pub fn round(&self) -> NonZeroU32 {
        self.round
    }
}

impl Annotated for Bye {
    fn annotations(&self) -> &TypeMap {
        &self.annotations
    }

    fn annotations_mut(&mut self) -> &mut TypeMap {
        &mut self.annotations
    }
}
