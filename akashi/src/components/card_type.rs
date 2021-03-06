//! Utilities for working with in-game card categories or types.

use crate::card::Card;
use crate::ecs::entity_store::{ReadReference, StoreHandle, WriteReference};
use crate::ecs::{Component, ComponentAdapter, ComponentManager, Entity, EntityManager};
use crate::snowflake::{Snowflake, SnowflakeGenerator};
use crate::util::Result;

use dashmap::DashMap;

use std::any::TypeId;
use std::collections::HashSet;
use std::sync::Arc;

/// An [`Entity`](Entity) representing an abstract card type.
///
/// For instance, this [`Entity`](Entity) can be used to group together card data
/// common to a specific character or other card variety.
pub struct CardType {
    type_id: Snowflake,
    component_manager: Arc<ComponentManager<CardType>>,
    components_attached: HashSet<TypeId>,
    component_preloads: DashMap<TypeId, Box<dyn Component<Self> + Send + Sync + 'static>>,
    dirty: bool,
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
            component_preloads: DashMap::new(),
            dirty: false,
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
            component_preloads: DashMap::new(),
            dirty: false,
        }
    }

    pub fn id(&self) -> Snowflake {
        self.type_id
    }
}

impl Entity for CardType {
    fn new(id: Snowflake, cm: Arc<ComponentManager<Self>>, components: HashSet<TypeId>) -> Self {
        CardType::new(id, cm, components)
    }

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

    fn preloaded_components(
        &self,
    ) -> &DashMap<TypeId, Box<dyn Component<Self> + Send + Sync + 'static>> {
        &self.component_preloads
    }

    fn dirty(&self) -> bool {
        self.dirty
    }

    fn dirty_mut(&mut self) -> &mut bool {
        &mut self.dirty
    }
}

impl Clone for CardType {
    fn clone(&self) -> Self {
        Self {
            type_id: self.type_id,
            dirty: self.dirty,
            component_manager: self.component_manager.clone(),
            components_attached: self.components_attached.clone(),
            component_preloads: DashMap::new(),
        }
    }
}

/// A [`Component`] representing a particular [`CardType`]
/// entity that is associated with a [`Card`].
#[derive(Clone)]
pub struct AttachedCardType {
    type_id: Snowflake,
}

impl AttachedCardType {
    /// Constructs a new `AttachedCardType` instance.
    pub fn new(type_id: Snowflake) -> AttachedCardType {
        AttachedCardType { type_id }
    }

    /// Gets the ID of the associated [`CardType`] entity.
    pub fn type_id(&self) -> Snowflake {
        self.type_id
    }

    /// Gets an immutable, read-locked reference to the actual
    /// [`CardType`] entity referred to by this component
    /// from storage.
    pub fn load(&self, ecs: &EntityManager) -> Result<ReadReference<StoreHandle<CardType>>> {
        ecs.load(self.type_id)
    }

    /// Gets a mutable, write-locked reference to the actual
    /// [`CardType`] entity referred to by this component
    /// from storage.
    pub fn load_mut(&self, ecs: &EntityManager) -> Result<WriteReference<StoreHandle<CardType>>> {
        ecs.load_mut(self.type_id)
    }
}

impl From<Snowflake> for AttachedCardType {
    fn from(id: Snowflake) -> AttachedCardType {
        AttachedCardType { type_id: id }
    }
}

impl From<AttachedCardType> for Snowflake {
    fn from(card_type: AttachedCardType) -> Snowflake {
        card_type.type_id
    }
}

impl Component<Card> for AttachedCardType {}

/// Acts as a [`ComponentBackend`](ComponentBackend) for
/// [`AttachedCardTypes`](AttachedCardType) by wrapping another
/// [`ComponentBackend`](ComponentBackend).
///
/// The wrapped storage type needs to implement loading and storing
/// card type IDs via the `ComponentBackend<Card, Snowflake>` trait.
pub type CardTypeLayer<W> = ComponentAdapter<Card, Snowflake, AttachedCardType, W>;

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
        ecs_manager: EntityManager,
        snowflake_gen: SnowflakeGenerator,
    }

    impl Fixtures {
        fn new() -> Fixtures {
            let mut ecs_manager = EntityManager::new();

            ecs_manager
                .register_entity(LocalEntityStorage::<Card>::new())
                .unwrap();

            ecs_manager
                .register_entity(LocalEntityStorage::<CardType>::new())
                .unwrap();

            ecs_manager
                .register_component(
                    "MockTypeData",
                    LocalComponentStorage::<CardType, MockTypeData>::new(),
                )
                .unwrap();

            ecs_manager
                .register_component(
                    "CardType",
                    CardTypeLayer::new(LocalComponentStorage::<Card, Snowflake>::new()),
                )
                .unwrap();

            Fixtures {
                ecs_manager,
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
        let mut card: Card = fixtures
            .ecs_manager
            .create(fixtures.snowflake_gen.generate())
            .unwrap();

        card.set_component(AttachedCardType::new(type_id)).unwrap();

        card_id = card.id();
        fixtures.ecs_manager.store(card).unwrap();

        // Now load it again and check to see if the CardTypeLayer
        // wrapper code loaded the correct type ID.
        let handle = fixtures.ecs_manager.load::<Card>(card_id).unwrap();

        let card = handle.get().unwrap();
        let attached_type: AttachedCardType = card.get_component().unwrap().unwrap();

        assert_eq!(attached_type.type_id, type_id);
    }

    #[test]
    fn test_attached_card_type_load() {
        let mut fixtures = Fixtures::new();

        // Create and store a new Card Type with attached MockTypeData.
        let mut card_type: CardType = fixtures
            .ecs_manager
            .create(fixtures.snowflake_gen.generate())
            .unwrap();

        let type_id = card_type.id();

        let type_data = MockTypeData {
            title: "Foo".to_owned(),
            character: "Alice".to_owned(),
        };

        card_type.set_component(type_data).unwrap();
        fixtures.ecs_manager.store(card_type).unwrap();

        // Create and store a new card with an attached type ID.
        let card_id: Snowflake;
        let mut card: Card = fixtures
            .ecs_manager
            .create(fixtures.snowflake_gen.generate())
            .unwrap();

        card.set_component(AttachedCardType::new(type_id)).unwrap();

        card_id = card.id();
        fixtures.ecs_manager.store(card).unwrap();

        // Reload the card from storage.
        let handle = fixtures.ecs_manager.load::<Card>(card_id).unwrap();
        let card = handle.get().unwrap();

        // Get attached type data.
        let attached_type: AttachedCardType = card.get_component().unwrap().unwrap();
        assert_eq!(attached_type.type_id, type_id);

        // Attempt to load the type's attached MockTypeData.
        let handle = attached_type.load(&fixtures.ecs_manager).unwrap();
        let card_type = handle.get().unwrap();
        let type_data: MockTypeData = card_type.get_component().unwrap().unwrap();

        assert_eq!(type_data.title, "Foo");
        assert_eq!(type_data.character, "Alice");
    }
}
