use crate::card::Card;
use crate::ecs::{Component, ComponentManager, ComponentStore, Entity};
use crate::snowflake::{Snowflake, SnowflakeGenerator};
use crate::store::{Store, StoreBackend};

use std::any::TypeId;
use std::collections::HashSet;
use std::ops::Deref;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct CardType {
    type_id: Snowflake,
    component_manager: Arc<ComponentManager<CardType>>,
    components_attached: HashSet<TypeId>,
}

impl CardType {
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

impl Component<Card> for CardType {}

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

#[derive(Debug, Clone)]
pub struct CardTypeId(Snowflake);

impl Component<Card> for CardTypeId {}

impl CardTypeId {
    pub fn new(id: Snowflake) -> CardTypeId {
        CardTypeId(id)
    }
}

impl From<Snowflake> for CardTypeId {
    fn from(id: Snowflake) -> CardTypeId {
        CardTypeId(id)
    }
}

impl From<CardTypeId> for Snowflake {
    fn from(id: CardTypeId) -> Snowflake {
        id.0
    }
}

impl Deref for CardTypeId {
    type Target = Snowflake;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct CardTypeLayer<T, U>
where
    T: ComponentStore<Card, CardTypeId>,
    U: StoreBackend<CardType> + 'static,
{
    entity_store: Arc<Store<CardType, U>>,
    component_manager: Arc<ComponentManager<CardType>>,
    type_store_backend: T,
}

impl<T, U> CardTypeLayer<T, U>
where
    T: ComponentStore<Card, CardTypeId>,
    U: StoreBackend<CardType> + 'static,
{
    pub fn new(
        entity_store: Arc<Store<CardType, U>>,
        component_manager: Arc<ComponentManager<CardType>>,
        type_store_backend: T,
    ) -> CardTypeLayer<T, U> {
        CardTypeLayer {
            entity_store,
            component_manager,
            type_store_backend,
        }
    }
}

// impl ComponentStore<Card, CardType> for CardTypeLayer {
//     fn load(&self, card: &Card) -> Result<Option<CardType>> {

//     }
// }
