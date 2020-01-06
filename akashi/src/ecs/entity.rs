use super::component::{Component, ComponentManager};
use crate::snowflake::Snowflake;
use crate::util::Result;

pub trait Entity: Sized {
    fn id(&self) -> Snowflake;
    fn component_manager(&self) -> &ComponentManager<Self>;
    //fn component_cache(&self) -> HashMap<TypeId, Box<Component<Self>>>;

    fn get_component<T: Component<Self> + 'static>(&self) -> Result<Option<T>>;
    fn set_component<T: Component<Self> + 'static>(&mut self, component: T) -> Result<()>;
    fn has_component<T: Component<Self> + 'static>(&self) -> Result<bool>;
    fn delete_component<T: Component<Self> + 'static>(&mut self) -> Result<()>;
}
