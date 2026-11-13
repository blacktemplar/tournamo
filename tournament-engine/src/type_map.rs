use std::{
    any::{Any, TypeId},
    fmt::Debug,
};

use dyn_clone::DynClone;
use serde::{Deserialize, Serialize};

use crate::map::{IdAble, IdMap};

/// A value that can be stored in a [`TypeMap`], keyed by its own concrete type.
///
/// An implementor needs an explicit, namespaced `typetag` name, which is a persistence contract:
/// renaming or moving the type must not change it.
// `DynClone` is required because a transition carrying such a value has to clone it on every
// replay of the log: `Transition::apply` only takes `&self`.
#[typetag::serde(tag = "type", content = "value")]
pub trait TypeMapValue: Any + Debug + DynClone {}

dyn_clone::clone_trait_object!(TypeMapValue);

/// Downcasts `value` to `&T` without checking that its concrete type really is `T`.
///
/// Prefer this over `value.downcast_ref::<T>().unwrap_unchecked()` in hot paths.
///
/// # Safety
/// The concrete type of `value` must be `T`.
// `unwrap_unchecked` does let LLVM drop the `TypeId` comparison, but not the virtual
// `Any::type_id` call producing the `TypeId`: an indirect call through the `dyn Any` vtable is not
// known to be side effect free. Casting the fat pointer to a thin one never touches the vtable.
// Replaceable by `Any::downcast_unchecked_ref` once that is stable (feature `downcast_unchecked`).
pub(crate) unsafe fn downcast_unchecked_ref<T: 'static>(value: &dyn Any) -> &T {
    debug_assert!(value.is::<T>());
    // SAFETY: the caller guarantees that `value` has concrete type `T`, so the data pointer of the
    // fat pointer points to a valid and properly aligned `T`. Casting to a thin pointer discards
    // the vtable and keeps exactly that data pointer.
    unsafe { &*(value as *const dyn Any).cast::<T>() }
}

/// Downcasts `value` to `&mut T` without checking that its concrete type really is `T`.
///
/// Prefer this over `value.downcast_mut::<T>().unwrap_unchecked()` in hot paths, see
/// [`downcast_unchecked_ref`].
///
/// # Safety
/// The concrete type of `value` must be `T`.
// Replaceable by `Any::downcast_unchecked_mut` once that is stable (feature `downcast_unchecked`).
unsafe fn downcast_unchecked_mut<T: 'static>(value: &mut dyn Any) -> &mut T {
    debug_assert!(value.is::<T>());
    // SAFETY: as in `downcast_unchecked_ref`, and `value` is a unique borrow, so the resulting
    // `&mut T` is the only reference to the value.
    unsafe { &mut *(value as *mut dyn Any).cast::<T>() }
}

/// A map of values keyed by their own Rust type, holding at most one value per type.
///
/// A key is a [`TypeId`] rather than a name, so a lookup always gets back a value of the type it
/// asked for. This carries the metrics of a [`Ranking`](crate::entities::Ranking) and
/// the consumer annotations of an [`Entrant`](crate::entities::Entrant).
///
/// Serializes as a plain sequence of its values, in insertion order.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TypeMap(IdMap<Box<dyn TypeMapValue>>);

/// Keys a value by its concrete type, which is what lets [`TypeMap`] downcast without checking.
impl IdAble for Box<dyn TypeMapValue> {
    type Id = TypeId;

    #[inline]
    fn id(&self) -> TypeId {
        // `**self` rather than `self`: the type id of the boxed value, not of the box
        (**self).type_id()
    }
}

impl TypeMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts `value`, replacing the value of the same type if there is one.
    ///
    /// A replaced value keeps the position of the one it replaces, so replacing never reorders the
    /// map.
    pub fn insert<T: TypeMapValue>(&mut self, value: T) {
        self.0.insert(Box::new(value));
    }

    /// Returns this map with `value` inserted, see [`insert`](Self::insert).
    ///
    /// # Examples
    /// ```
    /// # use tournament_engine::{Score, TypeMap, plugins::metrics_calculator::Points};
    /// let map = TypeMap::new()
    ///     .with(Points(Score::new(1.0)))
    ///     .with(Points(Score::new(2.0)));
    ///
    /// assert_eq!(map.get::<Points>(), Some(&Points(Score::new(2.0))));
    /// ```
    pub fn with<T: TypeMapValue>(mut self, value: T) -> Self {
        self.insert(value);
        self
    }

    /// Like [`insert`](Self::insert) for an already boxed value, returning the value it replaced.
    pub fn insert_boxed(&mut self, value: Box<dyn TypeMapValue>) -> Option<Box<dyn TypeMapValue>> {
        self.0.insert(value)
    }

    /// The value of type `T`, or [`None`] if the map holds none.
    pub fn get<T: 'static>(&self) -> Option<&T> {
        self.0.get(&TypeId::of::<T>()).map(|boxed| {
            let any: &dyn Any = boxed.as_ref();
            // SAFETY: an `IdMap` stores every value under its `IdAble::id`, which for a
            // `Box<dyn TypeMapValue>` is the type id of its concrete type, so the value at key
            // `TypeId::of::<T>()` is a `T`.
            unsafe { downcast_unchecked_ref::<T>(any) }
        })
    }

    /// A mutable reference to the value of type `T`, or [`None`] if the map holds none.
    pub fn get_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.0.get_mut(&TypeId::of::<T>()).map(|boxed| {
            let any: &mut dyn Any = boxed.as_mut();
            // SAFETY: as in `get`.
            unsafe { downcast_unchecked_mut::<T>(any) }
        })
    }
}

#[cfg(test)]
mod tests;
