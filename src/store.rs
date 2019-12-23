use std::collections::HashMap;
use std::error;
use std::fmt;
use std::result;
use std::sync::{Arc, Mutex, MutexGuard, Weak};

use crate::snowflake::Snowflake;

type Result<T> = result::Result<T, Box<dyn error::Error>>;

type StrongLockedRef<T> = Arc<Mutex<T>>;
type WeakLockedRef<T> = Weak<Mutex<T>>;

pub struct Store<T, U>
where
    U: StoreBackend<T>,
{
    backend: U,
    refs: Mutex<HashMap<Snowflake, WeakLockedRef<T>>>,
}

impl<T, U> Store<T, U>
where
    U: StoreBackend<T>,
{
    pub fn new(backend: U) -> Store<T, U> {
        Store {
            backend,
            refs: Mutex::new(HashMap::new()),
        }
    }

    fn load_new_ref(
        &self,
        map: &mut MutexGuard<HashMap<Snowflake, WeakLockedRef<T>>>,
        id: &Snowflake,
    ) -> Result<StrongLockedRef<T>> {
        let obj = self.backend.load(id)?;
        let r = Arc::new(Mutex::new(obj));
        map.insert(*id, Arc::downgrade(&r));
        Ok(r)
    }

    pub fn load(&self, id: &Snowflake) -> Result<StrongLockedRef<T>> {
        if !self.backend.exists(id)? {
            return Err(Box::new(NotFoundError { id: *id }));
        }

        let mut map = self.refs.lock().unwrap();
        let r: StrongLockedRef<T> = match map.get(id) {
            None => self.load_new_ref(&mut map, id)?,
            Some(wk) => match wk.upgrade() {
                None => self.load_new_ref(&mut map, id)?,
                Some(strong) => strong,
            },
        };

        Ok(r)
    }

    pub fn store(&self, id: &Snowflake, object: &T) -> Result<()> {
        self.backend.store(id, object)
    }

    pub fn exists(&self, id: &Snowflake) -> Result<bool> {
        self.backend.exists(id)
    }

    pub fn delete(&self, id: &Snowflake) -> Result<()> {
        self.backend.delete(id)
    }
}

pub trait StoreBackend<T> {
    fn load(&self, id: &Snowflake) -> Result<T>;
    fn exists(&self, id: &Snowflake) -> Result<bool>;
    fn store(&self, id: &Snowflake, object: &T) -> Result<()>;
    fn delete(&self, id: &Snowflake) -> Result<()>;
}

#[derive(Debug, Clone)]
pub struct NotFoundError {
    id: Snowflake,
}

impl NotFoundError {
    pub fn new(id: &Snowflake) -> NotFoundError {
        NotFoundError { id: *id }
    }
}

impl fmt::Display for NotFoundError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "could not find object {}", self.id)
    }
}

impl error::Error for NotFoundError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::{Arc, RwLock};
    use std::thread;

    use crate::snowflake::SnowflakeGenerator;

    #[derive(Clone)]
    struct MockStoredData {
        id: Snowflake,
        field_a: String,
        field_b: u64,
    }

    struct MockStoreBackend {
        data: RwLock<HashMap<Snowflake, MockStoredData>>,
    }

    impl MockStoreBackend {
        fn new() -> MockStoreBackend {
            MockStoreBackend {
                data: RwLock::new(HashMap::new()),
            }
        }
    }

    impl MockStoredData {
        fn new(id: Snowflake, field_a: String, field_b: u64) -> MockStoredData {
            MockStoredData {
                id,
                field_a,
                field_b,
            }
        }

        fn id<'a>(&'a self) -> &'a Snowflake {
            &self.id
        }
    }

    impl StoreBackend<MockStoredData> for MockStoreBackend {
        fn exists(&self, id: &Snowflake) -> Result<bool> {
            let map = self.data.read().unwrap();
            Ok(map.contains_key(id))
        }

        fn load(&self, id: &Snowflake) -> Result<MockStoredData> {
            let map = self.data.read().unwrap();
            match map.get(id) {
                None => Err(Box::new(NotFoundError::new(id))),
                Some(pl) => Ok(pl.clone()),
            }
        }

        fn store(&self, id: &Snowflake, data: &MockStoredData) -> Result<()> {
            let mut map = self.data.write().unwrap();
            map.insert(*id, data.clone());

            Ok(())
        }

        fn delete(&self, id: &Snowflake) -> Result<()> {
            let mut map = self.data.write().unwrap();
            map.remove(id);

            Ok(())
        }
    }

    type MockStore = Store<MockStoredData, MockStoreBackend>;

    #[test]
    fn test_exists() {
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let backend = MockStoreBackend::new();
        let data = MockStoredData::new(snowflake_gen.generate(), "foo".to_owned(), 1);

        backend.store(data.id(), &data).unwrap();
        let store = MockStore::new(backend);

        let id2 = snowflake_gen.generate();
        assert!(store.exists(data.id()).unwrap());
        assert!(!store.exists(&id2).unwrap());
    }

    #[test]
    fn test_load_nonexistent() {
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let backend = MockStoreBackend::new();
        let store = MockStore::new(backend);
        let result = store.load(&snowflake_gen.generate());

        assert!(result.is_err());
    }

    #[test]
    fn test_load() {
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let backend = MockStoreBackend::new();
        let data = MockStoredData::new(snowflake_gen.generate(), "foo".to_owned(), 1);

        backend.store(data.id(), &data).unwrap();
        let store = MockStore::new(backend);

        let wrapper = store.load(data.id()).unwrap();
        let data_copy = wrapper.lock().unwrap();

        assert_eq!(data.id(), data_copy.id());
        assert_eq!(data.field_a, data_copy.field_a);
        assert_eq!(data.field_b, data_copy.field_b);
    }

    #[test]
    fn test_concurrent_load() {
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let backend = MockStoreBackend::new();
        let id = snowflake_gen.generate();
        let data = MockStoredData::new(id, "foo".to_owned(), 1);

        backend.store(data.id(), &data).unwrap();
        let store = Arc::new(MockStore::new(backend));

        let store2 = store.clone();
        let handle = thread::spawn(move || {
            let wrapper_1 = store2.load(&id).unwrap();
            wrapper_1
        });

        let wrapper_2 = store.load(data.id()).unwrap();
        let wrapper_1 = handle.join().unwrap();

        // wrapper_1 and wrapper_2 should be Arcs pointing to the same
        // data.
        assert!(Arc::ptr_eq(&wrapper_1, &wrapper_2));
    }

    #[test]
    fn test_store() {
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let id = snowflake_gen.generate();
        let data = MockStoredData::new(id, "foo".to_owned(), 1);

        let backend = MockStoreBackend::new();
        let store = MockStore::new(backend);
        store.store(data.id(), &data).unwrap();

        let wrapper = store.load(&id).unwrap();
        let data_copy = wrapper.lock().unwrap();

        assert_eq!(*data_copy.id(), id);
        assert_eq!(data.field_a, data_copy.field_a);
        assert_eq!(data.field_b, data_copy.field_b);
    }
}
