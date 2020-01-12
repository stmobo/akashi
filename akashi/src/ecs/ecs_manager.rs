use super::component::ComponentManagerDowncast;
use super::entity_store::{
    EntityBackend, EntityStore, EntityStoreDowncast, EntityStoreDowncastHelper, ReadReference,
    StoreHandle, WriteReference,
};
use super::{Component, ComponentManager, Entity, Store, TypeNotFoundError};
use crate::snowflake::Snowflake;
use crate::util::Result;

use std::any;
use std::any::TypeId;
use std::collections::HashMap;
use std::sync::Arc;

pub struct EntityTypeData {
    store: Box<dyn EntityStoreDowncast>,
    component_manager: Arc<dyn ComponentManagerDowncast>,
}

pub struct ECSManager {
    types: HashMap<TypeId, EntityTypeData>,
}

impl ECSManager {
    fn get_type_data<'a, T>(
        &'a self,
    ) -> Option<(&'a (dyn EntityStore<T> + 'static), Arc<ComponentManager<T>>)>
    where
        T: Entity + Sync + Send + 'static,
    {
        let type_id = TypeId::of::<T>();
        let type_data = self.types.get(&type_id)?;

        let store_ref = type_data
            .store
            .downcast_ref::<EntityStoreDowncastHelper<T>>()?;

        let cm = type_data
            .component_manager
            .clone()
            .downcast_arc::<ComponentManager<T>>()
            .ok()?;

        Some((&*store_ref.0, cm))
    }

    fn get_store_dyn<'a, T>(&'a self) -> Option<&'a (dyn EntityStore<T> + 'static)>
    where
        T: Entity + Sync + Send + 'static,
    {
        let type_id = TypeId::of::<T>();
        let type_data = self.types.get(&type_id)?;

        let store_ref = type_data
            .store
            .downcast_ref::<EntityStoreDowncastHelper<T>>()?;

        Some(&*store_ref.0)
    }

    pub fn get_store<'a, T, U>(&'a self) -> Option<&'a Store<T, U>>
    where
        T: Entity + Sync + Send + 'static,
        U: EntityBackend<T> + Sync + Send + 'static,
    {
        let type_id = TypeId::of::<T>();
        let type_data = self.types.get(&type_id)?;

        let store_ref = type_data
            .store
            .downcast_ref::<EntityStoreDowncastHelper<T>>()?;

        store_ref.0.downcast_ref::<Store<T, U>>()
    }

    pub fn load<T>(&self, id: Snowflake) -> Result<ReadReference<StoreHandle<T>>>
    where
        T: Entity + Sync + Send + 'static,
    {
        let (store, cm) = self
            .get_type_data()
            .ok_or_else(|| TypeNotFoundError::new(String::from(any::type_name::<T>())))?;

        store.load(id, cm)
    }

    pub fn load_mut<T>(&self, id: Snowflake) -> Result<WriteReference<StoreHandle<T>>>
    where
        T: Entity + Sync + Send + 'static,
    {
        let (store, cm) = self
            .get_type_data()
            .ok_or_else(|| TypeNotFoundError::new(String::from(any::type_name::<T>())))?;

        store.load_mut(id, cm)
    }

    pub fn store<T>(&self, entity: T) -> Result<()>
    where
        T: Entity + Sync + Send + 'static,
    {
        let ent_store = self
            .get_store_dyn()
            .ok_or_else(|| TypeNotFoundError::new(String::from(any::type_name::<T>())))?;

        ent_store.store(entity)
    }

    pub fn delete<T>(&self, id: Snowflake) -> Result<()>
    where
        T: Entity + Sync + Send + 'static,
    {
        let (store, cm) = self
            .get_type_data::<T>()
            .ok_or_else(|| TypeNotFoundError::new(String::from(any::type_name::<T>())))?;

        store.delete(id, cm)
    }

    pub fn exists<T>(&self, id: Snowflake) -> Result<bool>
    where
        T: Entity + Sync + Send + 'static,
    {
        let store = self
            .get_store_dyn::<T>()
            .ok_or_else(|| TypeNotFoundError::new(String::from(any::type_name::<T>())))?;

        store.exists(id)
    }

    pub fn keys<T>(&self, page: u64, limit: u64) -> Result<Vec<Snowflake>>
    where
        T: Entity + Sync + Send + 'static,
    {
        let store = self
            .get_store_dyn::<T>()
            .ok_or_else(|| TypeNotFoundError::new(String::from(any::type_name::<T>())))?;

        store.keys(page, limit)
    }
}
