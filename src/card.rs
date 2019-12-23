use std::collections::HashMap;
use std::error;
use std::result;
use std::sync::{Arc, Mutex, Weak};

use crate::snowflake::Snowflake;
use crate::store::{Store, StoreBackend};

type Result<T> = result::Result<T, Box<dyn error::Error>>;

#[derive(Clone)]
pub struct Card {
    id: Snowflake,
    type_id: Snowflake,
}

#[derive(Clone)]
pub struct Inventory {
    id: Snowflake,
    cards: Vec<Card>,
}

impl Inventory {
    pub fn empty(id: Snowflake) -> Inventory {
        Inventory {
            id,
            cards: Vec::new(),
        }
    }
}

pub trait CardMetadataProvider<T> {
    fn get_card_metadata(&self, card_id: &Snowflake, type_id: &Snowflake) -> Result<T>;
    fn set_card_metadata(&self, card_id: &Snowflake, type_id: &Snowflake, data: &T) -> Result<()>;
    fn clear_card_metadata(&self, card_id: &Snowflake, type_id: &Snowflake) -> Result<()>;
}
