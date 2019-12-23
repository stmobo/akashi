use std::error;
use std::fmt;

use std::cell::RefCell;
use std::collections::HashMap;
use std::error::Error;
use std::sync::{Arc, RwLock};

use crate::resources::{ResourceCount, ResourceID};
use crate::store::{Store, StoreBackend, NotFoundError};
use crate::snowflake::Snowflake;

#[derive(Clone)]
pub struct Player {
    id: Snowflake,
    resources: Vec<u64>,
}

impl Player {
    pub fn new(id: Snowflake, resources: Vec<u64>) -> Player {
        Player { id, resources }
    }

    pub fn id(&self) -> Snowflake {
        return self.id;
    }

    pub fn get_resource(&self, id: ResourceID) -> Option<ResourceCount> {
        match self.resources.get(id as usize) {
            None => None,
            Some(val) => Some((id, *val)),
        }
    }

    pub fn set_resource(&mut self, count: ResourceCount) {
        match self.resources.get_mut(count.0 as usize) {
            None => panic!("Invalid resource ID"),
            Some(r) => *r = count.1,
        }
    }
}

type PlayerDataStore<T> = Store<Player, T>;

struct LocalDataBackend {
    players: RwLock<HashMap<Snowflake, Player>>,
}

impl LocalDataBackend {
    pub fn new() -> LocalDataBackend {
        LocalDataBackend {
            players: RwLock::new(HashMap::new()),
        }
    }
}

impl StoreBackend<Player> for LocalDataBackend {
    fn exists(&self, id: &Snowflake) -> Result<bool, Box<dyn error::Error>> {
        let map = self.players.read().unwrap();
        Ok(map.contains_key(id))
    }

    fn load(&self, id: &Snowflake) -> Result<Player, Box<dyn error::Error>> {
        let map = self.players.read().unwrap();
        match map.get(id) {
            None => Err(Box::new(NotFoundError::new(id))),
            Some(pl) => Ok(pl.clone()),
        }
    }

    fn store(&self, id: &Snowflake, player: &Player) -> Result<(), Box<dyn error::Error>> {
        let mut map = self.players.write().unwrap();
        map.insert(*id, player.clone());

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use crate::snowflake::SnowflakeGenerator;

    #[test]
    fn test_player_exists() {
        let mut snowflake_gen = SnowflakeGenerator::new();
        let backend = LocalDataBackend::new();
        let pl = Player::new(snowflake_gen.generate(0, 0), vec![0]);

        backend.store(&pl.id, &pl).unwrap();
        let store = PlayerDataStore::new(backend);

        let id2 = snowflake_gen.generate(0, 0);
        assert!(store.exists(&pl.id()).unwrap());
        assert!(!store.exists(&id2).unwrap());
    }

    #[test]
    fn test_load_nonexistent() {
        let mut snowflake_gen = SnowflakeGenerator::new();
        let backend = LocalDataBackend::new();
        let store = PlayerDataStore::new(backend);
        let result = store.load(&snowflake_gen.generate(0, 0));

        assert!(result.is_err());
    }

    #[test]
    fn test_load_player() {
        let mut snowflake_gen = SnowflakeGenerator::new();
        let backend = LocalDataBackend::new();
        let pl = Player::new(snowflake_gen.generate(0, 0), vec![1, 2, 3]);

        backend.store(&pl.id, &pl).unwrap();
        let store = PlayerDataStore::new(backend);

        let wrapper = store.load(&pl.id()).unwrap();
        let pl_copy = wrapper.lock().unwrap();

        assert_eq!(pl_copy.id(), pl.id());
        assert_eq!(pl_copy.get_resource(0), pl.get_resource(0));
        assert_eq!(pl_copy.get_resource(1), pl.get_resource(1));
        assert_eq!(pl_copy.get_resource(2), pl.get_resource(2));
    }

    #[test]
    fn test_concurrent_load() {
        let mut snowflake_gen = SnowflakeGenerator::new();
        let backend = LocalDataBackend::new();
        let pl = Player::new(snowflake_gen.generate(0, 0), vec![1, 2, 3]);

        backend.store(&pl.id, &pl).unwrap();
        let store = Arc::new(PlayerDataStore::new(backend));

        let store2 = store.clone();
        let id2 = pl.id();
        let handle = thread::spawn(move || {
            let wrapper_1 = store2.load(&id2).unwrap();
            wrapper_1
        });
        
        let wrapper_2 = store.load(&pl.id()).unwrap();
        let wrapper_1 = handle.join().unwrap();

        // wrapper_1 and wrapper_2 should be Arcs pointing to the same
        // data.
        assert!(Arc::ptr_eq(&wrapper_1, &wrapper_2));
    }

    #[test]
    fn test_store_player() {
        let mut snowflake_gen = SnowflakeGenerator::new();

        let backend = LocalDataBackend::new();
        let store = PlayerDataStore::new(backend);
        let id = snowflake_gen.generate(0, 0);
        store.store(&id, &Player::new(id, vec![1, 2, 3])).unwrap();

        let wrapper = store.load(&id).unwrap();
        let pl_copy = wrapper.lock().unwrap();

        assert_eq!(pl_copy.id(), id);
        assert_eq!(pl_copy.get_resource(0), Some((0, 1)));
        assert_eq!(pl_copy.get_resource(1), Some((1, 2)));
        assert_eq!(pl_copy.get_resource(2), Some((2, 3)));
    }
}
