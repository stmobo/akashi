use std::collections::HashMap;
use std::result;
use std::sync::{Arc, RwLock};

use failure::{format_err, Error};

use crate::card::{Card, Inventory};
use crate::component::{Component, ComponentManager, ComponentStore};
use crate::player::Player;
use crate::snowflake::Snowflake;
use crate::store::{NotFoundError, SharedStore, Store, StoreBackend};

type Result<T> = result::Result<T, Error>;

pub struct SharedLocalStore {
    backend: Arc<LocalStoreBackend>,
    players: Store<Player, LocalStoreBackend>,
    cards: Store<Card, LocalStoreBackend>,
}

impl SharedLocalStore {
    pub fn new() -> SharedLocalStore {
        let backend = Arc::new(LocalStoreBackend::new());
        SharedLocalStore {
            players: Store::new(backend.clone()),
            cards: Store::new(backend.clone()),
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

    fn load(&self, id: Snowflake) -> Result<Option<Player>> {
        let players = self.players.read().unwrap();
        if let Some(s) = players.get(&id) {
            Ok(Some(s.clone()))
        } else {
            Ok(None)
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

    fn load(&self, id: Snowflake) -> Result<Option<Card>> {
        let cards = self.cards.read().unwrap();
        match cards.get(&id) {
            None => Ok(None),
            Some(card) => Ok(Some(card.clone())),
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

pub struct LocalInventoryStore {
    backend: Arc<LocalStoreBackend>,
}

impl LocalInventoryStore {
    pub fn new(backend: Arc<LocalStoreBackend>) -> LocalInventoryStore {
        LocalInventoryStore { backend }
    }
}

impl ComponentStore<Inventory> for LocalInventoryStore {
    fn exists(&self, id: Snowflake, _cm: &ComponentManager) -> Result<bool> {
        let inventories = self.backend.inventories.read().unwrap();
        Ok(inventories.contains_key(&id))
    }

    fn load(&self, id: Snowflake, _cm: &ComponentManager) -> Result<Option<Inventory>> {
        let map = self.backend.inventories.read().unwrap();
        Ok(map.get(&id).map(|card_vec| {
            let mut inv = Inventory::empty(id);
            let cards = self.backend.cards.read().unwrap();

            for card_id in card_vec.iter() {
                if let Some(card) = cards.get(card_id) {
                    inv.insert(card.clone());
                }
            }

            inv
        }))
    }

    fn store(&self, id: Snowflake, data: Inventory, _cm: &ComponentManager) -> Result<()> {
        {
            let mut cards = self.backend.cards.write().unwrap();
            for card in data.iter() {
                cards.insert(card.id(), card.clone());
            }
        }

        let mut inventories = self.backend.inventories.write().unwrap();
        let ids: Vec<Snowflake> = data.iter().map(|x| x.id()).collect();
        inventories.insert(id, ids);
        Ok(())
    }

    fn delete(&self, id: Snowflake, _cm: &ComponentManager) -> Result<()> {
        let inv: Vec<Snowflake>;
        {
            let mut inventories = self.backend.inventories.write().unwrap();
            inv = match inventories.remove(&id) {
                None => return Ok(()),
                Some(v) => v,
            };
        }

        {
            let mut cards = self.backend.cards.write().unwrap();
            for card_id in inv.iter() {
                cards.remove(card_id);
            }
        }

        Ok(())
    }
}

pub struct LocalComponentStorage<T: Component + Clone + 'static> {
    data: RwLock<HashMap<Snowflake, T>>,
}

impl<T: Component + Clone + 'static> LocalComponentStorage<T> {
    pub fn new() -> LocalComponentStorage<T> {
        LocalComponentStorage {
            data: RwLock::new(HashMap::new()),
        }
    }
}

impl<T: Component + Clone + 'static> ComponentStore<T> for LocalComponentStorage<T> {
    fn load(&self, entity_id: Snowflake, _cm: &ComponentManager) -> Result<Option<T>> {
        let data_map = self
            .data
            .read()
            .map_err(|_e| format_err!("storage lock poisoned"))?;
        Ok(data_map.get(&entity_id).map(|x| x.clone()))
    }

    fn store(&self, entity_id: Snowflake, component: T, _cm: &ComponentManager) -> Result<()> {
        let mut data_map = self
            .data
            .write()
            .map_err(|_e| format_err!("storage lock poisoned"))?;
        data_map.insert(entity_id, component);
        Ok(())
    }

    fn exists(&self, entity_id: Snowflake, _cm: &ComponentManager) -> Result<bool> {
        let data_map = self
            .data
            .read()
            .map_err(|_e| format_err!("storage lock poisoned"))?;
        Ok(data_map.contains_key(&entity_id))
    }

    fn delete(&self, entity_id: Snowflake, _cm: &ComponentManager) -> Result<()> {
        let mut data_map = self
            .data
            .write()
            .map_err(|_e| format_err!("storage lock poisoned"))?;
        data_map.remove(&entity_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::thread;
    use std::time::Duration;

    use crate::component::ComponentManager;
    use crate::snowflake::SnowflakeGenerator;

    #[test]
    fn threaded_access() {
        let store = Arc::new(SharedLocalStore::new());
        let s2 = store.clone();
        let cm = Arc::new(ComponentManager::new());

        let handle = thread::spawn(move || {
            let store = s2;
            let mut snowflake_gen = SnowflakeGenerator::new(0, 1);

            let pl = Player::empty(&mut snowflake_gen, cm.clone());
            let pl_id = pl.id().clone();

            let players = store.players();
            let cards = store.cards();

            let card = Card::generate(&mut snowflake_gen, cm);
            let card_id = card.id().clone();

            players.store(pl_id, pl).unwrap();
            cards.store(card_id, card).unwrap();

            (pl_id, card_id)
        });

        thread::sleep(Duration::from_millis(50));

        let (player_id, card_id) = handle.join().unwrap();

        let players = store.players();
        let cards = store.cards();

        let pl_ref = players.load(player_id).unwrap();
        let pl_handle = pl_ref.lock().unwrap();
        assert!(pl_handle.get().is_some());

        let card_ref = cards.load(card_id).unwrap();
        let card_handle = card_ref.lock().unwrap();
        assert!(card_handle.get().is_some());
    }
}