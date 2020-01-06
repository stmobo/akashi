use super::component_store::{ComponentStore, ComponentTypeData};
use super::entity::Entity;
use crate::util::Result;

use std::any;
use std::any::TypeId;
use std::collections::HashMap;

use downcast_rs::Downcast;
use failure::Fail;

pub trait Component<T>: Downcast + Sync + Send {}
downcast_rs::impl_downcast!(Component<T>);

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

    pub fn register_component<U, V>(&mut self, store: V)
    where
        U: Component<T> + 'static,
        V: ComponentStore<T, U> + Sync + Send + 'static,
    {
        self.component_types
            .insert(TypeId::of::<U>(), ComponentTypeData::new(store));
    }

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
