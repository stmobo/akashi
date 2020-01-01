use std::collections::HashMap;
use std::error;
use std::fmt;
use std::result;
use std::sync::{Arc, Mutex, MutexGuard, Weak};

use crate::snowflake::Snowflake;

type Result<T> = result::Result<T, Box<dyn error::Error + Send + Sync>>;

type StrongLockedRef<T> = Arc<Mutex<T>>;
type WeakLockedRef<T> = Weak<Mutex<T>>;

pub trait SharedStore<T, U>
where
    U: StoreBackend<T>,
{
    fn get_store<'a>(&'a self) -> &'a Store<T, U>;
}

pub struct StoreHandle<T, U>
where
    U: StoreBackend<T>,
{
    backend: Arc<U>,
    id: Snowflake,
    object: Option<T>,
}

impl<T, U> StoreHandle<T, U>
where
    U: StoreBackend<T>,
{
    pub fn new(backend: Arc<U>, id: Snowflake, object: Option<T>) -> StoreHandle<T, U> {
        StoreHandle {
            backend,
            id,
            object,
        }
    }

    pub fn get(&self) -> Option<&T> {
        self.object.as_ref()
    }

    pub fn get_mut(&mut self) -> Option<&mut T> {
        self.object.as_mut()
    }

    pub fn replace(&mut self, object: T) {
        self.object = Some(object);
    }

    pub fn id(&self) -> &Snowflake {
        &self.id
    }

    pub fn exists(&self) -> bool {
        self.object.is_some()
    }

    pub fn store(&self) -> Result<()> {
        match &self.object {
            None => self.backend.delete(self.id),
            Some(obj) => self.backend.store(self.id, &obj),
        }
    }

    pub fn delete(&mut self) -> Result<()> {
        self.object = None;
        self.backend.delete(self.id)
    }
}

pub struct Store<T, U>
where
    U: StoreBackend<T>,
{
    backend: Arc<U>,
    refs: Mutex<HashMap<Snowflake, WeakLockedRef<StoreHandle<T, U>>>>,
}

impl<T, U> Store<T, U>
where
    U: StoreBackend<T>,
{
    pub fn new(backend: Arc<U>) -> Store<T, U> {
        Store {
            backend,
            refs: Mutex::new(HashMap::new()),
        }
    }

    fn load_new_ref(
        &self,
        map: &mut MutexGuard<HashMap<Snowflake, WeakLockedRef<StoreHandle<T, U>>>>,
        id: Snowflake,
    ) -> Result<StrongLockedRef<StoreHandle<T, U>>> {
        let obj = self.backend.load(id)?;
        let handle: StoreHandle<T, U> = StoreHandle::new(self.backend.clone(), id, Some(obj));

        let r = Arc::new(Mutex::new(handle));
        map.insert(id, Arc::downgrade(&r));
        Ok(r)
    }

    pub fn load(&self, id: Snowflake) -> Result<StrongLockedRef<StoreHandle<T, U>>> {
        let mut map = self.refs.lock().expect("refs map lock poisoned");

        if !self.backend.exists(id)? {
            let handle: StoreHandle<T, U> = StoreHandle::new(self.backend.clone(), id, None);
            let r = Arc::new(Mutex::new(handle));
            map.insert(id, Arc::downgrade(&r));
            return Ok(r);
        }

        let r: StrongLockedRef<StoreHandle<T, U>> = match map.get(&id) {
            None => self.load_new_ref(&mut map, id)?,
            Some(wk) => match wk.upgrade() {
                None => self.load_new_ref(&mut map, id)?,
                Some(strong) => strong,
            },
        };

        Ok(r)
    }

    pub fn store(&self, id: Snowflake, object: T) -> Result<()> {
        let wrapper = self.load(id)?;
        let mut handle = wrapper.lock().expect("wrapper lock poisoned");
        handle.replace(object);
        handle.store()
    }

    pub fn delete(&self, id: Snowflake) -> Result<()> {
        let wrapper = self.load(id)?;
        let mut handle = wrapper.lock().expect("wrapper lock poisoned");
        handle.delete()
    }

    pub fn exists(&self, id: Snowflake) -> Result<bool> {
        self.backend.exists(id)
    }

    pub fn keys(&self, page: u64, limit: u64) -> Result<Vec<Snowflake>> {
        self.backend.keys(page, limit)
    }
}

pub trait StoreBackend<T> {
    fn load(&self, id: Snowflake) -> Result<T>;
    fn exists(&self, id: Snowflake) -> Result<bool>;
    fn store(&self, id: Snowflake, object: &T) -> Result<()>;
    fn delete(&self, id: Snowflake) -> Result<()>;
    fn keys(&self, page: u64, limit: u64) -> Result<Vec<Snowflake>>;
}

#[derive(Debug, Clone)]
pub struct NotFoundError {
    id: Snowflake,
}

impl NotFoundError {
    pub fn new(id: Snowflake) -> NotFoundError {
        NotFoundError { id }
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
        fn exists(&self, id: Snowflake) -> Result<bool> {
            let map = self.data.read().unwrap();
            Ok(map.contains_key(&id))
        }

        fn load(&self, id: Snowflake) -> Result<MockStoredData> {
            let map = self.data.read().unwrap();
            match map.get(&id) {
                None => Err(Box::new(NotFoundError::new(id))),
                Some(pl) => Ok(pl.clone()),
            }
        }

        fn store(&self, id: Snowflake, data: &MockStoredData) -> Result<()> {
            let mut map = self.data.write().unwrap();
            map.insert(id, data.clone());

            Ok(())
        }

        fn delete(&self, id: Snowflake) -> Result<()> {
            let mut map = self.data.write().unwrap();
            map.remove(&id);

            Ok(())
        }

        fn keys(&self, page: u64, limit: u64) -> Result<Vec<Snowflake>> {
            let ids: Vec<Snowflake>;
            let start_index = page * limit;

            let data = self.data.read().unwrap();
            ids = data
                .keys()
                .skip(start_index as usize)
                .take(limit as usize)
                .map(|x| *x)
                .collect();

            Ok(ids)
        }
    }

    type MockStore = Store<MockStoredData, MockStoreBackend>;

    #[test]
    fn test_exists() {
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let backend = Arc::new(MockStoreBackend::new());
        let data = MockStoredData::new(snowflake_gen.generate(), "foo".to_owned(), 1);

        backend.store(*data.id(), &data).unwrap();
        let store = MockStore::new(backend);

        let id2 = snowflake_gen.generate();
        assert!(store.exists(*data.id()).unwrap());
        assert!(!store.exists(id2).unwrap());
    }

    #[test]
    fn test_load_nonexistent() {
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let backend = Arc::new(MockStoreBackend::new());
        let store = MockStore::new(backend);
        let result = store.load(snowflake_gen.generate()).unwrap();

        let handle = result.lock().unwrap();
        assert!(!handle.exists());
    }

    #[test]
    fn test_load() {
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let backend = Arc::new(MockStoreBackend::new());
        let data = MockStoredData::new(snowflake_gen.generate(), "foo".to_owned(), 1);

        backend.store(*data.id(), &data).unwrap();
        let store = MockStore::new(backend);

        let wrapper = store.load(*data.id()).unwrap();
        let handle = wrapper.lock().unwrap();

        assert!(handle.exists());
        let data_copy = handle.get().unwrap();

        assert_eq!(*data.id(), *data_copy.id());
        assert_eq!(data.field_a, data_copy.field_a);
        assert_eq!(data.field_b, data_copy.field_b);
    }

    #[test]
    fn test_concurrent_load() {
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let backend = Arc::new(MockStoreBackend::new());
        let id = snowflake_gen.generate();
        let data = MockStoredData::new(id, "foo".to_owned(), 1);

        backend.store(*data.id(), &data).unwrap();
        let store = Arc::new(MockStore::new(backend));

        let store2 = store.clone();
        let handle = thread::spawn(move || {
            let wrapper_1 = store2.load(id).unwrap();
            wrapper_1
        });

        let wrapper_2 = store.load(*data.id()).unwrap();
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

        let backend = Arc::new(MockStoreBackend::new());
        let store = MockStore::new(backend);

        {
            let wrapper = store.load(*data.id()).unwrap();
            let mut handle = wrapper.lock().unwrap();
            assert!(!handle.exists());

            handle.replace(data.clone());
            handle.store().unwrap();
        }

        let wrapper = store.load(*data.id()).unwrap();
        let handle = wrapper.lock().unwrap();
        let data_copy = handle.get().unwrap();

        assert_eq!(*data_copy.id(), id);
        assert_eq!(data.field_a, data_copy.field_a);
        assert_eq!(data.field_b, data_copy.field_b);
    }
}
