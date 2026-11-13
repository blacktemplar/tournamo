use crate::type_map::{TypeMap, TypeMapValue};
#[cfg(doc)]
use crate::{Competition, Entrant, Match};

/// An entity carrying annotations: data about it keyed by type, such as the display name of an
/// [`Entrant`] or the knockout stage of a [`Match`].
///
/// Annotations are given while building the entity, before the transition that creates it. Once it
/// is part of a [`Competition`] they cannot be changed, and they never take part in its id.
///
/// # Examples
/// ```
/// # use serde::{Deserialize, Serialize};
/// # use tournament_engine::{Annotated, Entrant, EntrantId, TypeMapValue, uuid::Uuid};
/// #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
/// struct DatabaseId(u64);
///
/// #[typetag::serde(name = "example::database_id")]
/// impl TypeMapValue for DatabaseId {}
///
/// let entrant = Entrant::new(EntrantId::new(Uuid::nil())).with_annotation(DatabaseId(17));
///
/// assert_eq!(entrant.annotation::<DatabaseId>(), Some(&DatabaseId(17)));
/// ```
pub trait Annotated: Sized {
    /// All annotations of this entity.
    fn annotations(&self) -> &TypeMap;

    /// All annotations of this entity, for building it.
    fn annotations_mut(&mut self) -> &mut TypeMap;

    /// The annotation of type `T`, if any.
    fn annotation<T: 'static>(&self) -> Option<&T> {
        self.annotations().get::<T>()
    }

    /// Returns this entity with `annotation` added, replacing an annotation of the same type.
    fn with_annotation<T: TypeMapValue>(mut self, annotation: T) -> Self {
        self.annotations_mut().insert(annotation);
        self
    }

    /// Returns this entity with `annotations` as its annotations, replacing any it had.
    fn with_annotations(mut self, annotations: TypeMap) -> Self {
        *self.annotations_mut() = annotations;
        self
    }
}
