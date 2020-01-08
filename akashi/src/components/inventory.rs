//! Card inventories as [`Player`] components.

use crate::card::Card;
use crate::ecs::Component;
use crate::player::Player;
use crate::snowflake::Snowflake;

use std::collections::HashMap;

/// Represents a collection of [`Card`] entities.
///
/// `Inventory` also implements `Component<Player>`, so you can attach
/// instances to [`Players`](Player) (given appropriate storage code).
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

    /// Adds a [`Card`] to this inventory.
    ///
    /// If another [`Card`] with the same ID was stored in this Inventory,
    /// it will be returned.
    pub fn insert(&mut self, card: Card) -> Option<Card> {
        self.cards.insert(card.id(), card)
    }

    /// Checks to see if this inventory contains a [`Card`] with the given
    /// ID.
    pub fn contains(&self, id: Snowflake) -> bool {
        self.cards.contains_key(&id)
    }

    /// Removes a [`Card`] from this inventory by ID and returns it,
    /// if any.
    pub fn remove(&mut self, id: Snowflake) -> Option<Card> {
        self.cards.remove(&id)
    }

    /// Checks to see if this inventory is empty.
    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }

    /// Gets how many [`Cards`](Card) are stored in this inventory.
    pub fn len(&self) -> usize {
        self.cards.len()
    }

    /// Iterates over all [`Cards`](Card) in this inventory.
    pub fn iter<'a>(&'a self) -> impl Iterator<Item = &'a Card> + '_ {
        self.cards.values()
    }

    /// Gets a [`Card`] in this inventory by ID.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::ComponentManager;
    use crate::snowflake::SnowflakeGenerator;

    use std::sync::Arc;

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
