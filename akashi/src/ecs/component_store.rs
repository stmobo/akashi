use super::component::Component;
use super::entity::Entity;
use crate::util::Result;

use std::any;
use std::fmt;
use std::sync::Arc;

use failure::Fail;

pub trait ComponentStore<T, U>
where
    T: Entity + 'static,
    U: Component<T> + 'static,
{
    fn load(&self, entity: &T) -> Result<Option<U>>;
    fn store(&self, entity: &T, component: U) -> Result<()>;
    fn exists(&self, entity: &T) -> Result<bool>;
    fn delete(&self, entity: &T) -> Result<()>;
}

type ComponentLoadFn<T> =
    Box<dyn Fn(&T) -> Result<Option<Box<dyn Component<T> + 'static>>> + Sync + Send>;

type ComponentStoreFn<T> =
    Box<dyn Fn(&T, Box<dyn Component<T> + 'static>) -> Result<()> + Sync + Send>;

type ComponentExistsFn<T> = Box<dyn Fn(&T) -> Result<bool> + Sync + Send>;

type ComponentDeleteFn<T> = Box<dyn Fn(&T) -> Result<()> + Sync + Send>;

pub struct ComponentTypeData<T: Entity + 'static> {
    pub load: ComponentLoadFn<T>,
    pub store: ComponentStoreFn<T>,
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
        V: ComponentStore<T, U> + Sync + Send + 'static,
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
