use std::any;
use std::any::TypeId;
use std::collections::HashMap;
use std::fmt;
use std::result;
use std::sync::Arc;

extern crate downcast_rs;
use downcast_rs::Downcast;

use failure::{Fail, Error};

use crate::snowflake::Snowflake;

pub trait Component: Downcast + Sync + Send {}
downcast_rs::impl_downcast!(Component);

pub type Result<T> = result::Result<T, Error>;

pub trait ComponentStore<T: Component + 'static> {
    fn load(&self, entity_id: Snowflake) -> Result<Option<T>>;
    fn store(&self, entity_id: Snowflake, component: T) -> Result<()>;
    fn exists(&self, entity_id: Snowflake) -> Result<bool>;
    fn delete(&self, entity_id: Snowflake) -> Result<()>;
}

type ComponentLoadFn =
    Box<dyn Fn(Snowflake) -> Result<Option<Box<dyn Component + 'static>>> + Sync + Send>;
type ComponentStoreFn =
    Box<dyn Fn(Snowflake, Box<dyn Component + 'static>) -> Result<()> + Sync + Send>;
type ComponentExistsFn = Box<dyn Fn(Snowflake) -> Result<bool> + Sync + Send>;
type ComponentDeleteFn = Box<dyn Fn(Snowflake) -> Result<()> + Sync + Send>;

pub struct ComponentTypeData {
    load: ComponentLoadFn,
    store: ComponentStoreFn,
    exists: ComponentExistsFn,
    delete: ComponentDeleteFn,
}

impl fmt::Debug for ComponentTypeData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // TODO: is there a better way to format this?
        write!(f, "<ComponentTypeData>")
    }
}

impl ComponentTypeData {
    pub fn new<T, U>(store: U) -> ComponentTypeData
    where
        T: Component + 'static,
        U: ComponentStore<T> + Sync + Send + 'static,
    {
        let s1 = Arc::new(store);
        let s2 = s1.clone();
        let s3 = s1.clone();
        let s4 = s1.clone();

        ComponentTypeData {
            load: Box::new(move |ent_id: Snowflake| {
                let res = s1.load(ent_id)?;
                if let Some(val) = res {
                    Ok(Some(Box::new(val)))
                } else {
                    Ok(None)
                }
            }),
            store: Box::new(
                move |ent_id: Snowflake, c: Box<dyn Component + 'static>| -> Result<()> {
                    let res = c.downcast::<T>();
                    if let Ok(val) = res {
                        s2.store(ent_id, *val)
                    } else {
                        Err(DowncastError {
                            component_name: any::type_name::<T>(),
                        }.into())
                    }
                },
            ),
            exists: Box::new(move |ent_id: Snowflake| s3.exists(ent_id)),
            delete: Box::new(move |ent_id: Snowflake| s4.delete(ent_id)),
        }
    }
}

#[derive(Debug)]
pub struct ComponentManager {
    component_types: HashMap<TypeId, ComponentTypeData>,
}

impl ComponentManager {
    pub fn new() -> ComponentManager {
        ComponentManager {
            component_types: HashMap::new(),
        }
    }

    pub fn register_component<T, U>(&mut self, store: U)
    where
        T: Component + 'static,
        U: ComponentStore<T> + Sync + Send + 'static,
    {
        self.component_types
            .insert(TypeId::of::<T>(), ComponentTypeData::new(store));
    }

    fn set_component<T: Component + 'static>(
        &self,
        entity_id: Snowflake,
        component: T,
    ) -> Result<()> {
        if let Some(data) = self.component_types.get(&TypeId::of::<T>()) {
            (data.store)(entity_id, Box::new(component))
        } else {
            Err(TypeNotFoundError {
                component_name: any::type_name::<T>().to_owned(),
            }.into())
        }
    }

    fn get_component<T: Component + 'static>(&self, entity_id: Snowflake) -> Result<Option<T>> {
        if let Some(data) = self.component_types.get(&TypeId::of::<T>()) {
            if let Some(comp) = (data.load)(entity_id)? {
                // if this downcast fails, the loader was written wrong
                let boxed = match comp.downcast::<T>() {
                    Ok(v) => v,
                    Err(_e) => panic!("Failed to downcast component from loader"),
                };
                Ok(Some(*boxed))
            } else {
                Ok(None)
            }
        } else {
            Err(TypeNotFoundError {
                component_name: any::type_name::<T>().to_owned(),
            }.into())
        }
    }

    fn delete_component<T: Component + 'static>(&self, entity_id: Snowflake) -> Result<()> {
        if let Some(data) = self.component_types.get(&TypeId::of::<T>()) {
            (data.delete)(entity_id)
        } else {
            Err(TypeNotFoundError {
                component_name: any::type_name::<T>().to_owned(),
            }.into())
        }
    }

    fn component_exists<T: Component + 'static>(&self, entity_id: Snowflake) -> Result<bool> {
        if let Some(data) = self.component_types.get(&TypeId::of::<T>()) {
            (data.exists)(entity_id)
        } else {
            Err(TypeNotFoundError {
                component_name: any::type_name::<T>().to_owned(),
            }.into())
        }
    }
}

#[derive(Fail, Debug)]
#[fail(
    display = "No handlers registered for Components of type {}",
    component_name
)]
pub struct TypeNotFoundError {
    component_name: String,
}

#[derive(Fail, Debug)]
#[fail(display = "Failed to downcast to type {}", component_name)]
pub struct DowncastError {
    component_name: &'static str,
}

pub trait ComponentsAttached {
    fn id(&self) -> Snowflake;
    fn component_manager(&self) -> &ComponentManager;

    fn get_component<T: Component + 'static>(&self) -> Result<Option<T>> {
        let cm = self.component_manager();
        cm.get_component::<T>(self.id())
    }

    fn set_component<T: Component + 'static>(&self, component: T) -> Result<()> {
        let cm = self.component_manager();
        cm.set_component::<T>(self.id(), component)
    }

    fn has_component<T: Component + 'static>(&self) -> Result<bool> {
        let cm = self.component_manager();
        cm.component_exists::<T>(self.id())
    }

    fn delete_component<T: Component + 'static>(&self) -> Result<()> {
        let cm = self.component_manager();
        cm.delete_component::<T>(self.id())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::any;

    use crate::card::Card;
    use crate::snowflake::SnowflakeGenerator;

    use crate::local_storage::LocalComponentStorage;

    #[derive(PartialEq, Debug, Clone)]
    struct TestComponentA(u64);
    impl Component for TestComponentA {}

    #[derive(PartialEq, Debug, Clone)]
    struct TestComponentB(u64);
    impl Component for TestComponentB {}

    #[derive(PartialEq, Debug, Clone)]
    struct TestComponentC(f64);
    impl Component for TestComponentC {}

    fn new_store<T: Component + Clone + 'static>() -> LocalComponentStorage<T> {
        LocalComponentStorage::new()
    }

    #[test]
    fn test_build_component_manager() {
        // Check to make sure this doesn't panic or anything.
        let mut cm = ComponentManager::new();
        cm.register_component(new_store::<TestComponentA>());
        cm.register_component(new_store::<TestComponentB>());
        cm.register_component(new_store::<TestComponentC>());
    }

    fn expect_err<E, T>(res: Result<T>)
    where
        E: Fail + Send + Sync,
        T: fmt::Debug,
    {
        match res {
            Ok(v) => panic!("expected failure, got Ok value: {:?}", v),
            Err(e) => e.downcast_ref::<E>().expect(
                format!("Could not downcast error to {:?}", any::type_name::<E>()).as_str(),
            ),
        };
    }

    #[test]
    fn test_unregistered_type() {
        let mut cm = ComponentManager::new();
        cm.register_component(new_store::<TestComponentA>());

        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let card = Card::generate(&mut snowflake_gen, Arc::new(cm));

        // Check to make sure attempts to use unregistered component types are gracefully handled.
        expect_err::<TypeNotFoundError, Option<TestComponentB>>(card.get_component());
        expect_err::<TypeNotFoundError, ()>(
            card.set_component::<TestComponentB>(TestComponentB(5)),
        );
        expect_err::<TypeNotFoundError, bool>(card.has_component::<TestComponentB>());
        expect_err::<TypeNotFoundError, ()>(card.delete_component::<TestComponentB>());
    }

    #[test]
    fn test_load_store_components() {
        let mut cm = ComponentManager::new();
        cm.register_component(new_store::<TestComponentA>());
        cm.register_component(new_store::<TestComponentB>());

        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let card = Card::generate(&mut snowflake_gen, Arc::new(cm));

        let component_a: Option<TestComponentA> = card.get_component().unwrap();
        let component_b: Option<TestComponentB> = card.get_component().unwrap();

        // Attempting to get an unset Component should return Ok(None).
        assert!(component_a.is_none());
        assert!(component_b.is_none());

        // Now add the Components.
        card.set_component(TestComponentA(5)).unwrap();
        card.set_component(TestComponentB(13)).unwrap();

        let component_a: Option<TestComponentA> = card.get_component().unwrap();
        let component_b: Option<TestComponentB> = card.get_component().unwrap();

        // Check to make sure the values we put in are the same ones we get back out...
        assert_eq!(component_a.unwrap().0, 5);
        assert_eq!(component_b.unwrap().0, 13);
    }

    #[test]
    fn test_components_exist() {
        let mut cm = ComponentManager::new();
        cm.register_component(new_store::<TestComponentA>());

        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let card = Card::generate(&mut snowflake_gen, Arc::new(cm));

        // Component hasn't been added yet.
        assert!(!card.has_component::<TestComponentA>().unwrap());

        // Now add it.
        card.set_component(TestComponentA(5)).unwrap();

        // Now it should exist.
        assert!(card.has_component::<TestComponentA>().unwrap());
    }

    #[test]
    fn test_delete_components() {
        let mut cm = ComponentManager::new();
        cm.register_component(new_store::<TestComponentA>());

        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let card = Card::generate(&mut snowflake_gen, Arc::new(cm));

        // Deletion of nonexistent Components shouldn't fail.
        assert!(!card.has_component::<TestComponentA>().unwrap());
        assert!(card.delete_component::<TestComponentA>().is_ok());

        // Add a new component.
        card.set_component(TestComponentA(5)).unwrap();
        assert!(card.has_component::<TestComponentA>().unwrap());

        // Now delete it.
        card.delete_component::<TestComponentA>().unwrap();
        assert!(!card.has_component::<TestComponentA>().unwrap());
    }
}
