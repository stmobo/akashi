use std::collections::HashMap;

use crate::metadata::MetadataAttached;
use crate::snowflake::{Snowflake, SnowflakeGenerator};

#[derive(Clone)]
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

#[derive(Clone)]
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

    pub fn insert(&mut self, card: Card) -> Option<Card> {
        self.cards.insert(card.id, card)
    }

    pub fn contains_key(&self, id: &Snowflake) -> bool {
        self.cards.contains_key(id)
    }

    pub fn remove(&mut self, id: &Snowflake) -> Option<Card> {
        self.cards.remove(id)
    }

    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }

    pub fn len(&self) -> usize {
        self.cards.len()
    }

    pub fn iter(&self) -> impl Iterator + '_ {
        self.cards.iter()
    }

    pub fn get<'a>(&'a self, id: &Snowflake) -> Option<&'a Card> {
        self.cards.get(id)
    }
}

mod tests {
    use super::*;

    #[test]
    fn test_card_generate() {
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let type_id = snowflake_gen.generate();

        let card1 = Card::generate(&mut snowflake_gen, type_id);
        let card2 = Card::generate(&mut snowflake_gen, type_id);

        assert_ne!(card1.id(), card2.id());
        assert_eq!(card1.type_id(), card2.type_id());
    }
}
