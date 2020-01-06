use std::ops::Deref;
use std::sync::Arc;

use crate::ecs::{Component, ComponentManager, Entity};
use crate::snowflake::{Snowflake, SnowflakeGenerator};
use crate::util::Result;

#[derive(Debug, Clone)]
pub struct Player {
    id: Snowflake,
    component_manager: Arc<ComponentManager<Player>>,
}

impl Player {
    pub fn new(id: Snowflake, component_manager: Arc<ComponentManager<Player>>) -> Player {
        Player {
            id,
            component_manager,
        }
    }

    pub fn empty(
        snowflake_gen: &mut SnowflakeGenerator,
        component_manager: Arc<ComponentManager<Player>>,
    ) -> Player {
        Player {
            id: snowflake_gen.generate(),
            component_manager,
        }
    }

    pub fn id(&self) -> Snowflake {
        self.id
    }

    pub fn component_manager(&self) -> &ComponentManager<Player> {
        self.component_manager.deref()
    }
}

impl PartialEq for Player {
    fn eq(&self, other: &Self) -> bool {
        (self.id == other.id) && Arc::ptr_eq(&self.component_manager, &other.component_manager)
    }
}

impl Entity for Player {
    fn id(&self) -> Snowflake {
        self.id()
    }

    fn component_manager(&self) -> &ComponentManager<Player> {
        self.component_manager()
    }

    fn get_component<T: Component<Player> + 'static>(&self) -> Result<Option<T>> {
        let cm = self.component_manager();
        cm.get_component::<T>(&self)
    }

    fn set_component<T: Component<Player> + 'static>(&mut self, component: T) -> Result<()> {
        let cm = self.component_manager();
        cm.set_component::<T>(&self, component)
    }

    fn has_component<T: Component<Player> + 'static>(&self) -> Result<bool> {
        let cm = self.component_manager();
        cm.component_exists::<T>(&self)
    }

    fn delete_component<T: Component<Player> + 'static>(&mut self) -> Result<()> {
        let cm = self.component_manager();
        cm.delete_component::<T>(&self)
    }
}
