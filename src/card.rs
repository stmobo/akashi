use std::result;
use std::error;
use std::sync::{Arc, Weak, Mutex};
use std::collections::HashMap;

use crate::snowflake::Snowflake;
use crate::store::{Store, StoreBackend};

type Result<T> = result::Result<T, Box<dyn error::Error>>;

pub struct Card {
    id: Snowflake,
    type_id: Snowflake,
}

pub struct Inventory<T: InventoryBackend> {
    id: Snowflake,
    backend: Arc<T>,
    cards: Vec<Card>,
    owner: Option<Snowflake>,
}

impl<T: InventoryBackend> Inventory<T> {
    pub fn load(backend: Arc<T>, id: Snowflake) -> Result<Inventory<T>> {
        Ok(Inventory {
            id,
            cards: backend.load_inv_cards(&id)?,
            owner: backend.load_inv_owner(&id)?,
            backend
        })
    }

    pub fn save(&self) -> Result<()> {
        self.backend.save_inv_cards(&self.id, &self.cards)?;
        self.backend.save_inv_owner(&self.id, &self.owner)?;
        Ok(())
    }
}

pub trait InventoryBackend {
    fn load_inv_cards(&self, id: &Snowflake) -> Result<Vec<Card>>;
    fn load_inv_owner(&self, id: &Snowflake) -> Result<Option<Snowflake>>;
    fn save_inv_cards(&self, id: &Snowflake, cards: &Vec<Card>) -> Result<()>;
    fn save_inv_owner(&self, id: &Snowflake, owner: &Option<Snowflake>) -> Result<()>;
}

pub trait CardMetadataProvider<T> {
    fn get_card_metadata(&self, card_id: &Snowflake, type_id: &Snowflake) -> Result<T>;
    fn set_card_metadata(&self, card_id: &Snowflake, type_id: &Snowflake, data: &T) -> Result<()>;
    fn clear_card_metadata(&self, card_id: &Snowflake, type_id: &Snowflake) -> Result<()>;
}

