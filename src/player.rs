use std::sync::Arc;
use std::ops::Deref;

use crate::snowflake::{Snowflake, SnowflakeGenerator};
use crate::component::ComponentManager;

#[derive(Debug, Clone)]
pub struct Player {
    id: Snowflake,
    component_manager: Arc<ComponentManager>,
}

impl Player {
    pub fn new(id: Snowflake, component_manager: Arc<ComponentManager>) -> Player {
        Player { id, component_manager }
    }

    pub fn empty(snowflake_gen: &mut SnowflakeGenerator, component_manager: Arc<ComponentManager>) -> Player {
        Player {
            id: snowflake_gen.generate(),
            component_manager,
        }
    }

    pub fn id(&self) -> Snowflake {
        self.id
    }

    pub fn component_manager(&self) -> &ComponentManager {
        self.component_manager.deref()
    }
}

impl PartialEq for Player {
    fn eq(&self, other: &Self) -> bool {
        (self.id == other.id) && Arc::ptr_eq(&self.component_manager, &other.component_manager)
    }
}

