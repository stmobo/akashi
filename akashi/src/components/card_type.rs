//! Utilities for working with in-game card categories or types.

use crate::card::Card;
use crate::ecs::entity_store::{
    EntityStore, ReadReference, Store, StoreBackend, StoreHandle, WriteReference,
};
use crate::ecs::{Component, ComponentManager, ComponentStore, Entity};
use crate::snowflake::{Snowflake, SnowflakeGenerator};
use crate::util::Result;

use std::any::TypeId;
use std::collections::HashSet;
use std::sync::Arc;

/// An [`Entity`](Entity) representing an abstract card type.
///
/// For instance, this [`Entity`](Entity) can be used to group together card data
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
    /// no attached [`Components`](Component).
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

/// A [`Component`] representing a particular [`CardType`]
/// entity that is associated with a [`Card`].
#[derive(Clone)]
pub struct AttachedCardType {
    type_id: Snowflake,
    store: Arc<dyn EntityStore<CardType> + Sync + Send + 'static>,
    component_manager: Arc<ComponentManager<CardType>>,
}

impl AttachedCardType {
    /// Constructs a new `AttachedCardType` instance.
    pub fn new<T: StoreBackend<CardType> + Sync + Send + 'static>(
        type_id: Snowflake,
        store: Arc<Store<CardType, T>>,
        component_manager: Arc<ComponentManager<CardType>>,
    ) -> AttachedCardType {
        AttachedCardType {
            type_id,
            store,
            component_manager,
        }
    }

    /// Gets the ID of the associated [`CardType`] entity.
    pub fn type_id(&self) -> Snowflake {
        self.type_id
    }

    /// Gets an immutable, read-locked reference to the actual
    /// [`CardType`] entity referred to by this component
    /// from storage.
    pub fn load(&self) -> Result<ReadReference<StoreHandle<CardType>>> {
        self.store
            .load(self.type_id, self.component_manager.clone())
    }

    /// Gets a mutable, write-locked reference to the actual
    /// [`CardType`] entity referred to by this component
    /// from storage.
    pub fn load_mut(&self) -> Result<WriteReference<StoreHandle<CardType>>> {
        self.store
            .load_mut(self.type_id, self.component_manager.clone())
    }
}

impl Component<Card> for AttachedCardType {}

/// Acts as a [`ComponentStore`](ComponentStore) for
/// [`AttachedCardTypes`](AttachedCardType) by wrapping another
/// [`ComponentStore`](ComponentStore).
///
/// The wrapped storage type needs to implement loading and storing
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
    /// Constructs a new `CardTypeLayer`.
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

impl<T, U> ComponentStore<Card, AttachedCardType> for CardTypeLayer<T, U>
where
    T: ComponentStore<Card, Snowflake> + Sync + Send + 'static,
    U: StoreBackend<CardType> + Sync + Send + 'static,
{
    fn load(&self, entity: &Card) -> Result<Option<AttachedCardType>> {
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

    fn store(&self, entity: &Card, component: AttachedCardType) -> Result<()> {
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

#[cfg(test)]
mod tests {
    use super::*;

    use crate::local_storage::{LocalComponentStorage, LocalEntityStorage};
    use crate::snowflake::SnowflakeGenerator;

    #[derive(Debug, Clone, PartialEq)]
    struct MockTypeData {
        title: String,
        character: String,
    }

    struct Fixtures {
        card_store: Arc<Store<Card, LocalEntityStorage<Card>>>,
        card_type_store: Arc<Store<CardType, LocalEntityStorage<CardType>>>,
        card_cm: Arc<ComponentManager<Card>>,
        card_type_cm: Arc<ComponentManager<CardType>>,
        snowflake_gen: SnowflakeGenerator,
    }

    impl Fixtures {
        fn new() -> Fixtures {
            let s: Arc<LocalEntityStorage<Card>> = Arc::new(LocalEntityStorage::new());
            let card_store = Arc::new(Store::new(s));

            let s: Arc<LocalEntityStorage<CardType>> = Arc::new(LocalEntityStorage::new());
            let card_type_store = Arc::new(Store::new(s));

            let mut card_type_cm: ComponentManager<CardType> = ComponentManager::new();
            card_type_cm.register_component(
                "MockTypeData",
                LocalComponentStorage::<CardType, MockTypeData>::new(),
            );

            let card_type_cm = Arc::new(card_type_cm);

            let mut card_cm: ComponentManager<Card> = ComponentManager::new();
            card_cm.register_component(
                "CardType",
                CardTypeLayer::new(
                    LocalComponentStorage::<Card, Snowflake>::new(),
                    card_type_store.clone(),
                    card_type_cm.clone(),
                ),
            );

            let card_cm = Arc::new(card_cm);

            Fixtures {
                card_store,
                card_type_store,
                card_cm,
                card_type_cm,
                snowflake_gen: SnowflakeGenerator::new(0, 0),
            }
        }
    }

    impl Component<CardType> for MockTypeData {}

    #[test]
    fn test_store_type() {
        let mut fixtures = Fixtures::new();
        let type_id = fixtures.snowflake_gen.generate();
        let card_id: Snowflake;

        // Create and store a new card with an attached type ID.
        let mut card = Card::generate(&mut fixtures.snowflake_gen, fixtures.card_cm.clone());
        card.set_component(AttachedCardType::new(
            type_id,
            fixtures.card_type_store.clone(),
            fixtures.card_type_cm.clone(),
        ))
        .unwrap();

        card_id = card.id();
        fixtures.card_store.store(card).unwrap();

        // Now load it again and check to see if the CardTypeLayer
        // wrapper code loaded the correct type ID.
        let handle = fixtures
            .card_store
            .load(card_id, fixtures.card_cm.clone())
            .unwrap();
        let card = handle.get().unwrap();
        let attached_type: AttachedCardType = card.get_component().unwrap().unwrap();

        assert_eq!(attached_type.type_id, type_id);
    }

    #[test]
    fn test_attached_card_type_load() {
        let mut fixtures = Fixtures::new();

        // Create and store a new Card Type with attached MockTypeData.
        let mut card_type =
            CardType::generate(&mut fixtures.snowflake_gen, fixtures.card_type_cm.clone());
        let type_id = card_type.id();

        let type_data = MockTypeData {
            title: "Foo".to_owned(),
            character: "Alice".to_owned(),
        };

        card_type.set_component(type_data).unwrap();
        fixtures.card_type_store.store(card_type).unwrap();

        // Create and store a new card with an attached type ID.
        let card_id: Snowflake;
        let mut card = Card::generate(&mut fixtures.snowflake_gen, fixtures.card_cm.clone());

        card.set_component(AttachedCardType::new(
            type_id,
            fixtures.card_type_store.clone(),
            fixtures.card_type_cm.clone(),
        ))
        .unwrap();

        card_id = card.id();
        fixtures.card_store.store(card).unwrap();

        // Reload the card from storage.
        let handle = fixtures
            .card_store
            .load(card_id, fixtures.card_cm.clone())
            .unwrap();
        let card = handle.get().unwrap();

        // Get attached type data.
        let attached_type: AttachedCardType = card.get_component().unwrap().unwrap();
        assert_eq!(attached_type.type_id, type_id);

        // Attempt to load the type's attached MockTypeData.
        let handle = attached_type.load().unwrap();
        let card_type = handle.get().unwrap();
        let type_data: MockTypeData = card_type.get_component().unwrap().unwrap();

        assert_eq!(type_data.title, "Foo");
        assert_eq!(type_data.character, "Alice");
    }
}
