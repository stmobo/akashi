use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::metadata::MetadataAttached;
use crate::snowflake::{Snowflake, SnowflakeGenerator};

#[derive(Clone, Serialize, Deserialize, Eq, PartialEq, Debug)]
pub struct Card {
    id: Snowflake,
    type_id: Snowflake,
}

impl Card {
    pub fn new(id: Snowflake, type_id: Snowflake) -> Card {
        Card { id, type_id }
    }

    pub fn generate(snowflake_gen: &mut SnowflakeGenerator, type_id: Snowflake) -> Card {
        Card {
            id: snowflake_gen.generate(),
            type_id,
        }
    }

    pub fn id(&self) -> &Snowflake {
        &self.id
    }

    pub fn type_id(&self) -> &Snowflake {
        &self.type_id
    }
}

impl MetadataAttached for Card {}

#[derive(Clone, Serialize, Deserialize, Eq, PartialEq, Debug)]
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

mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[allow(unused_imports)]
    use crate::snowflake::SnowflakeGenerator;

    #[test]
    fn test_card_generate() {
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let type_id = snowflake_gen.generate();

        let card1 = Card::generate(&mut snowflake_gen, type_id);
        let card2 = Card::generate(&mut snowflake_gen, type_id);

        assert_ne!(card1.id(), card2.id());
        assert_eq!(card1.type_id(), card2.type_id());
    }

    #[test]
    fn test_inv() {
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let type_id = snowflake_gen.generate();

        let mut inv = Inventory::empty(snowflake_gen.generate());

        assert_eq!(inv.len(), 0);
        assert!(inv.is_empty());

        let card = Card::generate(&mut snowflake_gen, type_id);
        let id = card.id();

        let res = inv.insert(card.clone());
        assert!(res.is_none());
        assert!(inv.contains_key(*id));
        assert_eq!(inv.len(), 1);

        assert!(inv.get(*id).is_some());
        assert_eq!(inv.get(*id).unwrap().id(), id);

        let res = inv.remove(*id);
        assert!(res.is_some());
        assert_eq!(res.unwrap().id(), id);
    }
}
