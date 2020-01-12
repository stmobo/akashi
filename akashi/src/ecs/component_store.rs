//! A trait for defining [`Component`] storage backends.

use super::component::Component;
use super::entity::Entity;
use crate::util::Result;

use std::any;
use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

use failure::Fail;

/// This trait is used to mark backing storage objects for [`Components`](Component).
///
/// Structs that implement this trait can be passed to
/// [`ComponentManager::register_component`](super::ComponentManager::register_component)
/// to allow Entities to load and store Component data.
pub trait ComponentBackend<T, U>
where
    T: Entity + 'static,
    U: Component<T> + 'static,
{
    /// Loads an instance of a [`Component`] from storage.
    fn load(&self, entity: &T) -> Result<Option<U>>;

    /// Saves an instance of a [`Component`] to storage.
    fn store(&self, entity: &T, component: U) -> Result<()>;

    /// Check to see if there is any stored [`Component`] data associated
    /// with an `Entity`.
    fn exists(&self, entity: &T) -> Result<bool>;

    /// Delete the stored [`Component`] data associated with the given
    /// `Entity`, if any.
    fn delete(&self, entity: &T) -> Result<()>;
}

type ComponentLoadFn<T> =
    Box<dyn Fn(&T) -> Result<Option<Box<dyn Component<T> + 'static>>> + Sync + Send>;

type ComponentBackendFn<T> =
    Box<dyn Fn(&T, Box<dyn Component<T> + 'static>) -> Result<()> + Sync + Send>;

type ComponentExistsFn<T> = Box<dyn Fn(&T) -> Result<bool> + Sync + Send>;

type ComponentDeleteFn<T> = Box<dyn Fn(&T) -> Result<()> + Sync + Send>;

/// Used internally by [`ComponentManager`](super::ComponentManager) as a
/// proxy to [`ComponentBackend`] trait methods.
pub struct ComponentTypeData<T: Entity + 'static> {
    pub load: ComponentLoadFn<T>,
    pub store: ComponentBackendFn<T>,
    pub exists: ComponentExistsFn<T>,
    pub delete: ComponentDeleteFn<T>,
}

impl<T> fmt::Debug for ComponentTypeData<T>
where
    T: Entity + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // TODO: is there a better way to format this?
        write!(f, "<ComponentTypeData>")
    }
}

impl<T> ComponentTypeData<T>
where
    T: Entity + 'static,
{
    pub fn new<U, V>(store: V) -> ComponentTypeData<T>
    where
        U: Component<T> + 'static,
        V: ComponentBackend<T, U> + Sync + Send + 'static,
    {
        let s1 = Arc::new(store);
        let s2 = s1.clone();
        let s3 = s1.clone();
        let s4 = s1.clone();

        ComponentTypeData {
            load: Box::new(move |ent: &T| {
                let res = s1.load(ent)?;
                if let Some(val) = res {
                    Ok(Some(Box::new(val)))
                } else {
                    Ok(None)
                }
            }),
            store: Box::new(
                move |ent: &T, c: Box<dyn Component<T> + 'static>| -> Result<()> {
                    let res = c.downcast::<U>();
                    if let Ok(val) = res {
                        s2.store(ent, *val)
                    } else {
                        Err(DowncastError {
                            component_name: any::type_name::<U>(),
                        }
                        .into())
                    }
                },
            ),
            exists: Box::new(move |ent: &T| s3.exists(ent)),
            delete: Box::new(move |ent: &T| s4.delete(ent)),
        }
    }
}

#[derive(Fail, Debug)]
#[fail(display = "Failed to downcast to type {}", component_name)]
pub struct DowncastError {
    component_name: &'static str,
}

pub struct ComponentAdapter<E, F, T, W>
where
    E: Entity + 'static,
    F: Into<T> + Component<E> + 'static,
    T: Into<F> + Component<E> + 'static,
    W: ComponentBackend<E, F> + Sync + Send,
{
    wrapped: W,
    ent_type: PhantomData<*const E>,
    from_type: PhantomData<*const F>,
    to_type: PhantomData<*const T>,
}

// Safety: Sync is automatically un-derived due to all the phantom pointer data.
// but since we don't actually store those types, ComponentAdapter should be
// Sync so long as type W is Sync.
unsafe impl<E, F, T, W> Sync for ComponentAdapter<E, F, T, W>
where
    E: Entity + 'static,
    F: Into<T> + Component<E> + 'static,
    T: Into<F> + Component<E> + 'static,
    W: ComponentBackend<E, F> + Sync + Send,
{
}

// Safety: ditto above-- the only reason this isn't Send is because of the
// phantom pointer data.
unsafe impl<E, F, T, W> Send for ComponentAdapter<E, F, T, W>
where
    E: Entity + 'static,
    F: Into<T> + Component<E> + 'static,
    T: Into<F> + Component<E> + 'static,
    W: ComponentBackend<E, F> + Sync + Send,
{
}

impl<E, F, T, W> ComponentAdapter<E, F, T, W>
where
    E: Entity + 'static,
    F: Into<T> + Component<E> + 'static,
    T: Into<F> + Component<E> + 'static,
    W: ComponentBackend<E, F> + Sync + Send,
{
    pub fn new(wrapped: W) -> ComponentAdapter<E, F, T, W> {
        ComponentAdapter {
            wrapped,
            ent_type: PhantomData,
            from_type: PhantomData,
            to_type: PhantomData,
        }
    }
}

impl<E, F, T, W> ComponentBackend<E, T> for ComponentAdapter<E, F, T, W>
where
    E: Entity + 'static,
    F: Into<T> + Component<E> + 'static,
    T: Into<F> + Component<E> + 'static,
    W: ComponentBackend<E, F> + Sync + Send,
{
    fn load(&self, entity: &E) -> Result<Option<T>> {
        Ok(self
            .wrapped
            .load(entity)?
            .map(|other_type: F| other_type.into()))
    }

    fn store(&self, entity: &E, component: T) -> Result<()> {
        let other_type: F = component.into();
        self.wrapped.store(entity, other_type)
    }

    fn exists(&self, entity: &E) -> Result<bool> {
        self.wrapped.exists(entity)
    }

    fn delete(&self, entity: &E) -> Result<()> {
        self.wrapped.delete(entity)
    }
}
