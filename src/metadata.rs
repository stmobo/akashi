use std::error;
use std::result;

type Result<T> = result::Result<T, Box<dyn error::Error>>;

pub trait MetadataAttached
where
    Self: Sized,
{
    fn get_metadata<'a, 'b, U>(&'b self, provider: &'a U) -> Result<U::Item>
    where
        U: MetadataProvider<'a, 'b, Self>,
    {
        provider.get(&self)
    }

    fn set_metadata<'a, 'b, U>(&'b self, provider: &'a mut U, data: U::Item) -> Result<()>
    where
        U: MutableMetadataProvider<'a, 'b, Self>,
    {
        provider.set(&self, data)
    }

    fn clear_metadata<'a, 'b, U>(&'b self, provider: &'a mut U) -> Result<()>
    where
        U: MutableMetadataProvider<'a, 'b, Self>,
    {
        provider.clear(&self)
    }
}

pub trait MetadataProvider<'a, 'b, T> {
    type Item;

    fn get(&'a self, attached_to: &'b T) -> Result<Self::Item>;
}

pub trait MutableMetadataProvider<'a, 'b, T>: MetadataProvider<'a, 'b, T> {
    fn set(&'a mut self, attached_to: &'b T, data: Self::Item) -> Result<()>;
    fn clear(&'a mut self, attached_to: &'b T) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashMap;

    use crate::snowflake::{Snowflake, SnowflakeGenerator};
    use crate::store::NotFoundError;

    struct TestAttachedType {
        id: Snowflake,
    }

    impl TestAttachedType {
        fn new(id: Snowflake) -> TestAttachedType {
            TestAttachedType { id }
        }

        fn id(&self) -> &Snowflake {
            &self.id
        }
    }

    impl MetadataAttached for TestAttachedType {}

    #[derive(Clone)]
    struct TestMetadata {
        title: String,
        value: u64,
    }

    impl TestMetadata {
        fn new(title: String, value: u64) -> TestMetadata {
            TestMetadata { title, value }
        }
    }

    struct TestMetadataStorage {
        data: HashMap<Snowflake, TestMetadata>,
    }

    impl TestMetadataStorage {
        pub fn new() -> TestMetadataStorage {
            TestMetadataStorage {
                data: HashMap::new(),
            }
        }

        pub fn add(&mut self, id: &Snowflake, data: TestMetadata) {
            self.data.insert(*id, data);
        }
    }

    impl<'a, 'b> MetadataProvider<'a, 'b, TestAttachedType> for TestMetadataStorage {
        type Item = &'a TestMetadata;

        fn get(&'a self, att: &TestAttachedType) -> Result<Self::Item> {
            match self.data.get(att.id()) {
                None => Err(Box::new(NotFoundError::new(*att.id()))),
                Some(data) => Ok(data),
            }
        }
    }

    impl<'a, 'b> MutableMetadataProvider<'a, 'b, TestAttachedType> for TestMetadataStorage {
        fn set(&'a mut self, att: &TestAttachedType, data: Self::Item) -> Result<()> {
            self.data.insert(*att.id(), data.clone());
            Ok(())
        }

        fn clear(&'a mut self, att: &TestAttachedType) -> Result<()> {
            self.data.remove(att.id());
            Ok(())
        }
    }

    #[test]
    fn test_get_metadata() {
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let id = snowflake_gen.generate();

        let mut storage = TestMetadataStorage::new();
        storage.add(&id, TestMetadata::new("foo".to_owned(), 1));

        let att = TestAttachedType::new(id);
        let md = att.get_metadata(&storage).unwrap();

        assert_eq!(md.title.as_str(), "foo");
        assert_eq!(md.value, 1);
    }

    #[test]
    fn test_set_metadata() {
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let id = snowflake_gen.generate();

        let mut storage = TestMetadataStorage::new();
        storage.add(&id, TestMetadata::new("foo".to_owned(), 1));

        let att = TestAttachedType::new(id);
        let new_metadata = TestMetadata::new("bar".to_owned(), 2);
        att.set_metadata(&mut storage, &new_metadata).unwrap();

        let md = att.get_metadata(&storage).unwrap();
        assert_eq!(md.title.as_str(), "bar");
        assert_eq!(md.value, 2);
    }

    #[test]
    fn test_clear_metadata() {
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let id = snowflake_gen.generate();

        let mut storage = TestMetadataStorage::new();
        storage.add(&id, TestMetadata::new("foo".to_owned(), 1));

        let att = TestAttachedType::new(id);
        att.clear_metadata(&mut storage).unwrap();
        assert!(att.get_metadata(&storage).is_err());
    }
}
