//! Storage systems that work entirely in-memory, for testing and prototyping
//! use.

use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::{Arc, RwLock};

use failure::format_err;

use crate::card::Card;
use crate::components::Inventory;
use crate::ecs::entity_store::SharedStore;
use crate::ecs::{Component, ComponentManager, ComponentStore, Entity, Store, StoreBackend};
use crate::player::Player;
use crate::snowflake::Snowflake;
use crate::util::Result;

/// A convenient container for [`Player`] and [`Card`] storage in-memory.
///
/// This is mainly meant for use in testing and for prototyping. It has
/// no provisions for storing data to a persistent medium.
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

/// In-memory storage backend for [`Players`](Player), [`Cards`](Card), and
/// [`Inventories`](Inventory).
pub struct LocalStoreBackend {
    players: RwLock<HashMap<Snowflake, Player>>,
    cards: RwLock<HashMap<Snowflake, Card>>,
    inventories: RwLock<HashMap<Snowflake, Vec<Snowflake>>>,
}

impl LocalStoreBackend {
    fn new() -> LocalStoreBackend {
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

    fn load(&self, id: Snowflake, _cm: Arc<ComponentManager<Player>>) -> Result<Option<Player>> {
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

    fn load(&self, id: Snowflake, _cm: Arc<ComponentManager<Card>>) -> Result<Option<Card>> {
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

/// A storage backend for [`Inventories`](Inventory) that uses a
/// [`LocalStoreBackend`].
pub struct LocalInventoryStore {
    backend: Arc<LocalStoreBackend>,
}

impl LocalInventoryStore {
    pub fn new(backend: Arc<LocalStoreBackend>) -> LocalInventoryStore {
        LocalInventoryStore { backend }
    }
}

impl ComponentStore<Player, Inventory> for LocalInventoryStore {
    fn exists(&self, player: &Player) -> Result<bool> {
        let inventories = self.backend.inventories.read().unwrap();
        Ok(inventories.contains_key(&player.id()))
    }

    fn load(&self, player: &Player) -> Result<Option<Inventory>> {
        let map = self.backend.inventories.read().unwrap();
        Ok(map.get(&player.id()).map(|card_vec| {
            let mut inv = Inventory::empty();
            let cards = self.backend.cards.read().unwrap();

            for card_id in card_vec.iter() {
                if let Some(card) = cards.get(card_id) {
                    inv.insert(card.clone());
                }
            }

            inv
        }))
    }

    fn store(&self, player: &Player, data: Inventory) -> Result<()> {
        {
            let mut cards = self.backend.cards.write().unwrap();
            for card in data.iter() {
                cards.insert(card.id(), card.clone());
            }
        }

        let mut inventories = self.backend.inventories.write().unwrap();
        let ids: Vec<Snowflake> = data.iter().map(|x| x.id()).collect();
        inventories.insert(player.id(), ids);
        Ok(())
    }

    fn delete(&self, player: &Player) -> Result<()> {
        let inv: Vec<Snowflake>;
        {
            let mut inventories = self.backend.inventories.write().unwrap();
            inv = match inventories.remove(&player.id()) {
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

/// In-memory [`Entity`] storage backend.
///
/// This is mainly meant for use in testing and for prototyping. It has
/// no provisions for storing data to a persistent medium.
pub struct LocalEntityStorage<T: Entity + Clone + 'static> {
    data: RwLock<HashMap<Snowflake, T>>,
}

impl<T> LocalEntityStorage<T>
where
    T: Entity + Clone + 'static,
{
    pub fn new() -> LocalEntityStorage<T> {
        LocalEntityStorage {
            data: RwLock::new(HashMap::new()),
        }
    }
}

impl<T> StoreBackend<T> for LocalEntityStorage<T>
where
    T: Entity + Clone + 'static,
{
    fn exists(&self, id: Snowflake) -> Result<bool> {
        let data = self.data.read().unwrap();
        Ok(data.contains_key(&id))
    }

    fn load(&self, id: Snowflake, _cm: Arc<ComponentManager<T>>) -> Result<Option<T>> {
        let data = self.data.read().unwrap();
        Ok(data.get(&id).map(|v| v.clone()))
    }

    fn store(&self, id: Snowflake, obj: &T) -> Result<()> {
        let mut data = self.data.write().unwrap();
        data.insert(id, obj.clone());
        Ok(())
    }

    fn delete(&self, id: Snowflake) -> Result<()> {
        let mut data = self.data.write().unwrap();
        data.remove(&id);
        Ok(())
    }

    fn keys(&self, page: u64, limit: u64) -> Result<Vec<Snowflake>> {
        let ids: Vec<Snowflake>;
        let start_index = page * limit;

        {
            let data = self.data.read().unwrap();
            ids = data
                .keys()
                .skip(start_index as usize)
                .take(limit as usize)
                .copied()
                .collect();
        }

        Ok(ids)
    }
}

/// In-memory [`Component`] storage backend.
///
/// This is mainly meant for use in testing and for prototyping. It has
/// no provisions for storing data to a persistent medium.
pub struct LocalComponentStorage<T, U>
where
    T: Entity + 'static,
    U: Component<T> + Clone + 'static,
{
    data: RwLock<HashMap<Snowflake, U>>,
    pd: PhantomData<T>,
}

impl<T, U> LocalComponentStorage<T, U>
where
    T: Entity + 'static,
    U: Component<T> + Clone + 'static,
{
    pub fn new() -> LocalComponentStorage<T, U> {
        LocalComponentStorage {
            data: RwLock::new(HashMap::new()),
            pd: PhantomData,
        }
    }
}

impl<T, U> ComponentStore<T, U> for LocalComponentStorage<T, U>
where
    T: Entity + 'static,
    U: Component<T> + Clone + 'static,
{
    fn load(&self, entity: &T) -> Result<Option<U>> {
        let data_map = self
            .data
            .read()
            .map_err(|_e| format_err!("storage lock poisoned"))?;
        Ok(data_map.get(&entity.id()).map(|x| x.clone()))
    }

    fn store(&self, entity: &T, component: U) -> Result<()> {
        let mut data_map = self
            .data
            .write()
            .map_err(|_e| format_err!("storage lock poisoned"))?;
        data_map.insert(entity.id(), component);
        Ok(())
    }

    fn exists(&self, entity: &T) -> Result<bool> {
        let data_map = self
            .data
            .read()
            .map_err(|_e| format_err!("storage lock poisoned"))?;
        Ok(data_map.contains_key(&entity.id()))
    }

    fn delete(&self, entity: &T) -> Result<()> {
        let mut data_map = self
            .data
            .write()
            .map_err(|_e| format_err!("storage lock poisoned"))?;
        data_map.remove(&entity.id());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::thread;
    use std::time::Duration;

    use crate::ecs::ComponentManager;
    use crate::snowflake::SnowflakeGenerator;

    #[test]
    fn threaded_access() {
        let store = Arc::new(SharedLocalStore::new());
        let s2 = store.clone();

        let pl_cm = Arc::new(ComponentManager::new());
        let pl_cm2 = pl_cm.clone();

        let card_cm = Arc::new(ComponentManager::new());
        let card_cm2 = card_cm.clone();

        let handle = thread::spawn(move || {
            let store = s2;
            let pl_cm = pl_cm2;
            let card_cm = card_cm2;

            let mut snowflake_gen = SnowflakeGenerator::new(0, 1);

            let pl = Player::empty(&mut snowflake_gen, pl_cm);
            let pl_id = pl.id().clone();

            let players = store.players();
            let cards = store.cards();

            let card = Card::generate(&mut snowflake_gen, card_cm);
            let card_id = card.id().clone();

            players.store(pl).unwrap();
            cards.store(card).unwrap();

            (pl_id, card_id)
        });

        thread::sleep(Duration::from_millis(50));

        let (player_id, card_id) = handle.join().unwrap();

        let players = store.players();
        let cards = store.cards();

        let pl_handle = players.load(player_id, pl_cm.clone()).unwrap();
        assert!(pl_handle.get().is_some());

        let card_handle = cards.load(card_id, card_cm).unwrap();
        assert!(card_handle.get().is_some());
    }
}
