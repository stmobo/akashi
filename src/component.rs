use std::any;
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt;
use std::sync::{Weak, Arc, Mutex, RwLock};
use std::result;

extern crate downcast_rs;
use downcast_rs::DowncastSync;

use failure::Fail;

use crate::player::Player;
use crate::card::{Card, Inventory};
use crate::snowflake::Snowflake;

pub trait Component: DowncastSync + Sync + Send + fmt::Debug {}
downcast_rs::impl_downcast!(sync Component);

pub type Result<T> = result::Result<T, Box<dyn Fail>>;

pub trait ComponentStore<T: Component + 'static> {
    fn load(&self, entity_id: Snowflake) -> Result<Option<T>>;
    fn store(&self, entity_id: Snowflake, component: T) -> Result<()>;
    fn exists(&self, entity_id: Snowflake) -> Result<bool>;
    fn delete(&self, entity_id: Snowflake) -> Result<()>;
}

type ComponentLoadFn = Box<dyn Fn(Snowflake) -> Result<Option<Box<dyn Component + 'static>>> + Sync + Send>; 
type ComponentStoreFn = Box<dyn Fn(Snowflake, Box<dyn Component + 'static>) -> Result<()> + Sync + Send>;
type ComponentExistsFn = Box<dyn Fn(Snowflake) -> Result<bool> + Sync + Send>;
type ComponentDeleteFn = Box<dyn Fn(Snowflake) -> Result<()> + Sync + Send>;

pub struct ComponentTypeData {
    load: ComponentLoadFn, 
    store: ComponentStoreFn,
    exists: ComponentExistsFn,
    delete: ComponentDeleteFn,
}

impl fmt::Debug for ComponentTypeData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // TODO: is there a better way to format this?
        write!(f, "<ComponentTypeData>")
    }
}

impl ComponentTypeData {
    pub fn new<T, U>(store: U) -> ComponentTypeData
    where
        T: Component + 'static,
        U: ComponentStore<T> + Sync + Send + 'static,
    {
        let s1 = Arc::new(store);
        let s2 = s1.clone();
        let s3 = s1.clone();
        let s4 = s1.clone();

        ComponentTypeData {
            load: Box::new(move |ent_id: Snowflake| {
                let res = s1.load(ent_id)?;
                if let Some(val) = res {
                    Ok(Some(Box::new(val)))
                } else {
                    Ok(None)
                }
            }),
            store: Box::new(move |ent_id: Snowflake, c: Box<dyn Component + 'static>| -> Result<()>{
                let res = c.downcast::<T>();
                if let Ok(val) = res {
                    s2.store(ent_id, *val)
                } else {
                    Err(Box::new(DowncastError { component_name: any::type_name::<T>() }))
                }
            }),
            exists: Box::new(move |ent_id: Snowflake| s3.exists(ent_id)),
            delete: Box::new(move |ent_id: Snowflake| s4.delete(ent_id)),
        }
    }
}

#[derive(Debug)]
pub struct ComponentManagerBuilder {
    component_types: HashMap<TypeId, ComponentTypeData>,
}

impl ComponentManagerBuilder {
    pub fn register_component<T, U>(&mut self, store: U)
    where
        T: Component + 'static,
        U: ComponentStore<T> + Sync + Send + 'static
    {
        self.component_types.insert(
            TypeId::of::<T>(),
            ComponentTypeData::new(store)
        );
    }

    pub fn finish(self) -> ComponentManager {
        ComponentManager { component_types: self.component_types }
    }
}

#[derive(Debug)]
pub struct ComponentManager {
    component_types: HashMap<TypeId, ComponentTypeData>,
}

impl ComponentManager {
    pub fn build() -> ComponentManagerBuilder {
        ComponentManagerBuilder { component_types: HashMap::new() }
    }

    fn set_component<T: Component + 'static>(&self, entity_id: Snowflake, component: T) -> Result<()> {
        if let Some(data) = self.component_types.get(&TypeId::of::<T>()) {
            (data.store)(entity_id, Box::new(component))
        } else {
            Err(Box::new(TypeNotFoundError { component_name: any::type_name::<T>().to_owned() }))
        }
    }

    fn get_component<T: Component + 'static>(&self, entity_id: Snowflake) -> Result<Option<T>> {
        if let Some(data) = self.component_types.get(&TypeId::of::<T>()) {
            if let Some(comp) = (data.load)(entity_id)? {
                // if this downcast fails, the loader was written wrong
                let boxed = comp.downcast::<T>().expect("Failed to downcast component from loader");
                Ok(Some(*boxed))
            } else {
                Ok(None)
            }
        } else {
            Err(Box::new(TypeNotFoundError { component_name: any::type_name::<T>().to_owned() }))
        }
    }

    fn delete_component<T: Component + 'static>(&self, entity_id: Snowflake) -> Result<()> {
        if let Some(data) = self.component_types.get(&TypeId::of::<T>()) {
            (data.delete)(entity_id)
        } else {
            Err(Box::new(TypeNotFoundError { component_name: any::type_name::<T>().to_owned() }))
        }
    }

    fn component_exists<T: Component + 'static>(&self, entity_id: Snowflake) -> Result<bool> {
        if let Some(data) = self.component_types.get(&TypeId::of::<T>()) {
            (data.exists)(entity_id)
        } else {
            Err(Box::new(TypeNotFoundError { component_name: any::type_name::<T>().to_owned() }))
        }
    }
}

#[derive(Fail, Debug)]
#[fail(display = "No handlers registered for Components of type {}", component_name)]
struct TypeNotFoundError {
    component_name: String,
}

#[derive(Fail, Debug)]
#[fail(display = "Failed to downcast to type {}", component_name)]
struct DowncastError {
    component_name: &'static str,
}

pub trait ComponentsAttached {
    fn get_component<T: Component + 'static>(&self) -> Result<Option<T>>;
    fn set_component<T: Component + 'static>(&self, component: T) -> Result<()>;
    fn has_component<T: Component + 'static>(&self) -> Result<bool>;
    fn delete_component<T: Component + 'static>(&self) -> Result<()>;
}

impl ComponentsAttached for Player {
    fn get_component<T: Component + 'static>(&self) -> Result<Option<T>> {
        let cm = self.component_manager();
        cm.get_component::<T>(self.id())
    }

    fn set_component<T: Component + 'static>(&self, component: T) -> Result<()> {
        let cm = self.component_manager();
        cm.set_component::<T>(self.id(), component)
    }

    fn has_component<T: Component + 'static>(&self) -> Result<bool> {
        let cm = self.component_manager();
        cm.component_exists::<T>(self.id())
    }

    fn delete_component<T: Component + 'static>(&self) -> Result<()> {
        let cm = self.component_manager();
        cm.delete_component::<T>(self.id())
    }
}

