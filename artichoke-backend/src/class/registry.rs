//! A registry for [class specs](Spec) that uses types as keys.
//!
//! This data structure is used for associating data type metadata with a Rust
//! type which can be used to ensure the lifetime of the associated metadata.
//!
//! # Example: `mrb_data_type`
//!
//! In the mruby C API, custom data types define a `mrb_data_type` struct which
//! contains the custom data type's class name and free function. The C API
//! requires that this struct live at least as long as the `mrb_state`.
//! Typically, the `mrb_data_type` is `static`.
//!
//! ```c
//! static const struct mrb_data_type mrb_time_type = { "Time", mrb_free };
//! ```
//!
//! The registry resembles an append-only [`HashMap`].
//!
//! The registry stores class specs behind a [`Box`] pointer to ensure pointers
//! to the interior of the spec, like the [`CString`](std::ffi::CString) class
//! name, are not invalidated as the underlying storage reallocates.

use std::any::{self, Any, TypeId};
use std::collections::hash_map::{RandomState, Values};
use std::collections::HashMap;
use std::hash::BuildHasher;
use std::iter::FusedIterator;

use crate::class::Spec;

/// An iterator of all [class specs](Spec) stored in the [`Registry`].
#[derive(Debug, Clone)]
pub struct ClassSpecs<'a>(Values<'a, TypeId, Box<Spec>>);

impl<'a> ExactSizeIterator for ClassSpecs<'a> {}

impl<'a> FusedIterator for ClassSpecs<'a> {}

impl<'a> Iterator for ClassSpecs<'a> {
    type Item = &'a Spec;

    fn next(&mut self) -> Option<Self::Item> {
        let value = self.0.next()?;
        Some(value.as_ref())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }

    fn count(self) -> usize {
        self.0.count()
    }
}

/// A registry for [Class specs](crate::class::Spec) that uses types as keys.
///
/// This data structure is used for associating data type metadata with a Rust
/// type which can be used to ensure the lifetime of the associated metadata.
///
/// # Example: `mrb_data_type`
///
/// In the mruby C API, custom data types define a `mrb_data_type` struct which
/// contains the custom data type's class name and free function. The C API
/// requires that this struct live at least as long as the `mrb_state`.
/// Typically, the `mrb_data_type` is `static`.
///
/// ```c
/// static const struct mrb_data_type mrb_time_type = { "Time", mrb_free };
/// ```
///
/// The registry resembles an append-only [`HashMap`].
///
/// The registry stores Class specs behind a [`Box`] pointer to ensure pointers
/// to the interior of the spec, like the [`CString`](std::ffi::CString) class
/// name, are not invalidated as the underlying storage reallocates.
#[derive(Default, Debug)]
pub struct Registry<S = RandomState>(HashMap<TypeId, Box<Spec>, S>);

impl<S> PartialEq for Registry<S>
where
    S: BuildHasher,
{
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<S> Eq for Registry<S> where S: BuildHasher {}

impl<'a, S> IntoIterator for &'a Registry<S> {
    type Item = &'a Spec;
    type IntoIter = ClassSpecs<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.class_specs()
    }
}

impl Registry<RandomState> {
    /// Construct a new, empty `Registry`.
    #[must_use]
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    /// Construct a new `Registry` with the given `capacity`.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self(HashMap::with_capacity(capacity))
    }
}

impl<S> Registry<S> {
    /// Construct a new `Registry` with the given `hash_builder`.
    #[must_use]
    pub fn with_hasher(hash_builder: S) -> Self {
        Self(HashMap::with_hasher(hash_builder))
    }

    /// Construct a new `Registry` with the given `capacity` and `hash_builder`.
    #[must_use]
    pub fn with_capacity_and_hasher(capacity: usize, hash_builder: S) -> Self {
        Self(HashMap::with_capacity_and_hasher(capacity, hash_builder))
    }

    /// Returns the number of [class specs](Spec) the registry can hold without
    /// reallocating.
    ///
    /// This number is a lower bound; the `Registry` might be able to hold more,
    /// but is guaranteed to be able to hold at least this many.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }

    /// An iterator of all [class specs](Spec) stored in the [`Registry`] in
    /// arbitrary order.
    #[must_use]
    pub fn class_specs(&self) -> ClassSpecs<'_> {
        ClassSpecs(self.0.values())
    }

    /// Returns the number of [class specs](Spec) in the registry.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns true if the registry contains [class specs](Spec).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns a reference to the registry's [`BuildHasher`].
    #[must_use]
    pub fn hasher(&self) -> &S {
        self.0.hasher()
    }
}

impl<S> Registry<S>
where
    S: BuildHasher,
{
    /// Returns true if the registry contains a [class spec](Spec) for the
    /// specified type.
    #[must_use]
    pub fn contains<K>(&self) -> bool
    where
        K: Any,
    {
        let key = TypeId::of::<K>();
        self.0.contains_key(&key)
    }

    /// Inserts a type-[class spec](Spec) pair into the registry.
    ///
    /// This operation will only succeed if `K` has never been inserted into the
    /// registry.
    ///
    /// # Panics
    ///
    /// If `insert` has previously been called with type `K`, this function will
    /// panic. [`Registry`] is append-only and does not allow mutations.
    pub fn insert<K>(&mut self, spec: Box<Spec>)
    where
        K: Any,
    {
        let key = TypeId::of::<K>();
        if let Some(old_spec) = self.0.insert(key, spec) {
            panic!(
                "Attempted duplicate insert of {}. Registry is append-only. Previous spec: {:?}",
                any::type_name::<K>(),
                old_spec
            );
        }
    }

    /// Returns a reference to the [class spec](Spec) corresponding to the type
    /// key.
    ///
    /// If the type `K` has not been registered, [`None`] is returned.
    #[must_use]
    pub fn get<K>(&self) -> Option<&Spec>
    where
        K: Any,
    {
        let key = TypeId::of::<K>();
        let value = self.0.get(&key)?;
        Some(value.as_ref())
    }

    /// Reserves `capacity` for at least additional more elements to be inserted
    /// in the `Registry`. The collection may reserve more space to avoid
    /// frequent reallocations.
    pub fn reserve(&mut self, additional: usize) {
        self.0.reserve(additional);
    }

    /// Shrinks the capacity of the map as much as possible. It will drop down
    /// as much as possible while maintaining the internal rules and possibly
    /// leaving some space in accordance with the resize policy.
    pub fn shrink_to_fit(&mut self) {
        self.0.shrink_to_fit();
    }
}
