//! A representation of a game player.

use std::any::TypeId;
use std::collections::HashSet;
use std::sync::Arc;

use crate::ecs::{ComponentManager, Entity};
use crate::snowflake::{Snowflake, SnowflakeGenerator};

/// Represents a player / user.
///
/// Strictly speaking, this is just a minimal Entity object.
#[derive(Debug, Clone)]
pub struct Player {
    id: Snowflake,
    component_manager: Arc<ComponentManager<Player>>,
    components_attached: HashSet<TypeId>,
}

impl Player {
    /// Create a new `Player` instance.
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

    /// Create an 'empty' `Player` instance with no attached `Components`
    /// and a randomly-generated ID.
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

    /// Get this `Player`'s unique ID.
    pub fn id(&self) -> Snowflake {
        self.id
    }

    /// Get a reference to this `Player`'s associated `ComponentManager`.
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
