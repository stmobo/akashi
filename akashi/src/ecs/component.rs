//! The internals of the `Entity`-`Component` attachment system.

use super::component_store::{ComponentStore, ComponentTypeData};
use super::entity::Entity;
use crate::util::Result;

use std::any;
use std::any::TypeId;
use std::collections::HashMap;

use downcast_rs::Downcast;
use failure::Fail;

/// Represents a Component within Akashi's Entity-Component-System
/// architecture.
///
/// Components are used to hold data and provide functionality for
/// Entities, such as Players and Cards.
///
/// This trait doesn't provide anything on its own, but it does
/// allow a type to interact with the rest of the ECS code.
pub trait Component<T>: Downcast + Sync + Send {}
downcast_rs::impl_downcast!(Component<T>);

/// Manages operations related to `Component`s, such as saving and loading
/// `Component` data.
///
/// Typically, you won't need to call any methods on `ComponentManager`
/// objects aside from `register_component`, since the corresponding
/// Entity trait methods will do so for you.
///
/// # Errors
///
/// Most of the methods on this object ultimately end up
/// wrapping methods on registered `Component` storage objects.
/// Errors returned from those methods will be passed through by
/// methods on `ComponentManager`.
///
/// Additionally, attempts to perform operations with `Component` types
/// for which no backing store has been registered with `register_component`
/// will return `TypeNotFoundError`s.
#[derive(Debug)]
pub struct ComponentManager<T: Entity + 'static> {
    component_types: HashMap<TypeId, ComponentTypeData<T>>,
}

impl<T: Entity + 'static> ComponentManager<T> {
    pub fn new() -> ComponentManager<T> {
        ComponentManager {
            component_types: HashMap::new(),
        }
    }

    /// Registers a backing storage object for a `Component` type.
    ///
    /// This registers a backing store and associated functions for
    /// a `Component` type, allowing Entities that use this manager
    /// to get/set Component data of that type.
    pub fn register_component<U, V>(&mut self, store: V)
    where
        U: Component<T> + 'static,
        V: ComponentStore<T, U> + Sync + Send + 'static,
    {
        self.component_types
            .insert(TypeId::of::<U>(), ComponentTypeData::new(store));
    }

    /// Check to see if a particular `Component` type has registered
    /// operations.
    pub fn is_registered<U: Component<T> + 'static>(&self) -> bool {
        self.component_types.contains_key(&TypeId::of::<U>())
    }

    /// Save data for a `Component` to the appropriate backing store.
    pub fn set_component<U: Component<T> + 'static>(&self, entity: &T, component: U) -> Result<()> {
        if let Some(data) = self.component_types.get(&TypeId::of::<U>()) {
            (data.store)(entity, Box::new(component))
        } else {
            Err(TypeNotFoundError {
                component_name: any::type_name::<U>().to_owned(),
            }
            .into())
        }
    }

    /// Load data for a `Component` from the appropriate backing store.
    pub fn get_component<U: Component<T> + 'static>(&self, entity: &T) -> Result<Option<U>> {
        if let Some(data) = self.component_types.get(&TypeId::of::<U>()) {
            if let Some(comp) = (data.load)(entity)? {
                // if this downcast fails, the loader was written wrong
                let boxed = match comp.downcast::<U>() {
                    Ok(v) => v,
                    Err(_e) => panic!("Failed to downcast component from loader"),
                };
                Ok(Some(*boxed))
            } else {
                Ok(None)
            }
        } else {
            Err(TypeNotFoundError {
                component_name: any::type_name::<U>().to_owned(),
            }
            .into())
        }
    }

    /// Delete the data for an attached `Component` from its registered
    /// backing store.
    pub fn delete_component<U: Component<T> + 'static>(&self, entity: &T) -> Result<()> {
        if let Some(data) = self.component_types.get(&TypeId::of::<U>()) {
            (data.delete)(entity)
        } else {
            Err(TypeNotFoundError {
                component_name: any::type_name::<U>().to_owned(),
            }
            .into())
        }
    }

    /// Delete the data for a `Component` with the associated `TypeId`.
    ///
    /// This should probably only be used internally.
    pub fn delete_component_by_id(&self, entity: &T, type_id: &TypeId) -> Result<()> {
        if let Some(data) = self.component_types.get(&type_id) {
            (data.delete)(entity)
        } else {
            Err(TypeNotFoundError {
                component_name: format!("{:?}", type_id),
            }
            .into())
        }
    }

    /// Check to see if associated `Component` data exists for the given
    /// entity and Component type.
    pub fn component_exists<U: Component<T> + 'static>(&self, entity: &T) -> Result<bool> {
        if let Some(data) = self.component_types.get(&TypeId::of::<U>()) {
            (data.exists)(entity)
        } else {
            Err(TypeNotFoundError {
                component_name: any::type_name::<U>().to_owned(),
            }
            .into())
        }
    }
}

#[derive(Fail, Debug)]
#[fail(
    display = "No handlers registered for Components of type {}",
    component_name
)]
pub struct TypeNotFoundError {
    component_name: String,
}

impl TypeNotFoundError {
    pub fn new(component_name: String) -> TypeNotFoundError {
        TypeNotFoundError { component_name }
    }
}
