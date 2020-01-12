//! Storage systems that work entirely in-memory, for testing and prototyping
//! use.

use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::{Arc, RwLock};

use failure::format_err;

use crate::ecs::{Component, ComponentBackend, ComponentManager, Entity, EntityBackend};
use crate::snowflake::Snowflake;
use crate::util::Result;

/// In-memory [`Entity`] storage backend.
///
/// This is mainly meant for use in testing and for prototyping. It has
/// no provisions for storing data to a persistent medium.
pub struct LocalEntityStorage<T: Entity + Clone + 'static> {
    data: RwLock<HashMap<Snowflake, T>>,
}

impl<T> LocalEntityStorage<T>
where
    T: Entity + Clone + 'static,
{
    pub fn new() -> LocalEntityStorage<T> {
        LocalEntityStorage {
            data: RwLock::new(HashMap::new()),
        }
    }
}

impl<T> EntityBackend<T> for LocalEntityStorage<T>
where
    T: Entity + Clone + 'static,
{
    fn exists(&self, id: Snowflake) -> Result<bool> {
        let data = self.data.read().unwrap();
        Ok(data.contains_key(&id))
    }

    fn load(&self, id: Snowflake, _cm: Arc<ComponentManager<T>>) -> Result<Option<T>> {
        let data = self.data.read().unwrap();
        Ok(data.get(&id).map(|v| v.clone()))
    }

    fn store(&self, id: Snowflake, obj: &T) -> Result<()> {
        let mut data = self.data.write().unwrap();
        data.insert(id, obj.clone());
        Ok(())
    }

    fn delete(&self, id: Snowflake) -> Result<()> {
        let mut data = self.data.write().unwrap();
        data.remove(&id);
        Ok(())
    }

    fn keys(&self, page: u64, limit: u64) -> Result<Vec<Snowflake>> {
        let mut ids: Vec<Snowflake>;

        let data = self.data.read().unwrap();
        ids = data.keys().copied().collect();
        drop(data);

        ids.sort_unstable();
        Ok(ids
            .into_iter()
            .skip((page * limit) as usize)
            .take(limit as usize)
            .collect())
    }
}

/// In-memory [`Component`] storage backend.
///
/// This is mainly meant for use in testing and for prototyping. It has
/// no provisions for storing data to a persistent medium.
pub struct LocalComponentStorage<T, U>
where
    T: Entity + 'static,
    U: Component<T> + Clone + 'static,
{
    data: RwLock<HashMap<Snowflake, U>>,
    pd: PhantomData<T>,
}

impl<T, U> LocalComponentStorage<T, U>
where
    T: Entity + 'static,
    U: Component<T> + Clone + 'static,
{
    pub fn new() -> LocalComponentStorage<T, U> {
        LocalComponentStorage {
            data: RwLock::new(HashMap::new()),
            pd: PhantomData,
        }
    }
}

impl<T, U> ComponentBackend<T, U> for LocalComponentStorage<T, U>
where
    T: Entity + 'static,
    U: Component<T> + Clone + 'static,
{
    fn load(&self, entity: &T) -> Result<Option<U>> {
        let data_map = self
            .data
            .read()
            .map_err(|_e| format_err!("storage lock poisoned"))?;
        Ok(data_map.get(&entity.id()).map(|x| x.clone()))
    }

    fn store(&self, entity: &T, component: U) -> Result<()> {
        let mut data_map = self
            .data
            .write()
            .map_err(|_e| format_err!("storage lock poisoned"))?;
        data_map.insert(entity.id(), component);
        Ok(())
    }

    fn exists(&self, entity: &T) -> Result<bool> {
        let data_map = self
            .data
            .read()
            .map_err(|_e| format_err!("storage lock poisoned"))?;
        Ok(data_map.contains_key(&entity.id()))
    }

    fn delete(&self, entity: &T) -> Result<()> {
        let mut data_map = self
            .data
            .write()
            .map_err(|_e| format_err!("storage lock poisoned"))?;
        data_map.remove(&entity.id());
        Ok(())
    }
}
