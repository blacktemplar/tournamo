use std::num::NonZeroU32;

use serde::{Deserialize, Serialize};
use uuid::{Uuid, uuid};

/// Namespaces for various deterministic ids
const NAMESPACE_MATCH: Uuid = uuid!("d4098937-85f4-49ce-945a-be8ae74da780");
const NAMESPACE_RANKING: Uuid = uuid!("eb5e6d16-15af-42e1-b0f4-f5a643fd1e9a");
const NAMESPACE_BYE: Uuid = uuid!("5b0f3c1e-2d8a-4c67-9e41-7a3f6d92b8c5");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntrantId(Uuid);

impl EntrantId {
    pub fn new(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub(super) fn uuid(self) -> Uuid {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PhaseId(Uuid);

impl PhaseId {
    pub fn new(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RankingId(Uuid);

impl RankingId {
    /// The id of the ranking of `entrants`.
    pub(super) fn derive<'a>(entrants: impl IntoIterator<Item = &'a EntrantId>) -> Self {
        Self(derive_id(
            &NAMESPACE_RANKING,
            entrants.into_iter().map(|e| e.0),
            None,
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MatchId(Uuid);

impl MatchId {
    /// The id of the match of the rankings `teams` (team A's, then team B's) in `round`.
    pub(super) fn derive<'a>(
        teams: impl IntoIterator<Item = &'a RankingId>,
        round: NonZeroU32,
    ) -> Self {
        Self(derive_id(
            &NAMESPACE_MATCH,
            teams.into_iter().map(|ranking| ranking.0),
            Some(round),
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ByeId(Uuid);

impl ByeId {
    /// The id of the bye of `ranking` in `round`.
    pub(super) fn derive(ranking: RankingId, round: NonZeroU32) -> Self {
        Self(derive_id(&NAMESPACE_BYE, [ranking.0], Some(round)))
    }
}

/// The UUIDv5 in `namespace` of the bytes of `uuids`, followed by those of `round` if given.
fn derive_id(
    namespace: &Uuid,
    uuids: impl IntoIterator<Item = Uuid>,
    round: Option<NonZeroU32>,
) -> Uuid {
    let bytes: Vec<u8> = uuids
        .into_iter()
        .flat_map(Uuid::into_bytes)
        .chain(
            round
                .into_iter()
                .flat_map(|round| round.get().to_le_bytes()),
        )
        .collect();
    Uuid::new_v5(namespace, &bytes)
}
