use std::collections::HashMap;
use std::ops::Deref;
use std::sync::Arc;

use crate::component::{Component, ComponentManager, ComponentsAttached};
use crate::snowflake::{Snowflake, SnowflakeGenerator};

#[derive(Clone, Debug)]
pub struct Card {
    id: Snowflake,
    component_manager: Arc<ComponentManager>,
}

impl Card {
    pub fn new(id: Snowflake, component_manager: Arc<ComponentManager>) -> Card {
        Card {
            id,
            component_manager,
        }
    }

    pub fn generate(
        snowflake_gen: &mut SnowflakeGenerator,
        component_manager: Arc<ComponentManager>,
    ) -> Card {
        Card {
            id: snowflake_gen.generate(),
            component_manager,
        }
    }

    pub fn id(&self) -> Snowflake {
        self.id
    }

    pub fn component_manager(&self) -> &ComponentManager {
        self.component_manager.deref()
    }
}

impl PartialEq for Card {
    fn eq(&self, other: &Self) -> bool {
        (self.id == other.id) && Arc::ptr_eq(&self.component_manager, &other.component_manager)
    }
}

impl ComponentsAttached for Card {
    fn id(&self) -> Snowflake {
        self.id()
    }
    fn component_manager(&self) -> &ComponentManager {
        self.component_manager()
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Inventory {
    id: Snowflake,
    cards: HashMap<Snowflake, Card>,
}

impl Inventory {
    pub fn empty(id: Snowflake) -> Inventory {
        Inventory {
            id,
            cards: HashMap::new(),
        }
    }

    pub fn id(&self) -> &Snowflake {
        &self.id
    }

    pub fn insert(&mut self, card: Card) -> Option<Card> {
        self.cards.insert(card.id, card)
    }

    pub fn contains_key(&self, id: Snowflake) -> bool {
        self.cards.contains_key(&id)
    }

    pub fn remove(&mut self, id: Snowflake) -> Option<Card> {
        self.cards.remove(&id)
    }

    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }

    pub fn len(&self) -> usize {
        self.cards.len()
    }

    pub fn iter<'a>(&'a self) -> impl Iterator<Item = &'a Card> + '_ {
        self.cards.values()
    }

    pub fn get(&self, id: Snowflake) -> Option<&Card> {
        self.cards.get(&id)
    }
}

impl Component for Inventory {}

mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[allow(unused_imports)]
    use crate::snowflake::SnowflakeGenerator;

    #[test]
    fn test_card_generate() {
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let cm = Arc::new(ComponentManager::new());

        let card1 = Card::generate(&mut snowflake_gen, cm.clone());
        let card2 = Card::generate(&mut snowflake_gen, cm);

        assert_ne!(card1.id(), card2.id());
    }

    #[test]
    fn test_inv() {
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let cm = Arc::new(ComponentManager::new());
        let mut inv = Inventory::empty(snowflake_gen.generate());

        assert_eq!(inv.len(), 0);
        assert!(inv.is_empty());

        let card = Card::generate(&mut snowflake_gen, cm);
        let id = card.id();

        let res = inv.insert(card.clone());
        assert!(res.is_none());
        assert!(inv.contains_key(id));
        assert_eq!(inv.len(), 1);

        assert!(inv.get(id).is_some());
        assert_eq!(inv.get(id).unwrap().id(), id);

        let res = inv.remove(id);
        assert!(res.is_some());
        assert_eq!(res.unwrap().id(), id);
    }
}