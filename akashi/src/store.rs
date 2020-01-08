//! Akashi's storage system for `Entities`.

use std::any;
use std::cell::Cell;
use std::fmt;
use std::ops::Deref;
use std::sync::{Arc, Weak};

extern crate stable_deref_trait;
use stable_deref_trait::CloneStableDeref;

use chashmap::CHashMap;
use failure::err_msg;
use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::ecs::{ComponentManager, Entity};
use crate::snowflake::Snowflake;
use crate::util::Result;

rental! {
    pub mod handle_ref {
        //! Self-referential wrappers around shared locks.
        use super::*;

        /// A self-referential type that wraps `Arc<RwLock<StoreHandle>>`
        /// into a single immutable reference.
        ///
        /// This struct effectively contains an `Arc` pointing to an
        /// `RwLock` and a read guard for that same lock.
        ///
        /// It also supports `Deref` to the inner StoreHandle, which is
        /// usually all you'll need.
        #[rental(debug, clone, deref_suffix, covariant, map_suffix = "T")]
        pub struct HandleReadRef<H: CloneStableDeref + Deref + 'static, T: 'static> {
            head: H,
            suffix: RwLockReadGuard<'head, T>,
        }

        /// A self-referential type that wraps `Arc<RwLock<StoreHandle>>`
        /// into a single mutable reference.
        ///
        /// This struct effectively contains an `Arc` pointing to an
        /// `RwLock` and a write guard for that same lock.
        ///
        /// It also supports `Deref` and `DerefMut` to the inner
        /// StoreHandle, which is usually all you'll need.
        #[rental(debug, clone, deref_mut_suffix, covariant, map_suffix = "T")]
        pub struct HandleWriteRef<H: CloneStableDeref + Deref + 'static, T: 'static> {
            head: H,
            suffix: RwLockWriteGuard<'head, T>,
        }
    }
}

pub use handle_ref::{HandleReadRef, HandleWriteRef};

type StoreReference<T> = Arc<RwLock<T>>;
type WeakStoreReference<T> = Weak<RwLock<T>>;
pub type ReadReference<T> = HandleReadRef<StoreReference<T>, T>;
pub type WriteReference<T> = HandleWriteRef<StoreReference<T>, T>;

/// Converts `Arc<RwLock<T>>` to a `ReadReference` by taking the inner read lock.
pub fn read_store_reference<T: 'static>(head: StoreReference<T>) -> ReadReference<T> {
    HandleReadRef::new(head, |s| s.read())
}

/// Converts `Arc<RwLock<T>>` to a `WriteReference` by locking the inner write lock.
pub fn write_store_reference<T: 'static>(head: StoreReference<T>) -> WriteReference<T> {
    HandleWriteRef::new(head, |s| s.write())
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

/// A shared handle to an `Entity` and its storage backend.
///
/// # Errors
///
/// Most of the methods associated with `StoreHandle` call methods on
/// storage backend objects. Errors returned by these methods will bubble up
/// through `StoreHandle`'s methods.
pub struct StoreHandle<T>
where
    T: Entity + 'static,
{
    backend: Arc<dyn StoreBackend<T> + Sync + Send + 'static>,
    id: Snowflake,
    object: Option<T>,
    initialized: bool,
}

impl<T> StoreHandle<T>
where
    T: Entity + 'static,
{
    fn new<U>(backend: Arc<U>, id: Snowflake, object: Option<T>) -> StoreHandle<T>
    where
        U: StoreBackend<T> + Sync + Send + 'static,
    {
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
    refs: CHashMap<Snowflake, WeakStoreReference<StoreHandle<T>>>,
}

impl<T, U> Store<T, U>
where
    T: Entity + 'static,
    U: StoreBackend<T> + Sync + Send + 'static,
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
    fn get_handle(&self, id: Snowflake) -> Result<StoreReference<StoreHandle<T>>> {
        let ret_cell: Cell<Result<StoreReference<StoreHandle<T>>>> =
            Cell::new(Err(err_msg("unknown load error")));

        // All of this needs to be done with a write lock on the bucket
        // for this hashmap entry.
        self.refs.alter(id, |val| {
            // If a previously-retrieved handle is still around, use that.
            if let Some(weak) = val.clone() {
                if let Some(strong) = weak.upgrade() {
                    // use _e here so that the compiler stops complaining
                    // about us not using a Result
                    let _e = ret_cell.replace(Ok(strong));
                    return val;
                }
            }

            // Create a new handle and store it into the hashmap.
            // This handle starts uninitialized.
            let handle: StoreHandle<T> = StoreHandle::new(self.backend.clone(), id, None);
            let ret = Arc::new(RwLock::new(handle));
            let weak = Arc::downgrade(&ret);

            let _e = ret_cell.replace(Ok(ret));
            Some(weak)
        });

        ret_cell.into_inner()
    }

    /// Gets an immutable reference to the handle for the `Entity` with
    /// the given ID.
    ///
    /// Data for the `Entity` will be loaded from storage if needed.
    ///
    /// The returned reference is read-locked, so multiple threads can
    /// use references from this function at once.
    pub fn load(
        &self,
        id: Snowflake,
        cm: Arc<ComponentManager<T>>,
    ) -> Result<ReadReference<StoreHandle<T>>> {
        let wrapper = self.get_handle(id)?;

        {
            let mut write_handle = wrapper.write();
            if !write_handle.initialized() {
                write_handle.set_object(self.backend.load(id, cm)?);
            }
        }

        let handle = read_store_reference(wrapper);
        Ok(handle)
    }

    /// Gets a mutable reference to the handle for the `Entity` with
    /// the given ID.
    ///
    /// Data for the `Entity` will be loaded from storage if needed.
    ///
    /// The returned reference is write-locked, so exclusive access to
    /// the handle is ensured.
    pub fn load_mut(
        &self,
        id: Snowflake,
        cm: Arc<ComponentManager<T>>,
    ) -> Result<WriteReference<StoreHandle<T>>> {
        let wrapper = self.get_handle(id)?;
        let mut handle = write_store_reference(wrapper);

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
        let mut handle = wrapper.write();

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
        let mut handle = self.load_mut(id, cm)?;
        handle.delete()
    }

    /// Checks to see if an `Entity` with the given ID exists.
    pub fn exists(&self, id: Snowflake) -> Result<bool> {
        let wrapper = self.get_handle(id)?;
        let handle = wrapper.read();

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

/// An interface for loading and storing [`Entities`](Entity).
///
/// This trait provides an abstract interface for loading and storing
/// [`Entities`](Entity) objects.
///
/// For example, you can use it as a way to access [`Stores`](Store)
/// without having to carry around a [`StoreBackend`](StoreBackend)
/// type parameter everywhere. This comes at the cost of dynamic dispatch
/// overhead, though.
pub trait EntityStore<T>
where
    T: Entity + 'static,
{
    fn load(
        &self,
        id: Snowflake,
        cm: Arc<ComponentManager<T>>,
    ) -> Result<ReadReference<StoreHandle<T>>>;

    fn load_mut(
        &self,
        id: Snowflake,
        cm: Arc<ComponentManager<T>>,
    ) -> Result<WriteReference<StoreHandle<T>>>;

    fn store(&self, object: T) -> Result<()>;
    fn delete(&self, id: Snowflake, cm: Arc<ComponentManager<T>>) -> Result<()>;
    fn exists(&self, id: Snowflake) -> Result<bool>;
    fn keys(&self, page: u64, limit: u64) -> Result<Vec<Snowflake>>;
}

impl<T, U> EntityStore<T> for Store<T, U>
where
    T: Entity + 'static,
    U: StoreBackend<T> + Sync + Send + 'static,
{
    fn load(
        &self,
        id: Snowflake,
        cm: Arc<ComponentManager<T>>,
    ) -> Result<ReadReference<StoreHandle<T>>> {
        self.load(id, cm)
    }

    fn load_mut(
        &self,
        id: Snowflake,
        cm: Arc<ComponentManager<T>>,
    ) -> Result<WriteReference<StoreHandle<T>>> {
        self.load_mut(id, cm)
    }

    fn store(&self, object: T) -> Result<()> {
        self.store(object)
    }

    fn delete(&self, id: Snowflake, cm: Arc<ComponentManager<T>>) -> Result<()> {
        self.delete(id, cm)
    }

    fn exists(&self, id: Snowflake) -> Result<bool> {
        self.exists(id)
    }
    fn keys(&self, page: u64, limit: u64) -> Result<Vec<Snowflake>> {
        self.keys(page, limit)
    }
}

impl<T, U> fmt::Debug for Store<T, U>
where
    T: Entity + 'static,
    U: StoreBackend<T> + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Store<{}, {}> {{ {} keys }}",
            any::type_name::<T>(),
            any::type_name::<U>(),
            self.refs.len()
        )
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
            let mut handle = store.load_mut(*data.id(), cm.clone()).unwrap();
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
