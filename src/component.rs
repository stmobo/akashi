use std::any;
use std::any::TypeId;
use std::collections::HashMap;
use std::fmt;
use std::result;
use std::sync::Arc;

extern crate downcast_rs;
use downcast_rs::Downcast;

use failure::Fail;

use crate::snowflake::Snowflake;

pub trait Component: Downcast + Sync + Send {}
downcast_rs::impl_downcast!(Component);

pub type Result<T> = result::Result<T, Box<dyn Fail>>;

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
                        Err(Box::new(DowncastError {
                            component_name: any::type_name::<T>(),
                        }))
                    }
                },
            ),
            exists: Box::new(move |ent_id: Snowflake| s3.exists(ent_id)),
            delete: Box::new(move |ent_id: Snowflake| s4.delete(ent_id)),
        }
    }
}

#[derive(Debug)]
pub struct ComponentManagerBuilder {
    component_types: HashMap<TypeId, ComponentTypeData>,
}

impl ComponentManagerBuilder {
    pub fn register_component<T, U>(mut self, store: U) -> Self
    where
        T: Component + 'static,
        U: ComponentStore<T> + Sync + Send + 'static,
    {
        self.component_types
            .insert(TypeId::of::<T>(), ComponentTypeData::new(store));

        self
    }

    pub fn finish(self) -> ComponentManager {
        ComponentManager {
            component_types: self.component_types,
        }
    }
}

#[derive(Debug)]
pub struct ComponentManager {
    component_types: HashMap<TypeId, ComponentTypeData>,
}

impl ComponentManager {
    pub fn build() -> ComponentManagerBuilder {
        ComponentManagerBuilder {
            component_types: HashMap::new(),
        }
    }

    fn set_component<T: Component + 'static>(
        &self,
        entity_id: Snowflake,
        component: T,
    ) -> Result<()> {
        if let Some(data) = self.component_types.get(&TypeId::of::<T>()) {
            (data.store)(entity_id, Box::new(component))
        } else {
            Err(Box::new(TypeNotFoundError {
                component_name: any::type_name::<T>().to_owned(),
            }))
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
            Err(Box::new(TypeNotFoundError {
                component_name: any::type_name::<T>().to_owned(),
            }))
        }
    }

    fn delete_component<T: Component + 'static>(&self, entity_id: Snowflake) -> Result<()> {
        if let Some(data) = self.component_types.get(&TypeId::of::<T>()) {
            (data.delete)(entity_id)
        } else {
            Err(Box::new(TypeNotFoundError {
                component_name: any::type_name::<T>().to_owned(),
            }))
        }
    }

    fn component_exists<T: Component + 'static>(&self, entity_id: Snowflake) -> Result<bool> {
        if let Some(data) = self.component_types.get(&TypeId::of::<T>()) {
            (data.exists)(entity_id)
        } else {
            Err(Box::new(TypeNotFoundError {
                component_name: any::type_name::<T>().to_owned(),
            }))
        }
    }
}

#[derive(Fail, Debug)]
#[fail(
    display = "No handlers registered for Components of type {}",
    component_name
)]
struct TypeNotFoundError {
    component_name: String,
}

#[derive(Fail, Debug)]
#[fail(display = "Failed to downcast to type {}", component_name)]
struct DowncastError {
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
    use std::sync::Mutex;

    use crate::card::Card;
    use crate::snowflake::SnowflakeGenerator;

    #[derive(PartialEq, Debug, Clone)]
    struct TestComponentA(u64);
    impl Component for TestComponentA {}

    #[derive(PartialEq, Debug, Clone)]
    struct TestComponentB(u64);
    impl Component for TestComponentB {}

    #[derive(PartialEq, Debug, Clone)]
    struct TestComponentC(f64);
    impl Component for TestComponentC {}

    struct MockComponentStore<T: Component + Clone + 'static> {
        map: Mutex<HashMap<Snowflake, T>>,
    }

    fn new_store<T: Component + Clone + 'static>() -> MockComponentStore<T> {
        MockComponentStore {
            map: Mutex::new(HashMap::new()),
        }
    }

    impl<T: Component + Clone + 'static> ComponentStore<T> for MockComponentStore<T> {
        fn load(&self, entity_id: Snowflake) -> Result<Option<T>> {
            let map = self.map.lock().unwrap();
            Ok(map.get(&entity_id).map(|x| x.clone()))
        }

        fn store(&self, entity_id: Snowflake, component: T) -> Result<()> {
            let mut map = self.map.lock().unwrap();
            map.insert(entity_id, component);
            Ok(())
        }

        fn exists(&self, entity_id: Snowflake) -> Result<bool> {
            let map = self.map.lock().unwrap();
            Ok(map.contains_key(&entity_id))
        }

        fn delete(&self, entity_id: Snowflake) -> Result<()> {
            let mut map = self.map.lock().unwrap();
            map.remove(&entity_id);
            Ok(())
        }
    }

    #[test]
    fn test_build_component_manager() {
        // Check to make sure this doesn't panic or anything.
        let _cm = ComponentManager::build()
            .register_component(new_store::<TestComponentA>())
            .register_component(new_store::<TestComponentB>())
            .register_component(new_store::<TestComponentC>())
            .finish();
    }

    #[test]
    fn test_load_store_components() {
        let cm = ComponentManager::build()
            .register_component(new_store::<TestComponentA>())
            .register_component(new_store::<TestComponentB>())
            .finish();

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
        let cm = ComponentManager::build()
            .register_component(new_store::<TestComponentA>())
            .finish();

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
        let cm = ComponentManager::build()
            .register_component(new_store::<TestComponentA>())
            .finish();

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
