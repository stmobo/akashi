use std::collections::HashMap;
use std::error;
use std::result;
use std::sync::{Arc, RwLock};

use crate::card::{Card, Inventory};
use crate::player::Player;
use crate::snowflake::Snowflake;
use crate::store::{NotFoundError, SharedStore, Store, StoreBackend};

type Result<T> = result::Result<T, Box<dyn error::Error + Send + Sync>>;

pub struct SharedLocalStore {
    backend: Arc<LocalStoreBackend>,
    players: Store<Player, LocalStoreBackend>,
    cards: Store<Card, LocalStoreBackend>,
    inventories: Store<Inventory, LocalStoreBackend>,
}

impl SharedLocalStore {
    pub fn new() -> SharedLocalStore {
        let backend = Arc::new(LocalStoreBackend::new());
        SharedLocalStore {
            players: Store::new(backend.clone()),
            cards: Store::new(backend.clone()),
            inventories: Store::new(backend.clone()),
            backend,
        }
    }

    pub fn backend(&self) -> Arc<LocalStoreBackend> {
        self.backend.clone()
    }

    pub fn players(&self) -> &Store<Player, LocalStoreBackend> {
        &self.players
    }

    pub fn cards(&self) -> &Store<Card, LocalStoreBackend> {
        &self.cards
    }

    pub fn inventories(&self) -> &Store<Inventory, LocalStoreBackend> {
        &self.inventories
    }
}

impl Default for SharedLocalStore {
    fn default() -> SharedLocalStore {
        SharedLocalStore::new()
    }
}

impl SharedStore<Player, LocalStoreBackend> for SharedLocalStore {
    fn get_store<'a>(&'a self) -> &'a Store<Player, LocalStoreBackend> {
        self.players()
    }
}

impl SharedStore<Card, LocalStoreBackend> for SharedLocalStore {
    fn get_store<'a>(&'a self) -> &'a Store<Card, LocalStoreBackend> {
        self.cards()
    }
}

impl SharedStore<Inventory, LocalStoreBackend> for SharedLocalStore {
    fn get_store<'a>(&'a self) -> &'a Store<Inventory, LocalStoreBackend> {
        self.inventories()
    }
}

pub struct LocalStoreBackend {
    players: RwLock<HashMap<Snowflake, Player>>,
    cards: RwLock<HashMap<Snowflake, Card>>,
    inventories: RwLock<HashMap<Snowflake, Vec<Snowflake>>>,
}

impl LocalStoreBackend {
    pub fn new() -> LocalStoreBackend {
        LocalStoreBackend {
            players: RwLock::new(HashMap::new()),
            cards: RwLock::new(HashMap::new()),
            inventories: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for LocalStoreBackend {
    fn default() -> LocalStoreBackend {
        LocalStoreBackend::new()
    }
}

impl StoreBackend<Player> for LocalStoreBackend {
    fn exists(&self, id: Snowflake) -> Result<bool> {
        let players = self.players.read().unwrap();
        Ok(players.contains_key(&id))
    }

    fn load(&self, id: Snowflake) -> Result<Player> {
        let players = self.players.read().unwrap();
        if let Some(s) = players.get(&id) {
            Ok(s.clone())
        } else {
            Err(Box::new(NotFoundError::new(id)))
        }
    }

    fn store(&self, id: Snowflake, data: &Player) -> Result<()> {
        let mut players = self.players.write().unwrap();
        players.insert(id, data.clone());

        Ok(())
    }

    fn delete(&self, id: Snowflake) -> Result<()> {
        let mut players = self.players.write().unwrap();
        players.remove(&id);

        Ok(())
    }

    fn keys(&self, page: u64, limit: u64) -> Result<Vec<Snowflake>> {
        let ids: Vec<Snowflake>;
        let start_index = page * limit;

        {
            let players = self.players.read().unwrap();
            ids = players
                .keys()
                .skip(start_index as usize)
                .take(limit as usize)
                .copied()
                .collect();
        }

        Ok(ids)
    }
}

impl StoreBackend<Card> for LocalStoreBackend {
    fn exists(&self, id: Snowflake) -> Result<bool> {
        let cards = self.cards.read().unwrap();
        Ok(cards.contains_key(&id))
    }

    fn load(&self, id: Snowflake) -> Result<Card> {
        let cards = self.cards.read().unwrap();
        match cards.get(&id) {
            None => Err(Box::new(NotFoundError::new(id))),
            Some(card) => Ok(card.clone()),
        }
    }

    fn store(&self, id: Snowflake, data: &Card) -> Result<()> {
        let mut cards = self.cards.write().unwrap();
        cards.insert(id, data.clone());
        Ok(())
    }

    fn delete(&self, id: Snowflake) -> Result<()> {
        let mut cards = self.cards.write().unwrap();
        cards.remove(&id);
        Ok(())
    }

    fn keys(&self, page: u64, limit: u64) -> Result<Vec<Snowflake>> {
        let ids: Vec<Snowflake>;
        let start_index = page * limit;

        {
            let cards = self.cards.read().unwrap();
            ids = cards
                .keys()
                .skip(start_index as usize)
                .take(limit as usize)
                .copied()
                .collect();
        }

        Ok(ids)
    }
}

impl StoreBackend<Inventory> for LocalStoreBackend {
    fn exists(&self, id: Snowflake) -> Result<bool> {
        let inventories = self.inventories.read().unwrap();
        Ok(inventories.contains_key(&id))
    }

    fn load(&self, id: Snowflake) -> Result<Inventory> {
        let map = self.inventories.read().unwrap();
        match map.get(&id) {
            None => Err(Box::new(NotFoundError::new(id))),
            Some(v) => {
                let mut inv = Inventory::empty(id);
                let cards = self.cards.read().unwrap();

                for card_id in v.iter() {
                    if let Some(card) = cards.get(card_id) {
                        inv.insert(card.clone());
                    }
                }

                Ok(inv)
            }
        }
    }

    fn store(&self, id: Snowflake, data: &Inventory) -> Result<()> {
        {
            let mut cards = self.cards.write().unwrap();
            for card in data.iter() {
                cards.insert(*card.id(), card.clone());
            }
        }

        let mut inventories = self.inventories.write().unwrap();
        let ids: Vec<Snowflake> = data.iter().map(|x| *x.id()).collect();
        inventories.insert(id, ids);
        Ok(())
    }

    fn delete(&self, id: Snowflake) -> Result<()> {
        let inv: Vec<Snowflake>;
        {
            let mut inventories = self.inventories.write().unwrap();
            inv = match inventories.remove(&id) {
                None => return Ok(()),
                Some(v) => v,
            };
        }

        {
            let mut cards = self.cards.write().unwrap();
            for card_id in inv.iter() {
                cards.remove(card_id);
            }
        }

        Ok(())
    }

    fn keys(&self, page: u64, limit: u64) -> Result<Vec<Snowflake>> {
        let ids: Vec<Snowflake>;
        let start_index = page * limit;

        {
            let inventories = self.inventories.read().unwrap();
            ids = inventories
                .keys()
                .skip(start_index as usize)
                .take(limit as usize)
                .copied()
                .collect();
        }

        Ok(ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::thread;
    use std::time::Duration;

    use crate::snowflake::SnowflakeGenerator;

    #[test]
    fn threaded_access() {
        let store = Arc::new(SharedLocalStore::new());
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let type_id = snowflake_gen.generate();
        let _type2_id = snowflake_gen.generate();

        let s2 = store.clone();
        let thread_2_typeid = type_id.clone();
        let handle = thread::spawn(move || {
            let store = s2;
            let type_id = thread_2_typeid;
            let mut snowflake_gen = SnowflakeGenerator::new(0, 1);

            let mut pl = Player::empty(&mut snowflake_gen);
            let pl_id = pl.id().clone();
            pl.set_resource(0, 10);

            let players = store.players();
            let inventories = store.inventories();

            let mut inv = Inventory::empty(snowflake_gen.generate());
            let card = Card::generate(&mut snowflake_gen, type_id);
            let card_id = card.id().clone();

            inv.insert(card);
            pl.attach_inventory("cards", *inv.id());

            players.store(*pl.id(), pl).unwrap();
            inventories.store(*inv.id(), inv).unwrap();

            (pl_id, card_id)
        });

        thread::sleep(Duration::from_millis(50));

        let (player_id, card_id) = handle.join().unwrap();

        let players = store.players();
        let inventories = store.inventories();

        let pl_ref = players.load(player_id).unwrap();
        let pl_handle = pl_ref.lock().unwrap();
        let pl = pl_handle.get().unwrap();

        assert!(pl.get_resource(0).is_some());
        assert_eq!(pl.get_resource(0).unwrap(), 10);

        let inv_id = pl.get_inventory("cards").unwrap();
        let inv_ref = inventories.load(*inv_id).unwrap();
        let inv_handle = inv_ref.lock().unwrap();
        let inv = inv_handle.get().unwrap();

        let card = inv.get(card_id);
        assert!(card.is_some());

        let card = card.unwrap();
        assert_eq!(*card.type_id(), type_id);
    }
}
