//! Akashi's Entity-Component-System architecture.

pub mod component;
pub mod component_store;
pub mod entity;

pub use component::{Component, ComponentManager};
pub use component_store::ComponentStore;
pub use entity::Entity;

pub use component::TypeNotFoundError;
pub use component_store::DowncastError;
pub use entity::ClearComponentsError;

#[cfg(test)]
mod tests {
    use super::*;

    use crate::card::Card;
    use crate::local_storage::LocalComponentStorage;
    use crate::snowflake::SnowflakeGenerator;

    use std::any;
    use std::fmt;

    use failure::{Error, Fail};
    use std::sync::Arc;

    #[derive(PartialEq, Debug, Clone)]
    struct TestComponentA(u64);
    impl Component<Card> for TestComponentA {}

    #[derive(PartialEq, Debug, Clone)]
    struct TestComponentB(u64);
    impl Component<Card> for TestComponentB {}

    #[derive(PartialEq, Debug, Clone)]
    struct TestComponentC(f64);
    impl Component<Card> for TestComponentC {}

    fn new_store<T: Component<Card> + Clone + 'static>() -> LocalComponentStorage<Card, T> {
        LocalComponentStorage::new()
    }

    #[test]
    fn test_build_component_manager() {
        // Check to make sure this doesn't panic or anything.
        let mut cm = ComponentManager::new();
        cm.register_component("TestComponentA", new_store::<TestComponentA>());
        cm.register_component("TestComponentB", new_store::<TestComponentB>());
        cm.register_component("TestComponentC", new_store::<TestComponentC>());
    }

    fn expect_err<E, T>(res: Result<T, Error>)
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
        cm.register_component("TestComponentA", new_store::<TestComponentA>());

        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let mut card = Card::generate(&mut snowflake_gen, Arc::new(cm));

        // Check to make sure attempts to use unregistered component types are gracefully handled.
        expect_err::<TypeNotFoundError, Option<TestComponentB>>(card.get_component());
        expect_err::<TypeNotFoundError, ()>(
            card.set_component::<TestComponentB>(TestComponentB(5)),
        );
        assert!(!card.has_component::<TestComponentB>());
        expect_err::<TypeNotFoundError, ()>(card.delete_component::<TestComponentB>());
    }

    #[test]
    fn test_load_store_components() {
        let mut cm = ComponentManager::new();
        cm.register_component("TestComponentA", new_store::<TestComponentA>());
        cm.register_component("TestComponentB", new_store::<TestComponentB>());

        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let mut card = Card::generate(&mut snowflake_gen, Arc::new(cm));

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
        cm.register_component("TestComponentA", new_store::<TestComponentA>());

        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let mut card = Card::generate(&mut snowflake_gen, Arc::new(cm));

        // Component hasn't been added yet.
        assert!(!card.has_component::<TestComponentA>());

        // Now add it.
        card.set_component(TestComponentA(5)).unwrap();

        // Now it should exist.
        assert!(card.has_component::<TestComponentA>());
    }

    #[test]
    fn test_delete_components() {
        let mut cm = ComponentManager::new();
        cm.register_component("TestComponentA", new_store::<TestComponentA>());

        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let mut card = Card::generate(&mut snowflake_gen, Arc::new(cm));

        // Deletion of nonexistent Components shouldn't fail.
        assert!(!card.has_component::<TestComponentA>());
        assert!(card.delete_component::<TestComponentA>().is_ok());

        // Add a new component.
        card.set_component(TestComponentA(5)).unwrap();
        assert!(card.has_component::<TestComponentA>());

        // Now delete it.
        card.delete_component::<TestComponentA>().unwrap();
        assert!(!card.has_component::<TestComponentA>());
    }
}
