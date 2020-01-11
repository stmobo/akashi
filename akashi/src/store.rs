//! Akashi's storage system for [`Entities`](Entity).

use std::any;
use std::fmt;
use std::ops::Deref;
use std::sync::{Arc, Weak};

extern crate stable_deref_trait;
use stable_deref_trait::CloneStableDeref;

use dashmap::DashMap;
use parking_lot::{Once, RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::ecs::{ComponentManager, Entity};
use crate::snowflake::Snowflake;
use crate::util::Result;

rental! {
    pub mod handle_ref {
        //! Self-referential wrappers around shared locks.
        use super::*;

        /// A self-referential type that wraps `Arc<RwLock<StoreHandle>>`
        /// into a read-locked immutable reference.
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
        /// into a write-locked mutable reference.
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

pub type StoreReference<T> = Arc<RwLock<T>>;
type WeakStoreReference<T> = Weak<RwLock<T>>;
pub type ReadReference<T> = HandleReadRef<StoreReference<T>, T>;
pub type WriteReference<T> = HandleWriteRef<StoreReference<T>, T>;

/// Converts `Arc<RwLock<T>>` to a [`ReadReference`] by taking the inner read lock.
pub fn read_store_reference<T: 'static>(head: StoreReference<T>) -> ReadReference<T> {
    HandleReadRef::new(head, |s| s.read())
}

/// Converts `Arc<RwLock<T>>` to a [`WriteReference`] by locking the inner write lock.
pub fn write_store_reference<T: 'static>(head: StoreReference<T>) -> WriteReference<T> {
    HandleWriteRef::new(head, |s| s.write())
}

/// This is a trait for wrapping up objects that contain stores for
/// multiple types of [`Entity`].
pub trait SharedStore<T, U>
where
    T: Entity + 'static,
    U: StoreBackend<T> + 'static,
{
    fn get_store<'a>(&'a self) -> &'a Store<T, U>;
}

/// A shared handle to an [`Entity`] and its storage backend.
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
        }
    }

    /// Gets a reference to the object within this handle.
    pub fn get(&self) -> Option<&T> {
        self.object.as_ref()
    }

    /// Gets a mutable reference to the object within this handle.
    pub fn get_mut(&mut self) -> Option<&mut T> {
        self.object.as_mut()
    }

    /// Replaces the object within this handle with something else.
    pub fn replace(&mut self, object: T) -> Option<T> {
        self.object.replace(object)
    }

    /// Gets the ID of the [`Entity`] in this handle.
    pub fn id(&self) -> Snowflake {
        self.id
    }

    /// Checks whether anything is actually contained in this handle.
    pub fn exists(&self) -> bool {
        self.object.is_some()
    }

    /// Puts whatever is in this handle into storage.
    pub fn store(&self) -> Result<()> {
        match &self.object {
            None => self.backend.delete(self.id),
            Some(obj) => self.backend.store(self.id, &obj),
        }
    }

    /// Clears out the data in this handle, then deletes the [`Entity`]
    /// from storage.
    pub fn delete(&mut self) -> Result<()> {
        if let Some(obj) = &mut self.object {
            obj.clear_components()?;
        }

        self.object = None;
        self.backend.delete(self.id)
    }

    fn set_object(&mut self, object: Option<T>) {
        self.object = object;
    }
}

#[doc(hidden)]
#[derive(Clone)]
pub struct StoredHandleData<T>
where
    T: Entity + 'static,
{
    initializer: Arc<Once>,
    handle: WeakStoreReference<StoreHandle<T>>,
}

// Strong version of StoredHandleData
#[doc(hidden)]
#[derive(Clone)]
pub struct HandleData<T>
where
    T: Entity + 'static,
{
    initializer: Arc<Once>,
    handle: StoreReference<StoreHandle<T>>,
}

/// Handles storing [`Entities`](Entity) and coordinating access to
/// them across multiple threads.
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
    refs: DashMap<Snowflake, StoredHandleData<T>>,
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
            refs: DashMap::new(),
        }
    }

    /// Retrieves or creates a possibly-uninitialized [`StoreHandle`] from
    /// the underlying hashmap.
    fn get_handle(&self, id: Snowflake) -> HandleData<T> {
        let mut entry = self.refs.entry(id).or_insert_with(|| StoredHandleData {
            initializer: Arc::new(Once::new()),
            handle: Weak::new(),
        });

        if let Some(strong) = entry.handle.upgrade() {
            HandleData {
                initializer: entry.initializer.clone(),
                handle: strong,
            }
        } else {
            let handle: StoreHandle<T> = StoreHandle::new(self.backend.clone(), id, None);
            let initializer = Arc::new(Once::new());
            let strong = Arc::new(RwLock::new(handle));

            entry.handle = Arc::downgrade(&strong);
            entry.initializer = initializer.clone();

            HandleData {
                initializer,
                handle: strong,
            }
        }
    }

    // Initializes a handle by loading data from the backend.
    // Uses the `initializer` contained in the handle_data to ensure
    // that only one thread tries to initialize at once.
    fn initialize_handle(
        &self,
        id: Snowflake,
        cm: Arc<ComponentManager<T>>,
        handle_data: HandleData<T>,
    ) -> Result<HandleData<T>> {
        let mut res: Result<()> = Result::Ok(());

        handle_data.initializer.call_once(|| {
            let mut write_handle = handle_data.handle.write();
            match self.backend.load(id, cm) {
                Err(e) => {
                    res = Err(e);
                    write_handle.set_object(None);
                }
                Ok(data) => {
                    write_handle.set_object(data);
                }
            };
        });

        match res {
            Err(e) => Err(e),
            Ok(_v) => Ok(handle_data),
        }
    }

    pub fn load_handle(
        &self,
        id: Snowflake,
        cm: Arc<ComponentManager<T>>,
    ) -> Result<StoreReference<StoreHandle<T>>> {
        let handle_data = self.initialize_handle(id, cm, self.get_handle(id))?;
        Ok(handle_data.handle.clone())
    }

    /// Gets an immutable reference to the handle for the [`Entity`]
    /// with the given ID.
    ///
    /// Data for the [`Entity`] will be loaded from storage if needed.
    ///
    /// The returned reference is read-locked, so multiple threads can
    /// use references from this function at once.
    pub fn load(
        &self,
        id: Snowflake,
        cm: Arc<ComponentManager<T>>,
    ) -> Result<ReadReference<StoreHandle<T>>> {
        let handle_data = self.initialize_handle(id, cm, self.get_handle(id))?;
        Ok(read_store_reference(handle_data.handle))
    }

    /// Gets a mutable reference to the handle for the [`Entity`] with
    /// the given ID.
    ///
    /// Data for the [`Entity`] will be loaded from storage if needed.
    ///
    /// The returned reference is write-locked, so exclusive access to
    /// the handle is ensured.
    pub fn load_mut(
        &self,
        id: Snowflake,
        cm: Arc<ComponentManager<T>>,
    ) -> Result<WriteReference<StoreHandle<T>>> {
        let handle_data = self.initialize_handle(id, cm, self.get_handle(id))?;
        Ok(write_store_reference(handle_data.handle))
    }

    /// Puts the given [`Entity`] into storage, overwriting any previously
    /// stored [`Entity`] data with the same ID.
    pub fn store(&self, object: T) -> Result<()> {
        let id = object.id();
        let handle_data = self.get_handle(id);

        // If the initializer gets called, `object` gets set to None,
        // and initializer_result is filled in with the result value.
        //
        // If the initializer is _not_ called, `object` remains as Some(object),
        // and initializer_result is None.
        let mut object: Option<T> = Some(object);
        let mut initializer_result: Option<Result<()>> = None;

        handle_data.initializer.call_once(|| {
            let mut handle = handle_data.handle.write();
            handle.set_object(object.take());
            initializer_result = Some(handle.store());
        });

        if let Some(obj) = object {
            let mut handle = handle_data.handle.write();
            handle.set_object(Some(obj));
            handle.store()
        } else {
            // This should be safe, because in the initializer,
            // object.take() is immediately followed by setting
            // initializer_result to some result.
            initializer_result.unwrap()
        }
    }

    /// Deletes the [`Entity`] with the given ID from storage.
    ///
    /// Note that internally, this method loads the [`Entity`] prior to
    /// deleting it, so that attached [`Components`](crate::Component) are
    /// properly deleted.
    ///
    /// If you already have an open handle to the [`Entity`], you should
    /// use [`StoreHandle::delete`] instead.
    pub fn delete(&self, id: Snowflake, cm: Arc<ComponentManager<T>>) -> Result<()> {
        let mut handle = self.load_mut(id, cm)?;
        handle.delete()
    }

    /// Checks to see if an [`Entity`] with the given ID exists.
    pub fn exists(&self, id: Snowflake) -> Result<bool> {
        let handle_data = self.get_handle(id);

        if handle_data.initializer.state().done() {
            let read_lock = handle_data.handle.read();
            Ok(read_lock.exists())
        } else {
            self.backend.exists(id)
        }
    }

    /// Retrieves a list of [`Entity`] IDs from storage.
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

/// This trait is used to mark backing storage objects for [`Entities`](Entity).
///
/// Structs that implement this trait can be used as backing storage
/// for [`Entities`](Entity) such as [`Players`](crate::Player) and
/// [`Cards`](crate::Card), and can be passed to [`Store::new`].
pub trait StoreBackend<T: Entity + 'static> {
    /// Loads data for an [`Entity`] from storage, if any [`Entity`] with
    /// the given ID exists.
    fn load(&self, id: Snowflake, cm: Arc<ComponentManager<T>>) -> Result<Option<T>>;

    /// Checks to see if an [`Entity`] with the given ID exists in storage.
    fn exists(&self, id: Snowflake) -> Result<bool>;

    /// Saves data for an [`Entity`] to storage.
    fn store(&self, id: Snowflake, object: &T) -> Result<()>;

    /// Deletes data for an [`Entity`] from storage.
    fn delete(&self, id: Snowflake) -> Result<()>;

    /// Retrieve a list of [`Entity`] IDs from storage.
    fn keys(&self, page: u64, limit: u64) -> Result<Vec<Snowflake>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::any::TypeId;
    use std::collections::{HashMap, HashSet};
    use std::sync::{mpsc, Arc, Barrier, RwLock};
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
        remove_on_load: bool,
    }

    impl MockStoreBackend {
        fn new() -> MockStoreBackend {
            MockStoreBackend {
                data: RwLock::new(HashMap::new()),
                remove_on_load: false,
            }
        }

        fn set_remove_on_load(&mut self, flag: bool) {
            self.remove_on_load = flag;
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
            if !self.remove_on_load {
                let map = self.data.read().unwrap();
                Ok(map.get(&id).map(|pl| pl.clone()))
            } else {
                let mut map = self.data.write().unwrap();
                let res = Ok(map.get(&id).map(|pl| pl.clone()));
                map.remove(&id);
                res
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
        // Create some test data to load.
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let backend = Arc::new(MockStoreBackend::new());
        let id = snowflake_gen.generate();
        let data = MockStoredData::new(id, "foo".to_owned(), 1);
        backend.store(id, &data).unwrap();

        // Create a bunch of threads.
        // These threads will all try to access the same data at the same
        // time.
        let store = Arc::new(MockStore::new(backend));
        let mut threads = Vec::with_capacity(9);
        let barrier = Arc::new(Barrier::new(10));

        for _ in 0..9 {
            let b_clone = barrier.clone();
            let s_clone = store.clone();

            let handle = thread::spawn(move || {
                b_clone.wait();
                let wrapper = s_clone.get_handle(id);
                wrapper
            });

            threads.push(handle);
        }

        barrier.wait();
        let our_wrapper = store.get_handle(id);

        for thread in threads {
            // Check what the other threads got for a handle.
            // Both wrapper objects should contain Arcs pointing to the
            // same data.
            let their_wrapper = thread.join().unwrap();
            assert!(Arc::ptr_eq(&our_wrapper.handle, &their_wrapper.handle));
        }
    }

    #[test]
    fn test_concurrent_access() {
        // Create some test data to load.
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let mut backend = MockStoreBackend::new();
        let id = snowflake_gen.generate();
        let data = MockStoredData::new(id, "foo".to_owned(), 1);

        // Detect if and when we end up performing multiple backend loads.
        backend.set_remove_on_load(true);
        backend.store(id, &data).unwrap();

        // Like in test_concurrent_load, create a bunch of threads primed
        // to access the data simultaneously.
        //
        // Unlike in test_concurrent_load, however, these threads will attempt
        // a full retrieval from the backend.
        let store = Arc::new(MockStore::new(Arc::new(backend)));
        let barrier = Arc::new(Barrier::new(10));
        let mut threads = Vec::with_capacity(9);
        let (tx, rx) = mpsc::channel();

        for _ in 0..9 {
            let b_clone = barrier.clone();
            let s_clone = store.clone();
            let tx_clone = tx.clone();

            let handle = thread::spawn(move || {
                b_clone.wait();
                let handle = s_clone.load(id, Arc::new(ComponentManager::new())).unwrap();
                let data = handle.get().unwrap();

                tx_clone
                    .send((data.id, data.field_a.clone(), data.field_b))
                    .unwrap();

                // Threads need to hang around to ensure that at least one
                // Arc to the handle exists at all times.
                b_clone.wait();
                assert_eq!(data.id, id);
            });

            threads.push(handle);
        }

        // Make sure our handle to the data is dropped before telling the
        // other threads to exit.
        {
            barrier.wait();
            let handle = store.load(id, Arc::new(ComponentManager::new())).unwrap();
            let data = handle.get().unwrap();

            // Get the data as seen by all threads.
            for _ in 0..9 {
                let their_data = rx.recv().unwrap();

                assert_eq!(data.id, their_data.0);
                assert_eq!(data.field_a, their_data.1);
                assert_eq!(data.field_b, their_data.2);
            }
        }

        // Tell all other threads to exit.
        // Once they all drop their handles, the underlying MockStoreData
        // should get dropped as well.
        barrier.wait();
        for thread in threads {
            thread.join().unwrap();
        }

        // Now make sure that the MockStoreData actually _did_ get dropped.
        // Since we did `backend.set_remove_on_load`, this should now load
        // None into the handle.
        let handle = store.load(id, Arc::new(ComponentManager::new())).unwrap();
        let data = handle.get();
        assert!(data.is_none());
    }

    #[test]
    fn test_multiple_single_thread_access() {
        // Create some test data to load.
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let backend = MockStoreBackend::new();
        let id = snowflake_gen.generate();
        let data = MockStoredData::new(id, "foo".to_owned(), 1);
        backend.store(id, &data).unwrap();

        let store = MockStore::new(Arc::new(backend));

        // Try to load two read handles to the same Entity at once on
        // the same thread.
        //
        // The main failure mode we want to check for here is deadlocking
        // (due to trying to grab write access to the handle for
        // initialization, say)
        let handle_1 = store.load(id, Arc::new(ComponentManager::new())).unwrap();
        let data_1 = handle_1.get().unwrap();

        let handle_2 = store.load(id, Arc::new(ComponentManager::new())).unwrap();
        let data_2 = handle_2.get().unwrap();

        assert_eq!(data_1.id, data_2.id);
        assert_eq!(data_1.field_a, data_2.field_a);
        assert_eq!(data_1.field_b, data_2.field_b);

        assert_eq!(data_1.id, data.id);
        assert_eq!(data_1.field_a, data.field_a);
        assert_eq!(data_1.field_b, data.field_b);
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
