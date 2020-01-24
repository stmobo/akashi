//! A manager for creating, loading, and storing [`Entities`](Entity).
use super::component::ComponentManagerDowncast;
use super::entity_store::{
    EntityBackend, EntityStore, EntityStoreDowncast, EntityStoreDowncastHelper, ReadReference,
    StoreHandle, WriteReference,
};
use super::{Component, ComponentBackend, ComponentManager, Entity, Store, TypeNotFoundError};
use crate::snowflake::Snowflake;
use crate::util::Result;

use failure::{err_msg, format_err};

use std::any;
use std::any::TypeId;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[doc(hidden)]
pub struct EntityTypeData {
    store: Box<dyn EntityStoreDowncast>,
    component_manager: Arc<dyn ComponentManagerDowncast>,
}

/// Manages creating, storing, and loading [`Entities`](Entity).
///
/// This acts as a collection of [`Stores`](super::Store) and
/// [`ComponentManagers`](ComponentManager), and provides a unified interface
/// for them to make working with [`Entities`](Entity) simpler.
///
/// # Errors
///
/// As with [`Store`](super::Store) and [`ComponentManager`], any errors reported
/// by backend storage drivers will bubble up through `EntityManager` methods.
///
/// Additionally, attempts to use any of the storage access methods
/// (`load`, `store`, `delete`, etc.) with types that do not exist will return
/// a [`TypeNotFoundError`].
///
/// # Example
///
/// ```
/// use akashi::Card;
/// use akashi::EntityManager;
/// use akashi::Component;
/// use akashi::Entity;
/// use akashi::local_storage::{LocalEntityStorage, LocalComponentStorage};
///
/// // Define a simple component type we can attach to our cards.
/// #[derive(Clone)]
/// struct MyCardComponent {
///     name: String,
///     value: u64
/// }
///
/// impl Component<Card> for MyCardComponent {};
///
/// let mut manager = EntityManager::new();
///
/// // Create and register a simple storage backend for Cards.
/// let card_backend: LocalEntityStorage<Card> = LocalEntityStorage::new();
/// manager.register_entity(card_backend).unwrap();
///
/// // Create and register a simple storage backend for MyCardComponent.
/// let component_backend: LocalComponentStorage<Card, MyCardComponent>
///     = LocalComponentStorage::new();
/// manager.register_component("MyCardComponent", component_backend).unwrap();
///
/// // Create a new Card, and attach some Component data to it.
/// let mut card: Card = manager.create(123456789.into()).unwrap();
/// card.set_component(MyCardComponent {
///     name: String::from("My Card"),
///     value: 100,
/// }).unwrap();
///
/// // Store the card we just made.
/// manager.store(card).unwrap();
///
/// // It should exist in storage now.
/// assert!(manager.exists::<Card>(123456789.into()).unwrap());
///
/// // If we list stored Card IDs now, we'll see it:
/// let card_ids = manager.keys::<Card>(0, 20).unwrap();
/// assert_eq!(card_ids.len(), 1);
/// assert_eq!(card_ids[0], 123456789.into());
///
/// {
///     // Load the card again from storage.
///     let handle = manager.load::<Card>(123456789.into()).unwrap();
///     let card = handle.get().unwrap();
///
///     // Load the component data we attached to the card earlier.
///     let my_data: MyCardComponent = card.get_component().unwrap().unwrap();
///
///     assert_eq!(my_data.name, "My Card");
///     assert_eq!(my_data.value, 100);
/// }
///
/// // Finally, delete the card from storage.
/// manager.delete::<Card>(123456789.into()).unwrap();
/// assert!(!manager.exists::<Card>(123456789.into()).unwrap());
/// ```
pub struct EntityManager {
    types: HashMap<TypeId, EntityTypeData>,
}

impl EntityManager {
    /// Creates a new `EntityManager`.
    pub fn new() -> EntityManager {
        EntityManager {
            types: HashMap::new(),
        }
    }

    /// Registers an [`Entity`] type and its associated storage backend.
    ///
    /// # Errors
    ///
    /// This function will return an error if the [`Entity`] type has already
    /// been registered before.
    ///
    /// # Example
    ///
    /// ```
    /// use akashi::Card;
    /// use akashi::EntityManager;
    /// use akashi::local_storage::LocalEntityStorage;
    ///
    /// let mut manager = EntityManager::new();
    ///
    /// let backend: LocalEntityStorage<Card> = LocalEntityStorage::new();
    ///
    /// // Registered entity type is auto-deduced from the backend type.
    /// assert!(manager.register_entity(backend).is_ok());
    ///
    /// let new_backend: LocalEntityStorage<Card> = LocalEntityStorage::new();
    ///
    /// // Trying to register an entity type twice fails.
    /// assert!(manager.register_entity(new_backend).is_err());
    /// ```
    pub fn register_entity<T, U>(&mut self, backend: U) -> Result<()>
    where
        T: Entity + Sync + Send + 'static,
        U: EntityBackend<T> + Sync + Send + 'static,
    {
        if self.types.contains_key(&TypeId::of::<T>()) {
            return Err(format_err!(
                "entity type already registered: {}",
                any::type_name::<T>()
            ));
        }

        let dc_helper = EntityStoreDowncastHelper(Box::new(Store::<T, U>::new(Arc::new(backend))));
        let type_data = EntityTypeData {
            store: Box::new(dc_helper),
            component_manager: Arc::new(ComponentManager::<T>::new()),
        };

        self.types.insert(TypeId::of::<T>(), type_data);

        Ok(())
    }

    /// Registers an [`Component`] type and its associated storage backend.
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    ///  - the [`Component`]'s associated [`Entity`] type has not been registered,
    ///  - the [`Component`] type has already been registered before, or
    ///  - there exist any [`Entity`] instances using the stored [`ComponentManager`]
    ///    (as then mutating the [`ComponentManager`]'s internal map of registered
    ///     types would not be safe).
    ///
    /// # Example
    ///
    /// ```
    /// use akashi::EntityManager;
    /// use akashi::local_storage::{LocalEntityStorage, LocalComponentStorage};
    /// use akashi::Player;
    /// use akashi::components::Resource;
    ///
    /// // Create a new EntityManager and register a storage backend for Players.
    /// let mut manager = EntityManager::new();
    /// let player_backend: LocalEntityStorage<Player> = LocalEntityStorage::new();
    /// manager.register_entity(player_backend).unwrap();
    ///
    /// let rsc_backend: LocalComponentStorage<Player, Resource> = LocalComponentStorage::new();
    ///
    /// // Registered component type is auto-deduced from the backend type.
    /// assert!(manager.register_component("Resource", rsc_backend).is_ok());
    ///
    /// let rsc_backend: LocalComponentStorage<Player, Resource> = LocalComponentStorage::new();
    ///
    /// // Trying to register a component type twice fails.
    /// assert!(manager.register_component("Resource", rsc_backend).is_err());
    /// ```
    pub fn register_component<T, U, V>(&mut self, name: &str, backend: V) -> Result<()>
    where
        T: Entity + Sync + Send + 'static,
        U: Component<T> + 'static,
        V: ComponentBackend<T, U> + Sync + Send + 'static,
    {
        let type_id = TypeId::of::<T>();

        if !self.types.contains_key(&TypeId::of::<T>()) {
            return Err(format_err!(
                "entity type not registered: {}",
                any::type_name::<T>()
            ));
        }

        let type_data = self.types.get_mut(&type_id).unwrap();

        let dyn_ref = Arc::get_mut(&mut type_data.component_manager)
            .ok_or_else(|| err_msg("could not get exclusive access to ComponentManager"))?;

        let cm_ref = dyn_ref
            .downcast_mut::<ComponentManager<T>>()
            .expect("failed to downcast ComponentManager");

        cm_ref.register_component(name, backend)
    }

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
            .downcast_ref::<EntityStoreDowncastHelper<T>>()
            .expect("failed to downcast EntityStore wrapper");

        let cm = type_data
            .component_manager
            .clone()
            .downcast_arc::<ComponentManager<T>>()
            .expect("failed to downcast ComponentManager");

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
            .downcast_ref::<EntityStoreDowncastHelper<T>>()
            .expect("failed to downcast EntityStore wrapper");

        Some(&*store_ref.0)
    }

    /// Gets a direct reference to the underlying [`Store`](super::Store)
    /// for an [`Entity`] type, if registered.
    ///
    /// # Example
    ///
    /// ```
    /// use akashi::Card;
    /// use akashi::EntityManager;
    /// use akashi::local_storage::LocalEntityStorage;
    /// use akashi::ecs::Store;
    ///
    /// // Set up a new EntityManager that can store Cards:
    /// let mut manager = EntityManager::new();
    /// let backend: LocalEntityStorage<Card> = LocalEntityStorage::new();
    /// manager.register_entity(backend).unwrap();
    ///
    /// // Create a new Card and store it:
    /// let card: Card = manager.create(123456789.into()).unwrap();
    /// manager.store(card).unwrap();
    ///
    /// let store: &Store<Card, LocalEntityStorage<Card>>;
    /// store = manager.get_store().unwrap();
    ///
    /// assert!(store.exists(123456789.into()).unwrap());
    /// ```
    pub fn get_store<'a, T, U>(&'a self) -> Option<&'a Store<T, U>>
    where
        T: Entity + Sync + Send + 'static,
        U: EntityBackend<T> + Sync + Send + 'static,
    {
        let type_id = TypeId::of::<T>();
        let type_data = self.types.get(&type_id)?;

        let store_ref = type_data
            .store
            .downcast_ref::<EntityStoreDowncastHelper<T>>()
            .expect("failed to downcast EntityStore wrapper");

        store_ref.0.downcast_ref::<Store<T, U>>()
    }

    /// Gets the underlying [`ComponentManager`] for an [`Entity`] type,
    /// if registered.
    ///
    /// # Example
    ///
    /// ```
    /// use akashi::Card;
    /// use akashi::EntityManager;
    /// use akashi::ecs::ComponentManager;
    /// use akashi::local_storage::LocalEntityStorage;
    /// use std::sync::Arc;
    /// use std::ptr;
    ///
    /// // Set up a new EntityManager that can store Cards:
    /// let mut manager = EntityManager::new();
    /// let backend: LocalEntityStorage<Card> = LocalEntityStorage::new();
    /// manager.register_entity(backend).unwrap();
    ///
    /// // A ComponentManager will be automatically created for Card entities:
    /// let component_manager: Arc<ComponentManager<Card>>;
    /// component_manager = manager.get_component_manager().unwrap();
    ///
    /// // Create a new Card.
    /// let card: Card = manager.create(123456789.into()).unwrap();
    ///
    /// // The new card will use the same ComponentManager we got back earlier.
    /// assert!(ptr::eq(card.component_manager(), &*component_manager));
    /// ```
    pub fn get_component_manager<T>(&self) -> Option<Arc<ComponentManager<T>>>
    where
        T: Entity + Sync + Send + 'static,
    {
        let type_id = TypeId::of::<T>();
        let type_data = self.types.get(&type_id)?;

        Some(
            type_data
                .component_manager
                .clone()
                .downcast_arc::<ComponentManager<T>>()
                .expect("failed to downcast ComponentManager"),
        )
    }

    /// Creates a new [`Entity`] of a previously-registered type.
    ///
    /// # Example
    ///
    /// ```
    /// use akashi::Card;
    /// use akashi::Player;
    /// use akashi::EntityManager;
    /// use akashi::local_storage::LocalEntityStorage;
    ///
    /// // Set up a new EntityManager that can store Cards (but not Players):
    /// let mut manager = EntityManager::new();
    /// let backend: LocalEntityStorage<Card> = LocalEntityStorage::new();
    /// manager.register_entity(backend).unwrap();
    ///
    /// // An object of the requested type is returned if the type was previously registered.
    /// let card: Option<Card> = manager.create(123456789.into());
    /// assert!(card.is_some());
    ///
    /// // Otherwise, None is returned.
    /// let player: Option<Player> = manager.create(987654321.into());
    /// assert!(player.is_none());
    /// ```
    pub fn create<T>(&self, id: Snowflake) -> Option<T>
    where
        T: Entity + Sync + Send + 'static,
    {
        let type_id = TypeId::of::<T>();
        let type_data = self.types.get(&type_id)?;

        let cm = type_data
            .component_manager
            .clone()
            .downcast_arc::<ComponentManager<T>>()
            .expect("failed to downcast ComponentManager");

        Some(T::new(id, cm, HashSet::new()))
    }

    /// Loads an immutable (read-locked) reference to an [`Entity`] from its
    /// configured storage backend.
    ///
    /// # Example
    ///
    /// ```
    /// use akashi::Card;
    /// use akashi::EntityManager;
    /// use akashi::local_storage::LocalEntityStorage;
    ///
    /// // Set up an EntityManager to store cards.
    /// let mut manager = EntityManager::new();
    /// let backend: LocalEntityStorage<Card> = LocalEntityStorage::new();
    /// manager.register_entity(backend).unwrap();
    ///
    /// // Create and store a card.
    /// let card: Card = manager.create(123456789.into()).unwrap();
    /// manager.store(card).unwrap();
    ///
    /// // Load it again.
    /// let handle = manager.load::<Card>(123456789.into()).unwrap();
    /// assert!(handle.get().is_some());
    /// ```
    pub fn load<T>(&self, id: Snowflake) -> Result<ReadReference<StoreHandle<T>>>
    where
        T: Entity + Sync + Send + 'static,
    {
        let (store, cm) = self
            .get_type_data()
            .ok_or_else(|| TypeNotFoundError::new(String::from(any::type_name::<T>())))?;

        store.load(id, cm)
    }

    /// Loads a mutable (write-locked) reference to an [`Entity`] from its
    /// configured storage backend.
    ///
    /// # Example
    ///
    /// ```
    /// use akashi::local_storage::{LocalEntityStorage, LocalComponentStorage};
    /// use akashi::EntityManager;
    /// use akashi::Component;
    /// use akashi::Entity;
    /// use akashi::Card;
    ///
    /// #[derive(Clone)]
    /// pub struct MyComponent(u64);
    /// impl Component<Card> for MyComponent {}
    ///
    /// // Set up an EntityManager to store cards and our example component.
    /// let mut manager = EntityManager::new();
    ///
    /// let backend: LocalEntityStorage<Card> = LocalEntityStorage::new();
    /// manager.register_entity(backend).unwrap();
    ///
    /// let component_backend: LocalComponentStorage<Card, MyComponent>
    ///     = LocalComponentStorage::new();
    /// manager.register_component("MyComponent", component_backend).unwrap();
    ///
    /// // Create and store a card.
    /// let card: Card = manager.create(123456789.into()).unwrap();
    /// manager.store(card).unwrap();
    ///
    /// {
    ///     // Load it again, mutably.
    ///     let mut handle = manager.load_mut::<Card>(123456789.into()).unwrap();
    ///     let mut card = handle.get_mut().unwrap();
    ///
    ///     // Attach some data to it, then update the stored Entity.
    ///     card.set_component(MyComponent(50)).unwrap();
    ///     handle.store().unwrap();
    /// }
    ///
    /// // Load it once more, immutably.
    /// let handle = manager.load::<Card>(123456789.into()).unwrap();
    /// let card = handle.get().unwrap();
    ///
    /// // Get the component data we previously attached to it.
    /// let component: MyComponent = card.get_component().unwrap().unwrap();
    /// assert_eq!(component.0, 50);
    /// ```
    pub fn load_mut<T>(&self, id: Snowflake) -> Result<WriteReference<StoreHandle<T>>>
    where
        T: Entity + Sync + Send + 'static,
    {
        let (store, cm) = self
            .get_type_data()
            .ok_or_else(|| TypeNotFoundError::new(String::from(any::type_name::<T>())))?;

        store.load_mut(id, cm)
    }

    /// Stores an [`Entity`] object to its configured storage backend.
    ///
    /// # Example
    ///
    /// ```
    /// use akashi::Card;
    /// use akashi::EntityManager;
    /// use akashi::local_storage::LocalEntityStorage;
    ///
    /// // Set up an EntityManager to store cards.
    /// let mut manager = EntityManager::new();
    /// let backend: LocalEntityStorage<Card> = LocalEntityStorage::new();
    /// manager.register_entity(backend).unwrap();
    ///
    /// // Create and store a card.
    /// let card: Card = manager.create(123456789.into()).unwrap();
    /// manager.store(card).unwrap();
    ///
    /// assert!(manager.exists::<Card>(123456789.into()).unwrap());
    /// ```
    pub fn store<T>(&self, entity: T) -> Result<()>
    where
        T: Entity + Sync + Send + 'static,
    {
        let ent_store = self
            .get_store_dyn()
            .ok_or_else(|| TypeNotFoundError::new(String::from(any::type_name::<T>())))?;

        ent_store.store(entity)
    }

    /// Moves the given [`Entity`] into a locked storage handle without writing
    /// it to storage, overwriting anything that may have been there before.
    ///
    /// Returns a write-locked reference to the handle.
    pub fn insert<T>(&self, entity: T) -> Result<WriteReference<StoreHandle<T>>>
    where
        T: Entity + Sync + Send + 'static,
    {
        let ent_store = self
            .get_store_dyn()
            .ok_or_else(|| TypeNotFoundError::new(String::from(any::type_name::<T>())))?;

        Ok(ent_store.insert(entity))
    }

    /// Deletes an [`Entity`] object from its configured storage backend by ID.
    ///
    /// # Example
    ///
    /// ```
    /// use akashi::Card;
    /// use akashi::EntityManager;
    /// use akashi::local_storage::LocalEntityStorage;
    ///
    /// // Set up an EntityManager to store cards.
    /// let mut manager = EntityManager::new();
    /// let backend: LocalEntityStorage<Card> = LocalEntityStorage::new();
    /// manager.register_entity(backend).unwrap();
    ///
    /// // Create and store a card.
    /// let card: Card = manager.create(123456789.into()).unwrap();
    /// manager.store(card).unwrap();
    ///
    /// assert!(manager.exists::<Card>(123456789.into()).unwrap());
    ///
    /// // Delete the card.
    /// manager.delete::<Card>(123456789.into()).unwrap();
    ///
    /// assert!(!manager.exists::<Card>(123456789.into()).unwrap());
    /// ```
    pub fn delete<T>(&self, id: Snowflake) -> Result<()>
    where
        T: Entity + Sync + Send + 'static,
    {
        let (store, cm) = self
            .get_type_data::<T>()
            .ok_or_else(|| TypeNotFoundError::new(String::from(any::type_name::<T>())))?;

        store.delete(id, cm)
    }

    /// Checks whether an [`Entity`] object with the given ID exists.
    pub fn exists<T>(&self, id: Snowflake) -> Result<bool>
    where
        T: Entity + Sync + Send + 'static,
    {
        let store = self
            .get_store_dyn::<T>()
            .ok_or_else(|| TypeNotFoundError::new(String::from(any::type_name::<T>())))?;

        store.exists(id)
    }

    /// Gets a listing of all stored object IDs for the given [`Entity`] type.
    ///
    /// # Example
    ///
    /// ```
    /// use akashi::Card;
    /// use akashi::EntityManager;
    /// use akashi::local_storage::LocalEntityStorage;
    ///
    /// // Set up an EntityManager to store cards.
    /// let mut manager = EntityManager::new();
    /// let backend: LocalEntityStorage<Card> = LocalEntityStorage::new();
    /// manager.register_entity(backend).unwrap();
    ///
    /// // Create and store a card.
    /// let card: Card = manager.create(123456789.into()).unwrap();
    /// manager.store(card).unwrap();
    ///
    /// // Get a list of the first 20 card IDs in our storage.
    /// // This should only contain the ID of the card we just inserted.
    /// let ids = manager.keys::<Card>(0, 20).unwrap();
    /// assert_eq!(ids.len(), 1);
    /// assert_eq!(ids[0], 123456789.into());
    /// ```
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
