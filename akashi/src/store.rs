//! Akashi's storage system for `Entities`.

use std::cell::Cell;
use std::ops::Deref;
use std::sync::{Arc, Mutex, MutexGuard, Weak};

extern crate stable_deref_trait;
use stable_deref_trait::StableDeref;

use chashmap::CHashMap;
use failure::err_msg;

use crate::ecs::{ComponentManager, Entity};
use crate::snowflake::Snowflake;
use crate::util::Result;

type StrongSharedMutex<T> = Arc<Mutex<T>>;
type WeakSharedMutex<T> = Weak<Mutex<T>>;

rental! {
    pub mod handle_ref {
        //! A self-referential wrapper around an `Arc`ed `Mutex`.
        use super::*;

        /// A self-referential type that wraps `Arc<Mutex<StoreHandle>>` into
        /// something more manageable.
        ///
        /// This struct contains both an `Arc<Mutex<StoreHandle>>` and
        /// a `MutexGuard` for that same handle, hence why this requires
        /// the `rental` crate.
        ///
        /// This type supports `Deref` and `DerefMut` to `StoreHandle`,
        /// which is probably all you'll need to use.
        #[rental(debug, clone, deref_mut_suffix, covariant, map_suffix = "T")]
        pub struct HandleRef<H: StableDeref + Deref + 'static, T: 'static> {
            head: H,
            suffix: MutexGuard<'head, T>,
        }
    }
}

type StoreHandleReference<T> = handle_ref::HandleRef<Arc<Mutex<T>>, T>;

/// Converts `Arc<Mutex<T>>` to a `HandleRef` by locking the inner `Mutex`.
fn rent_arced_mutex<T: 'static>(head: Arc<Mutex<T>>) -> Result<StoreHandleReference<T>> {
    handle_ref::HandleRef::try_new(head, |s| {
        s.lock().map_err(|_e| err_msg("wrapper lock poisoned"))
    })
    .map_err(|e| e.0)
}

/// This is a trait for wrapping up objects that contain stores for
/// multiple types of `Entity`.
pub trait SharedStore<T, U>
where
    T: Entity + 'static,
    U: StoreBackend<T> + 'static,
{
    fn get_store<'a>(&'a self) -> &'a Store<T, U>;
}

/// A shared handle to an `Entity`.
///
/// # Errors
///
/// Most of the methods associated with `StoreHandle` call methods on
/// storage backend objects. Errors returned by these methods will bubble up
/// through `StoreHandle`'s methods.
pub struct StoreHandle<T, U>
where
    T: Entity + 'static,
    U: StoreBackend<T> + 'static,
{
    backend: Arc<U>,
    id: Snowflake,
    object: Option<T>,
    initialized: bool,
}

impl<T, U> StoreHandle<T, U>
where
    T: Entity + 'static,
    U: StoreBackend<T> + 'static,
{
    fn new(backend: Arc<U>, id: Snowflake, object: Option<T>) -> StoreHandle<T, U> {
        StoreHandle {
            backend,
            id,
            object,
            initialized: false,
        }
    }

    /// Gets a reference to the object within this handle.
    pub fn get(&self) -> Option<&T> {
        assert!(self.initialized);
        self.object.as_ref()
    }

    /// Gets a mutable reference to the object within this handle.
    pub fn get_mut(&mut self) -> Option<&mut T> {
        assert!(self.initialized);
        self.object.as_mut()
    }

    /// Replaces the object within this handle with something else.
    pub fn replace(&mut self, object: T) -> Option<T> {
        let prev_object = self.object.replace(object);
        let prev_initialized = self.initialized;
        self.initialized = true;

        if prev_initialized {
            prev_object
        } else {
            None
        }
    }

    /// Gets the ID of the `Entity` in this handle.
    pub fn id(&self) -> Snowflake {
        self.id
    }

    /// Checks whether anything is actually contained in this handle.
    pub fn exists(&self) -> bool {
        assert!(self.initialized);
        self.object.is_some()
    }

    /// Puts whatever is in this handle into storage.
    pub fn store(&self) -> Result<()> {
        assert!(self.initialized);

        match &self.object {
            None => self.backend.delete(self.id),
            Some(obj) => self.backend.store(self.id, &obj),
        }
    }

    /// Clears out the data in this handle, then deletes the `Entity`
    /// from storage.
    pub fn delete(&mut self) -> Result<()> {
        if let Some(obj) = &mut self.object {
            obj.clear_components()?;
        }

        self.object = None;
        self.initialized = true;
        self.backend.delete(self.id)
    }

    fn set_object(&mut self, object: Option<T>) {
        self.object = object;
        self.initialized = true;
    }

    fn initialized(&self) -> bool {
        self.initialized
    }
}

/// Handles storing `Entities` and coordinating access to them across
/// multiple threads.
///
/// # Errors
///
/// Most of the methods associated with `Store` call methods on storage
/// backend objects. Errors returned by these methods will bubble up
/// through `Store`'s methods.
pub struct Store<T, U>
where
    T: Entity + 'static,
    U: StoreBackend<T> + 'static,
{
    backend: Arc<U>,
    refs: CHashMap<Snowflake, WeakSharedMutex<StoreHandle<T, U>>>,
}

impl<T, U> Store<T, U>
where
    T: Entity + 'static,
    U: StoreBackend<T> + 'static,
{
    /// Creates a new `Store` using the given storage backend.
    pub fn new(backend: Arc<U>) -> Store<T, U> {
        Store {
            backend,
            refs: CHashMap::new(),
        }
    }

    /// Retrieves or creates a possibly-uninitialized StoreHandle from
    /// the underlying hashmap.
    fn get_handle(&self, id: Snowflake) -> Result<StrongSharedMutex<StoreHandle<T, U>>> {
        let ret_cell: Cell<Result<StrongSharedMutex<StoreHandle<T, U>>>> =
            Cell::new(Err(err_msg("unknown load error")));

        // All of this needs to be done with a write lock on the bucket
        // for this hashmap entry.
        self.refs.alter(id, |val| {
            // If a previously-left weak pointer is still around, use that.
            if let Some(weak) = val.clone() {
                if let Some(strong) = weak.upgrade() {
                    // use _e here so that the compiler stops complaining
                    // about us not using a Result
                    let _e = ret_cell.replace(Ok(strong.clone()));
                    return Some(weak);
                }
            }

            // Create a new handle and store it into the hashmap.
            // This handle starts uninitialized.
            let handle: StoreHandle<T, U> = StoreHandle::new(self.backend.clone(), id, None);
            let ret = Arc::new(Mutex::new(handle));
            let weak = Arc::downgrade(&ret);

            let _e = ret_cell.replace(Ok(ret));
            Some(weak)
        });

        ret_cell.into_inner()
    }

    /// Loads the `Entity` with the given ID from storage, or get a
    /// handle to the `Entity` if it's already been loaded by another
    /// thread.
    ///
    /// This not only loads a handle to the `Entity` from the store,
    /// but also locks it for you. As such, the return type of this
    /// function is a smart reference to the loaded `StoreHandle`.
    pub fn load(
        &self,
        id: Snowflake,
        cm: Arc<ComponentManager<T>>,
    ) -> Result<StoreHandleReference<StoreHandle<T, U>>> {
        let wrapper = self.get_handle(id)?;
        let mut handle = rent_arced_mutex(wrapper)?;

        if !handle.initialized() {
            handle.set_object(self.backend.load(id, cm)?);
        }

        Ok(handle)
    }

    /// Puts the given `Entity` into storage, overwriting any previously
    /// stored `Entity` data with the same ID.
    pub fn store(&self, object: T) -> Result<()> {
        let id = object.id();
        let wrapper = self.get_handle(id)?;
        let mut handle = wrapper
            .lock()
            .map_err(|_e| format_err!("wrapper lock poisoned"))?;

        handle.set_object(Some(object));
        handle.store()
    }

    /// Deletes the `Entity` with the given ID from storage.
    ///
    /// Note that internally, this method loads the `Entity` prior to
    /// deleting it, so that attached `Component`s are properly deleted.
    ///
    /// If you already have an open handle to the `Entity`, you should
    /// use `StoreHandle::delete()` instead.
    pub fn delete(&self, id: Snowflake, cm: Arc<ComponentManager<T>>) -> Result<()> {
        let mut handle = self.load(id, cm)?;
        handle.delete()
    }

    /// Checks to see if an `Entity` with the given ID exists.
    pub fn exists(&self, id: Snowflake) -> Result<bool> {
        let wrapper = self.get_handle(id)?;
        let handle = wrapper
            .lock()
            .map_err(|_e| format_err!("wrapper lock poisoned"))?;

        if handle.initialized() {
            Ok(handle.exists())
        } else {
            self.backend.exists(id)
        }
    }

    /// Retrieves a list of `Entity` IDs from storage.
    pub fn keys(&self, page: u64, limit: u64) -> Result<Vec<Snowflake>> {
        self.backend.keys(page, limit)
    }
}

/// This trait is used to mark backing storage objects for `Entities`.
///
/// Structs that implement this trait can be used as backing storage
/// for `Entities` such as `Player`s and `Cards`, and can be passed to
/// `Store::new`.
pub trait StoreBackend<T: Entity + 'static> {
    /// Loads data for an `Entity` from storage, if any `Entity` with
    /// the given ID exists.
    fn load(&self, id: Snowflake, cm: Arc<ComponentManager<T>>) -> Result<Option<T>>;

    /// Checks to see if an `Entity` with the given ID exists in storage.
    fn exists(&self, id: Snowflake) -> Result<bool>;

    /// Saves data for an `Entity` to storage.
    fn store(&self, id: Snowflake, object: &T) -> Result<()>;

    /// Deletes data for an `Entity` from storage.
    fn delete(&self, id: Snowflake) -> Result<()>;

    /// Retrieve a list of `Entity` IDs from storage.
    fn keys(&self, page: u64, limit: u64) -> Result<Vec<Snowflake>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::any::TypeId;
    use std::collections::{HashMap, HashSet};
    use std::sync::{Arc, RwLock};
    use std::thread;

    use crate::snowflake::SnowflakeGenerator;

    #[derive(Clone)]
    struct MockStoredData {
        id: Snowflake,
        field_a: String,
        field_b: u64,
        cm: Arc<ComponentManager<MockStoredData>>,
        components_attached: HashSet<TypeId>,
    }

    impl Entity for MockStoredData {
        fn id(&self) -> Snowflake {
            self.id
        }

        fn component_manager(&self) -> &ComponentManager<MockStoredData> {
            &self.cm
        }

        fn components_attached(&self) -> &HashSet<TypeId> {
            &self.components_attached
        }

        fn components_attached_mut(&mut self) -> &mut HashSet<TypeId> {
            &mut self.components_attached
        }
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
                cm: Arc::new(ComponentManager::new()),
                components_attached: HashSet::new(),
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

        fn load(
            &self,
            id: Snowflake,
            _cm: Arc<ComponentManager<MockStoredData>>,
        ) -> Result<Option<MockStoredData>> {
            let map = self.data.read().unwrap();
            match map.get(&id) {
                None => Ok(None),
                Some(pl) => Ok(Some(pl.clone())),
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
        let handle = store
            .load(snowflake_gen.generate(), Arc::new(ComponentManager::new()))
            .unwrap();

        assert!(!handle.exists());
    }

    #[test]
    fn test_load() {
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let backend = Arc::new(MockStoreBackend::new());
        let data = MockStoredData::new(snowflake_gen.generate(), "foo".to_owned(), 1);

        backend.store(*data.id(), &data).unwrap();
        let store = MockStore::new(backend);

        let handle = store
            .load(*data.id(), Arc::new(ComponentManager::new()))
            .unwrap();

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
            let wrapper_1 = store2.get_handle(id).unwrap();
            wrapper_1
        });

        let wrapper_2 = store.get_handle(*data.id()).unwrap();
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
        let cm = Arc::new(ComponentManager::new());

        {
            let mut handle = store.load(*data.id(), cm.clone()).unwrap();
            assert!(!handle.exists());

            handle.replace(data.clone());
            handle.store().unwrap();
        }

        let handle = store.load(*data.id(), cm).unwrap();
        let data_copy = handle.get().unwrap();

        assert_eq!(*data_copy.id(), id);
        assert_eq!(data.field_a, data_copy.field_a);
        assert_eq!(data.field_b, data_copy.field_b);
    }
}
