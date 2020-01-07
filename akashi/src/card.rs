//! A representation of an in-game card.

use std::any::TypeId;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::ecs::{Component, ComponentManager, Entity};
use crate::player::Player;
use crate::snowflake::{Snowflake, SnowflakeGenerator};

/// Represents a tradable card.
///
/// Strictly speaking, this is just a bare-bones Entity.  
#[derive(Clone, Debug)]
pub struct Card {
    id: Snowflake,
    component_manager: Arc<ComponentManager<Card>>,
    components_attached: HashSet<TypeId>,
}

impl Card {
    /// Create a new `Card` instance.
    pub fn new(
        id: Snowflake,
        component_manager: Arc<ComponentManager<Card>>,
        components_attached: HashSet<TypeId>,
    ) -> Card {
        Card {
            id,
            component_manager,
            components_attached,
        }
    }

    /// Create an 'empty' `Card` instance with a random ID.
    pub fn generate(
        snowflake_gen: &mut SnowflakeGenerator,
        component_manager: Arc<ComponentManager<Card>>,
    ) -> Card {
        Card {
            id: snowflake_gen.generate(),
            component_manager,
            components_attached: HashSet::new(),
        }
    }

    /// Get this `Card`'s unique ID.
    pub fn id(&self) -> Snowflake {
        self.id
    }

    /// Get a reference to this `Card`'s associated `ComponentManager`.
    pub fn component_manager(&self) -> &ComponentManager<Card> {
        &self.component_manager
    }
}

impl PartialEq for Card {
    fn eq(&self, other: &Self) -> bool {
        (self.id == other.id) && Arc::ptr_eq(&self.component_manager, &other.component_manager)
    }
}

impl Entity for Card {
    fn id(&self) -> Snowflake {
        self.id()
    }

    fn component_manager(&self) -> &ComponentManager<Card> {
        &self.component_manager
    }

    fn components_attached(&self) -> &HashSet<TypeId> {
        &self.components_attached
    }

    fn components_attached_mut(&mut self) -> &mut HashSet<TypeId> {
        &mut self.components_attached
    }
}

/// Represents a collection of `Card` entities.
///
/// `Inventory` also implements `Component<Player>`, so you can attach
/// instances to `Player`s (given appropriate storage code).
#[derive(Clone, PartialEq, Debug)]
pub struct Inventory {
    cards: HashMap<Snowflake, Card>,
}

impl Inventory {
    /// Creates a new, empty `Inventory`.
    pub fn empty() -> Inventory {
        Inventory {
            cards: HashMap::new(),
        }
    }

    /// Adds a `Card` to this inventory.
    ///
    /// If another `Card` with the same ID was stored in this Inventory,
    /// it will be returned.
    pub fn insert(&mut self, card: Card) -> Option<Card> {
        self.cards.insert(card.id, card)
    }

    /// Checks to see if this inventory contains a `Card` with the given
    /// ID.
    pub fn contains(&self, id: Snowflake) -> bool {
        self.cards.contains_key(&id)
    }

    /// Removes a `Card` from this inventory by ID and returns it,
    /// if any.
    pub fn remove(&mut self, id: Snowflake) -> Option<Card> {
        self.cards.remove(&id)
    }

    /// Checks to see if this inventory is empty.
    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }

    /// Gets how many `Card`s are stored in this inventory.
    pub fn len(&self) -> usize {
        self.cards.len()
    }

    /// Iterates over all `Card`s in this inventory.
    pub fn iter<'a>(&'a self) -> impl Iterator<Item = &'a Card> + '_ {
        self.cards.values()
    }

    /// Get a `Card` in this inventory by ID.
    pub fn get(&self, id: Snowflake) -> Option<&Card> {
        self.cards.get(&id)
    }
}

impl Component<Player> for Inventory {}

impl Default for Inventory {
    fn default() -> Inventory {
        Inventory::empty()
    }
}

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
        let mut inv = Inventory::empty();

        assert_eq!(inv.len(), 0);
        assert!(inv.is_empty());

        let card = Card::generate(&mut snowflake_gen, cm);
        let id = card.id();

        let res = inv.insert(card.clone());
        assert!(res.is_none());
        assert!(inv.contains(id));
        assert_eq!(inv.len(), 1);

        assert!(inv.get(id).is_some());
        assert_eq!(inv.get(id).unwrap().id(), id);

        let res = inv.remove(id);
        assert!(res.is_some());
        assert_eq!(res.unwrap().id(), id);
    }
}
