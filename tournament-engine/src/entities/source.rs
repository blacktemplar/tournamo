use std::num::NonZeroU32;

use serde::{Deserialize, Serialize};
use vec1::Vec1;

#[cfg(doc)]
use crate::{AddPhase, Competition, Standings};
use crate::{EntrantId, PhaseId};

/// Where a phase takes entrants from: the places `from..=to` of the registration or of the
/// overall standings of an earlier phase, see [`AddPhase`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Source {
    of: SourceOf,
    from: NonZeroU32,
    /// To the last place if `None`.
    to: Option<NonZeroU32>,
}

/// What a [`Source`] takes places of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SourceOf {
    /// The registration: every entrant on their own, in the order of the registration, see
    /// [`Competition::registration`].
    Registration,
    /// The overall standings of the phase, by unique rank.
    Phase(PhaseId),
}

impl Source {
    /// The places `from..=to` of `of`, to the last place if `to` is `None`. A source whose `to` is
    /// before its `from` takes nothing, and [`AddPhase`] refuses it.
    pub const fn new(of: SourceOf, from: NonZeroU32, to: Option<NonZeroU32>) -> Self {
        Self { of, from, to }
    }

    /// Every place of `of`.
    pub const fn everyone(of: SourceOf) -> Self {
        Self::new(of, NonZeroU32::MIN, None)
    }

    /// Every entrant of the registration.
    pub const fn registration() -> Self {
        Self::everyone(SourceOf::Registration)
    }

    pub fn of(&self) -> SourceOf {
        self.of
    }

    /// The first place this source takes.
    pub fn from(&self) -> NonZeroU32 {
        self.from
    }

    /// The last place this source takes; `None` for the last place there is.
    pub fn to(&self) -> Option<NonZeroU32> {
        self.to
    }

    /// Whether the source takes no place, its `to` being before its `from`.
    pub fn is_empty(&self) -> bool {
        self.to.is_some_and(|to| to < self.from)
    }

    /// Whether this source takes `place`.
    pub fn takes(&self, place: NonZeroU32) -> bool {
        self.from <= place && self.to.is_none_or(|to| place <= to)
    }

    /// Whether this source and `other` take a place of the same standings.
    pub fn overlaps(&self, other: &Source) -> bool {
        self.of == other.of
            && !self.is_empty()
            && !other.is_empty()
            && self.to.is_none_or(|to| other.from <= to)
            && other.to.is_none_or(|to| self.from <= to)
    }

    /// The number of places this source takes of `available`, the number of places there are.
    pub fn count(&self, available: usize) -> usize {
        let to = self
            .to
            .map_or(available, |to| (to.get() as usize).min(available));
        (to + 1).saturating_sub(self.from.get() as usize)
    }
}

/// One entry of the field of a phase, the ordered list it starts with: the entrants of a row of a
/// source, and where they come from.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Qualifier {
    pub entrants: Vec1<EntrantId>,
    pub origin: Origin,
}

/// Where a [`Qualifier`] comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// The registration, at its place in the order of the registration.
    Registration { place: NonZeroU32 },
    /// A row of the standings of an earlier phase: its ranks in its division's [`Standings`] and
    /// in the overall standings of the phase.
    Phase {
        phase: PhaseId,
        division: usize,
        rank: NonZeroU32,
        unique_rank: NonZeroU32,
        overall_rank: NonZeroU32,
        overall_unique_rank: NonZeroU32,
    },
}

impl Origin {
    /// The place of the row in what its source takes places of: the registration or the overall
    /// standings of the phase.
    pub fn place(&self) -> NonZeroU32 {
        match *self {
            Origin::Registration { place } => place,
            Origin::Phase {
                overall_unique_rank,
                ..
            } => overall_unique_rank,
        }
    }
}

#[cfg(test)]
mod tests {
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::*;

    fn places(from: u32, to: Option<u32>) -> Source {
        Source::new(
            SourceOf::Registration,
            NonZeroU32::new(from).unwrap(),
            to.map(|to| NonZeroU32::new(to).unwrap()),
        )
    }

    #[test]
    fn ranges_overlap_where_they_share_a_place() {
        assert!(places(1, Some(8)).overlaps(&places(8, Some(9))));
        assert!(places(1, None).overlaps(&places(20, Some(30))));
        assert!(places(5, None).overlaps(&places(1, None)));
        assert!(!places(1, Some(8)).overlaps(&places(9, None)));
        assert!(!places(9, None).overlaps(&places(1, Some(8))));
    }

    #[test]
    fn an_empty_range_overlaps_nothing() {
        assert!(places(5, Some(4)).is_empty());
        assert!(!places(5, Some(4)).overlaps(&places(1, None)));
    }

    #[test]
    fn sources_of_different_standings_do_not_overlap() {
        let phase = Source::everyone(SourceOf::Phase(PhaseId::new(uuid::Uuid::nil())));
        assert!(!phase.overlaps(&Source::registration()));
    }

    #[test]
    fn a_source_counts_the_places_it_takes_of_those_there_are() {
        assert_eq!(places(1, Some(8)).count(12), 8);
        assert_eq!(places(9, None).count(12), 4);
        assert_eq!(places(9, Some(16)).count(12), 4);
        assert_eq!(places(13, None).count(12), 0);
        assert_eq!(places(5, Some(4)).count(12), 0);
    }
}
