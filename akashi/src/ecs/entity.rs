//! General game objects to which [`Components`](Component) are attached.

use super::component::{Component, ComponentManager, TypeNotFoundError};
use crate::snowflake::Snowflake;
use crate::util::Result;

use std::any;
use std::any::TypeId;
use std::collections::HashSet;
use std::fmt;
use std::result;

use failure::{Error, Fail};

/// Represents an Entity within Akashi's Entity-Component-System
/// architecture.
///
/// Entities, on their own, are essentially just a unique ID and a
/// collection of attached Components.
///
/// The two main Entity types that Akashi provides are [`Players`](crate::Player) and
/// [`Cards`](crate::Card).
///
/// # Errors
///
/// Many of the methods provided by this trait wrap similar methods on
/// [`ComponentManager`] objects, which in turn wrap a
/// number of [`Component`] storage objects.
/// Errors reported by those will bubble up through these methods.
///
/// Additionally, attempts to perform operations with [`Component`]
/// types for which no backing store has been registered with
/// [`ComponentManager::register_component`](ComponentManager::register_component)
/// will return [`TypeNotFoundError`](TypeNotFoundError).
pub trait Entity: Sized + 'static {
    /// Gets the unique ID used to identify this Entity and its
    /// Components.
    fn id(&self) -> Snowflake;

    /// Gets a reference to the [`ComponentManager`]
    /// used to perform operations on this Entity.
    fn component_manager(&self) -> &ComponentManager<Self>;

    /// Gets a reference to a `HashSet` containing the `TypeId`s of each
    /// [`Component`] attached to this Entity.
    fn components_attached(&self) -> &HashSet<TypeId>;

    /// Gets a mutable reference to the `HashSet` of attached component
    /// `TypeId`s.
    fn components_attached_mut(&mut self) -> &mut HashSet<TypeId>;

    /// Gets a [`Component`] attached to this Entity.
    fn get_component<T: Component<Self> + 'static>(&self) -> Result<Option<T>> {
        if !self.components_attached().contains(&TypeId::of::<T>()) {
            if !self.component_manager().is_registered::<T>() {
                Err(TypeNotFoundError::new(any::type_name::<T>().to_owned()).into())
            } else {
                Ok(None)
            }
        } else {
            self.component_manager().get_component::<T>(&self)
        }
    }

    /// Attaches a [`Component`] to this Entity, or updates an already-attached
    /// [`Component`].
    fn set_component<T: Component<Self> + 'static>(&mut self, component: T) -> Result<()> {
        self.component_manager()
            .set_component::<T>(&self, component)
            .map(|_v| {
                self.components_attached_mut().insert(TypeId::of::<T>());
            })
    }

    /// Checks to see if the given [`Component`] type has
    /// been attached to this Entity.
    ///
    /// Unlike most of the other `Entity` trait methods, this doesn't
    /// return a [`TypeNotFoundError`] for [`Components`](Component)
    /// without an associated backing store. Instead, it will just return
    /// `false`.
    fn has_component<T: Component<Self> + 'static>(&self) -> bool {
        self.components_attached().contains(&TypeId::of::<T>())
    }

    /// Deletes an attached [`Component`] from this Entity.
    fn delete_component<T: Component<Self> + 'static>(&mut self) -> Result<()> {
        self.components_attached_mut().remove(&TypeId::of::<T>());
        self.component_manager().delete_component::<T>(&self)
    }

    /// Delete all [`Components`](Component) attached to this Entity.
    ///
    /// # Errors
    ///
    /// Any errors reported by the backing storage objects for
    /// [`Components`](Component) attached to this Entity will be
    /// collected into a [`ClearComponentsError`].
    fn clear_components(&mut self) -> result::Result<(), ClearComponentsError> {
        let mut err = ClearComponentsError::new();
        for type_id in self.components_attached().iter() {
            if let Err(e) = self
                .component_manager()
                .delete_component_by_id(&self, type_id)
            {
                err.push(e);
            }
        }

        self.components_attached_mut().clear();

        if err.len() > 0 {
            Err(err)
        } else {
            Ok(())
        }
    }
}

/// This failure type collects errors from [`Entity::clear_components`].
#[derive(Fail, Debug)]
pub struct ClearComponentsError {
    errors: Vec<Error>,
}

impl fmt::Display for ClearComponentsError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "failed to clear components due to errors:\n")?;
        for err in self.errors.iter() {
            err.fmt(f)?;
        }

        Ok(())
    }
}

impl ClearComponentsError {
    pub fn new() -> ClearComponentsError {
        ClearComponentsError { errors: Vec::new() }
    }

    pub fn push(&mut self, err: Error) {
        self.errors.push(err);
    }

    pub fn len(&self) -> usize {
        self.errors.len()
    }
}
