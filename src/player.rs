use std::error;
use std::fmt;

use std::cell::RefCell;
use std::collections::HashMap;
use std::error::Error;
use std::sync::{Arc, Mutex, MutexGuard, RwLock, Weak};

use crate::resources::{ResourceCount, ResourceID};
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

type Result<T> = std::result::Result<T, Box<dyn error::Error>>;
type SharedPlayerRef = Arc<Mutex<Player>>;
type WeakPlayerRef = Weak<Mutex<Player>>;
pub struct PlayerDataStore<T: PlayerDataBackend> {
    backend: T,
    active_ptrs: Mutex<HashMap<Snowflake, WeakPlayerRef>>,
}

impl<T: PlayerDataBackend> PlayerDataStore<T> {
    pub fn new(backend: T) -> PlayerDataStore<T> {
        PlayerDataStore {
            backend,
            active_ptrs: Mutex::new(HashMap::new()),
        }
    }

    fn load_player(
        &self,
        map: &mut MutexGuard<HashMap<Snowflake, WeakPlayerRef>>,
        id: &Snowflake,
    ) -> Result<SharedPlayerRef> {
        // we always check for !self.exists(id) first before entering this function, so load_player should always return
        // non-None
        let p = self.backend.load_player(id)?.unwrap();
        let r = Arc::new(Mutex::new(p));
        map.insert(*id, Arc::downgrade(&r));
        Ok(r)
    }

    pub fn load(&self, id: &Snowflake) -> Result<Option<SharedPlayerRef>> {
        if !self.exists(id)? {
            return Ok(None);
        }

        let r: SharedPlayerRef;
        {
            let mut map = self.active_ptrs.lock().unwrap();

            if let Some(wk) = map.get(id) {
                if let Some(arc) = wk.upgrade() {
                    r = arc;
                } else {
                    r = self.load_player(&mut map, id)?;
                }
            } else {
                r = self.load_player(&mut map, id)?;
            }
        }
        Ok(Some(r))
    }

    pub fn exists(&self, id: &Snowflake) -> Result<bool> {
        self.backend.player_exists(id)
    }

    pub fn store(&self, player: &Player) -> Result<()> {
        self.backend.store_player(player)
    }
}

pub trait PlayerDataBackend {
    fn load_player(&self, id: &Snowflake) -> Result<Option<Player>>;
    fn player_exists(&self, id: &Snowflake) -> Result<bool>;
    fn store_player(&self, player: &Player) -> Result<()>;
}

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

impl PlayerDataBackend for LocalDataBackend {
    fn player_exists(&self, id: &Snowflake) -> Result<bool> {
        let map = self.players.read().unwrap();
        Ok(map.contains_key(id))
    }

    fn load_player(&self, id: &Snowflake) -> Result<Option<Player>> {
        let map = self.players.read().unwrap();
        match map.get(id) {
            None => Ok(None),
            Some(pl) => Ok(Some(pl.clone())),
        }
    }

    fn store_player(&self, player: &Player) -> Result<()> {
        let mut map = self.players.write().unwrap();
        map.insert(player.id, player.clone());

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

        backend.store_player(&pl).unwrap();
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
        let result = store.load(&snowflake_gen.generate(0, 0)).unwrap();

        assert!(result.is_none());
    }

    #[test]
    fn test_load_player() {
        let mut snowflake_gen = SnowflakeGenerator::new();
        let backend = LocalDataBackend::new();
        let pl = Player::new(snowflake_gen.generate(0, 0), vec![1, 2, 3]);

        backend.store_player(&pl).unwrap();
        let store = PlayerDataStore::new(backend);

        let wrapper = store.load(&pl.id()).unwrap().unwrap();
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

        backend.store_player(&pl).unwrap();
        let store = Arc::new(PlayerDataStore::new(backend));

        let store2 = store.clone();
        let id2 = pl.id();
        let handle = thread::spawn(move || {
            let wrapper_1 = store2.load(&id2).unwrap().unwrap();
            wrapper_1
        });
        
        let wrapper_2 = store.load(&pl.id()).unwrap().unwrap();
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
        store.store(&Player::new(id, vec![1, 2, 3])).unwrap();

        let wrapper = store.load(&id).unwrap().unwrap();
        let pl_copy = wrapper.lock().unwrap();

        assert_eq!(pl_copy.id(), id);
        assert_eq!(pl_copy.get_resource(0), Some((0, 1)));
        assert_eq!(pl_copy.get_resource(1), Some((1, 2)));
        assert_eq!(pl_copy.get_resource(2), Some((2, 3)));
    }
}
