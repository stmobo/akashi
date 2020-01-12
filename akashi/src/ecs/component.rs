//! The internals of the [`Entity`]-[`Component`] attachment system.

use super::component_store::{ComponentBackend, ComponentTypeData};
use super::entity::Entity;
use super::TypeNotFoundError;
use crate::util::Result;

use std::any;
use std::any::TypeId;
use std::collections::HashMap;
use std::fmt;

use failure::format_err;

use downcast_rs::{Downcast, DowncastSync};

/// Represents a Component within Akashi's Entity-Component-System
/// architecture.
///
/// Components are used to hold data and provide functionality for
/// [`Entities`](Entity), such as [`Players`](crate::Player) and
/// [`Cards`](crate::Card).
///
/// This trait doesn't provide anything on its own, but it does
/// allow a type to interact with the rest of the ECS code.
pub trait Component<T>: Downcast {}
downcast_rs::impl_downcast!(Component<T>);

/// Used as a helper for downcasting [`ComponentManagers`](ComponentManager).
///
/// You probably shouldn't use this yourself.
#[doc(hidden)]
pub trait ComponentManagerDowncast: DowncastSync + Sync + Send + fmt::Debug {}
downcast_rs::impl_downcast!(sync ComponentManagerDowncast);

impl<T: Entity + 'static> ComponentManagerDowncast for ComponentManager<T> {}

/// Manages operations related to [`Components`](Component), such as
/// saving and loading [`Component`] data.
///
/// Typically, you won't need to call any methods on `ComponentManager`
/// objects aside from [`register_component`](ComponentManager::register_component),
/// since the corresponding [`Entity`] trait methods will do so for you.
///
/// # Errors
///
/// Most of the methods on this object ultimately end up
/// wrapping methods on registered [`Component`] storage objects.
/// Errors returned from those methods will be passed through by
/// methods on `ComponentManager`.
///
/// Additionally, attempts to perform operations with [`Component`] types
/// for which no backing store has been registered with
/// [`register_component`](ComponentManager::register_component) will return
/// [`TypeNotFoundError`].
pub struct ComponentManager<T: Entity + 'static> {
    component_types: HashMap<TypeId, ComponentTypeData<T>>,
    component_names: HashMap<TypeId, String>,
    component_names_inv: HashMap<String, TypeId>,
}

impl<T: Entity + 'static> ComponentManager<T> {
    pub fn new() -> ComponentManager<T> {
        ComponentManager {
            component_types: HashMap::new(),
            component_names: HashMap::new(),
            component_names_inv: HashMap::new(),
        }
    }

    /// Registers a backing storage object and unique name for a
    /// [`Component`] type.
    ///
    /// This registers a backing store and associated functions for
    /// a [`Component`] type, allowing [`Entities`](Entity) that use this manager
    /// to get/set [`Component`] data of that type.
    pub fn register_component<U, V>(&mut self, name: &str, store: V) -> Result<()>
    where
        U: Component<T> + 'static,
        V: ComponentBackend<T, U> + Sync + Send + 'static,
    {
        if self.component_types.contains_key(&TypeId::of::<U>()) {
            return Err(format_err!(
                "component type already registered: {}",
                any::type_name::<U>()
            ));
        }

        self.component_types
            .insert(TypeId::of::<U>(), ComponentTypeData::new(store));

        self.component_names
            .insert(TypeId::of::<U>(), name.to_owned());

        self.component_names_inv
            .insert(name.to_owned(), TypeId::of::<U>());

        Ok(())
    }

    /// Check to see if a particular [`Component`] type has registered
    /// operations.
    pub fn is_registered<U: Component<T> + 'static>(&self) -> bool {
        self.component_types.contains_key(&TypeId::of::<U>())
    }

    pub fn component_name(&self, type_id: &TypeId) -> Option<&str> {
        self.component_names.get(type_id).map(|r| r.as_str())
    }

    pub fn component_type_id(&self, name: &str) -> Option<&TypeId> {
        self.component_names_inv.get(name)
    }

    /// Save data for a [`Component`] to the appropriate backing store.
    pub fn set_component<U: Component<T> + 'static>(&self, entity: &T, component: U) -> Result<()> {
        if let Some(data) = self.component_types.get(&TypeId::of::<U>()) {
            (data.store)(entity, Box::new(component))
        } else {
            Err(TypeNotFoundError::new(any::type_name::<U>().to_owned()).into())
        }
    }

    /// Load data for a [`Component`] from the appropriate backing store.
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
            Err(TypeNotFoundError::new(any::type_name::<U>().to_owned()).into())
        }
    }

    /// Delete the data for an attached [`Component`] from its registered
    /// backing store.
    pub fn delete_component<U: Component<T> + 'static>(&self, entity: &T) -> Result<()> {
        if let Some(data) = self.component_types.get(&TypeId::of::<U>()) {
            (data.delete)(entity)
        } else {
            Err(TypeNotFoundError::new(any::type_name::<U>().to_owned()).into())
        }
    }

    /// Delete the data for a [`Component`] with the associated `TypeId`.
    ///
    /// This should probably only be used internally.
    pub fn delete_component_by_id(&self, entity: &T, type_id: &TypeId) -> Result<()> {
        if let Some(data) = self.component_types.get(&type_id) {
            (data.delete)(entity)
        } else {
            Err(TypeNotFoundError::new(format!("{:?}", type_id)).into())
        }
    }

    /// Check to see if associated [`Component`] data exists for the given
    /// entity and Component type.
    pub fn component_exists<U: Component<T> + 'static>(&self, entity: &T) -> Result<bool> {
        if let Some(data) = self.component_types.get(&TypeId::of::<U>()) {
            (data.exists)(entity)
        } else {
            Err(TypeNotFoundError::new(any::type_name::<U>().to_owned()).into())
        }
    }
}

impl<T> fmt::Debug for ComponentManager<T>
where
    T: Entity + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ComponentManager<{}> {{ {} types }}",
            any::type_name::<T>(),
            self.component_types.len()
        )
    }
}
