use serde::{Deserialize, Serialize};

use crate::{Annotated, EntrantId, map::IdAble, type_map::TypeMap};

/// Someone taking part in a competition.
///
/// Beyond its [`EntrantId`] an entrant holds only `annotations`: everything a consumer wants to
/// show or store about it — a display name, a club, a database id — keyed by the consumer's own
/// type. Annotations are given once, by the
/// [`AddEntrant`](crate::transitions::add_entrant::AddEntrant) that creates the entrant, and cannot
/// be changed afterwards.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entrant {
    id: EntrantId,
    annotations: TypeMap,
}

impl IdAble for Entrant {
    type Id = EntrantId;

    fn id(&self) -> Self::Id {
        self.id
    }
}

impl Entrant {
    /// Creates an entrant without any annotations.
    pub fn new(id: EntrantId) -> Self {
        Self {
            id,
            annotations: TypeMap::new(),
        }
    }

    pub fn id(&self) -> EntrantId {
        self.id
    }
}

impl Annotated for Entrant {
    fn annotations(&self) -> &TypeMap {
        &self.annotations
    }

    fn annotations_mut(&mut self) -> &mut TypeMap {
        &mut self.annotations
    }
}
