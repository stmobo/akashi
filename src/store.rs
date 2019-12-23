use std::result;
use std::error;
use std::fmt;
use std::collections::HashMap;
use std::sync::{Arc, Weak, Mutex, MutexGuard};

use crate::snowflake::Snowflake;

type Result<T> = result::Result<T, Box<dyn error::Error>>;

type StrongLockedRef<T> = Arc<Mutex<T>>;
type WeakLockedRef<T> = Weak<Mutex<T>>;

pub struct Store<T, U>
    where U: StoreBackend<T>
{
    backend: U,
    refs: Mutex<HashMap<Snowflake, WeakLockedRef<T>>>,
}

impl<T, U> Store<T, U>
    where U: StoreBackend<T>
{
    pub fn new(backend: U) -> Store<T, U> {
        Store { backend, refs: Mutex::new(HashMap::new()) }
    }

    fn load_new_ref(&self, map: &mut MutexGuard<HashMap<Snowflake, WeakLockedRef<T>>>, id: &Snowflake) -> Result<StrongLockedRef<T>> {
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
            }
        };

        Ok(r)
    }

    pub fn store(&self, id: &Snowflake, object: &T) -> Result<()> {
        self.backend.store(id, object)
    }

    pub fn exists(&self, id: &Snowflake) -> Result<bool> {
        self.backend.exists(id)
    }
}

pub trait StoreBackend<T> {
    fn load(&self, id: &Snowflake) -> Result<T>;
    fn exists(&self, id: &Snowflake) -> Result<bool>;
    fn store(&self, id: &Snowflake, object: &T) -> Result<()>;
}

#[derive(Debug, Clone)]
pub struct NotFoundError {
    id: Snowflake
}

impl NotFoundError {
    pub fn new(id: &Snowflake) -> NotFoundError {
        NotFoundError{ id: *id }
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
