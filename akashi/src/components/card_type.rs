use crate::card::Card;
use crate::ecs::{Component, ComponentManager, ComponentStore, Entity};
use crate::snowflake::{Snowflake, SnowflakeGenerator};
use crate::store::{ReadReference, Store, StoreBackend, StoreHandle, WriteReference};
use crate::util::Result;

use std::any::TypeId;
use std::collections::HashSet;
use std::sync::Arc;

/// An `Entity` representing an abstract card type.
///
/// For instance, this `Entity` can be used to group together card data
/// common to a specific character or other card variety.
#[derive(Debug, Clone)]
pub struct CardType {
    type_id: Snowflake,
    component_manager: Arc<ComponentManager<CardType>>,
    components_attached: HashSet<TypeId>,
}

impl CardType {
    /// Creates a new `CardType` instance.
    pub fn new(
        type_id: Snowflake,
        component_manager: Arc<ComponentManager<CardType>>,
        components_attached: HashSet<TypeId>,
    ) -> CardType {
        CardType {
            type_id,
            component_manager,
            components_attached,
        }
    }

    /// Creates an empty `CardType` instance, with a randomized ID and
    /// no attached `Component`s.
    pub fn generate(
        snowflake_gen: &mut SnowflakeGenerator,
        component_manager: Arc<ComponentManager<CardType>>,
    ) -> CardType {
        CardType {
            type_id: snowflake_gen.generate(),
            component_manager,
            components_attached: HashSet::new(),
        }
    }

    pub fn id(&self) -> Snowflake {
        self.type_id
    }
}

impl Entity for CardType {
    fn id(&self) -> Snowflake {
        self.type_id
    }

    fn component_manager(&self) -> &ComponentManager<CardType> {
        &self.component_manager
    }

    fn components_attached(&self) -> &HashSet<TypeId> {
        &self.components_attached
    }

    fn components_attached_mut(&mut self) -> &mut HashSet<TypeId> {
        &mut self.components_attached
    }
}

/// A `Component` representing a particular `CardType` entity that is
/// associated with a `Card`.
#[derive(Debug, Clone)]
pub struct AttachedCardType<T>
where
    T: StoreBackend<CardType> + 'static,
{
    type_id: Snowflake,
    store: Arc<Store<CardType, T>>,
    component_manager: Arc<ComponentManager<CardType>>,
}

impl<T> AttachedCardType<T>
where
    T: StoreBackend<CardType> + 'static,
{
    /// Constructs a new `AttachedCardType` instance.
    pub fn new(
        type_id: Snowflake,
        store: Arc<Store<CardType, T>>,
        component_manager: Arc<ComponentManager<CardType>>,
    ) -> AttachedCardType<T> {
        AttachedCardType {
            type_id,
            store,
            component_manager,
        }
    }

    /// Gets the ID of the associated `CardType` entity.
    pub fn type_id(&self) -> Snowflake {
        self.type_id
    }

    /// Gets an immutable, read-locked reference to a StoreHandle for
    /// the associated `CardType` entity.
    pub fn load(&self) -> Result<ReadReference<StoreHandle<CardType, T>>> {
        self.store
            .load(self.type_id, self.component_manager.clone())
    }

    /// Gets a mutable, write-locked reference to a StoreHandle for the
    /// associated `CardType` entity.
    pub fn load_mut(&self) -> Result<WriteReference<StoreHandle<CardType, T>>> {
        self.store
            .load_mut(self.type_id, self.component_manager.clone())
    }
}

impl<T> Component<Card> for AttachedCardType<T> where
    T: StoreBackend<CardType> + Sync + Send + 'static
{
}

/// Provides `ComponentStore` services for `AttachedCardType` objects
/// by wrapping another `ComponentStore`.
///
/// The wrapped storage object needs to implement loading and storing
/// card type IDs via the `ComponentStore<Card, Snowflake>` trait.
pub struct CardTypeLayer<T, U>
where
    T: ComponentStore<Card, Snowflake> + 'static,
    U: StoreBackend<CardType> + 'static,
{
    component_backend: T,
    entity_store: Arc<Store<CardType, U>>,
    component_manager: Arc<ComponentManager<CardType>>,
}

impl<T, U> CardTypeLayer<T, U>
where
    T: ComponentStore<Card, Snowflake> + 'static,
    U: StoreBackend<CardType> + 'static,
{
    /// Construct a new CardTypeLayer object.
    pub fn new(
        component_backend: T,
        entity_store: Arc<Store<CardType, U>>,
        component_manager: Arc<ComponentManager<CardType>>,
    ) -> CardTypeLayer<T, U> {
        CardTypeLayer {
            component_backend,
            entity_store,
            component_manager,
        }
    }
}

impl<T, U> ComponentStore<Card, AttachedCardType<U>> for CardTypeLayer<T, U>
where
    T: ComponentStore<Card, Snowflake> + Sync + Send + 'static,
    U: StoreBackend<CardType> + Sync + Send + 'static,
{
    fn load(&self, entity: &Card) -> Result<Option<AttachedCardType<U>>> {
        let attached_id: Option<Snowflake> = self.component_backend.load(entity)?;

        if let Some(type_id) = attached_id {
            Ok(Some(AttachedCardType {
                type_id: type_id,
                store: self.entity_store.clone(),
                component_manager: self.component_manager.clone(),
            }))
        } else {
            Ok(None)
        }
    }

    fn store(&self, entity: &Card, component: AttachedCardType<U>) -> Result<()> {
        self.component_backend
            .store(entity, component.type_id.into())
    }

    fn exists(&self, entity: &Card) -> Result<bool> {
        self.component_backend.exists(entity)
    }

    fn delete(&self, entity: &Card) -> Result<()> {
        self.component_backend.delete(entity)
    }
}
