use std::any::TypeId;
use std::collections::HashSet;
use std::sync::Arc;

use crate::ecs::{ComponentManager, Entity};
use crate::snowflake::{Snowflake, SnowflakeGenerator};

#[derive(Debug, Clone)]
pub struct Player {
    id: Snowflake,
    component_manager: Arc<ComponentManager<Player>>,
    components_attached: HashSet<TypeId>,
}

impl Player {
    pub fn new(
        id: Snowflake,
        component_manager: Arc<ComponentManager<Player>>,
        components_attached: HashSet<TypeId>,
    ) -> Player {
        Player {
            id,
            component_manager,
            components_attached,
        }
    }

    pub fn empty(
        snowflake_gen: &mut SnowflakeGenerator,
        component_manager: Arc<ComponentManager<Player>>,
    ) -> Player {
        Player {
            id: snowflake_gen.generate(),
            component_manager,
            components_attached: HashSet::new(),
        }
    }

    pub fn id(&self) -> Snowflake {
        self.id
    }

    pub fn component_manager(&self) -> &ComponentManager<Player> {
        &self.component_manager
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
        &self.component_manager
    }

    fn components_attached(&self) -> &HashSet<TypeId> {
        &self.components_attached
    }

    fn components_attached_mut(&mut self) -> &mut HashSet<TypeId> {
        &mut self.components_attached
    }
}
