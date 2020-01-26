//! A representation of a game player.

use std::any::TypeId;
use std::collections::HashSet;
use std::sync::Arc;

use dashmap::DashMap;

use crate::ecs::{Component, ComponentManager, Entity};
use crate::snowflake::{Snowflake, SnowflakeGenerator};

/// Represents a player / user.
///
/// Strictly speaking, this is just a minimal [`Entity`] object.
pub struct Player {
    id: Snowflake,
    component_manager: Arc<ComponentManager<Player>>,
    components_attached: HashSet<TypeId>,
    component_preloads: DashMap<TypeId, Box<dyn Component<Self> + Send + Sync + 'static>>,
    dirty: bool,
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
            component_preloads: DashMap::new(),
            dirty: false,
        }
    }

    /// Create an 'empty' `Player` instance with no attached [`Components`](crate::Component)
    /// and a randomly-generated ID.
    pub fn empty(
        snowflake_gen: &mut SnowflakeGenerator,
        component_manager: Arc<ComponentManager<Player>>,
    ) -> Player {
        Player {
            id: snowflake_gen.generate(),
            component_manager,
            components_attached: HashSet::new(),
            component_preloads: DashMap::new(),
            dirty: false,
        }
    }

    /// Get this `Player`'s unique ID.
    pub fn id(&self) -> Snowflake {
        self.id
    }

    /// Get a reference to this `Player`'s associated [`ComponentManager`].
    pub fn component_manager(&self) -> &ComponentManager<Player> {
        &self.component_manager
    }
}

impl Clone for Player {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            dirty: self.dirty,
            component_manager: self.component_manager.clone(),
            components_attached: self.components_attached.clone(),
            component_preloads: DashMap::new(),
        }
    }
}

impl PartialEq for Player {
    fn eq(&self, other: &Self) -> bool {
        (self.id == other.id) && Arc::ptr_eq(&self.component_manager, &other.component_manager)
    }
}

impl Entity for Player {
    fn new(id: Snowflake, cm: Arc<ComponentManager<Self>>, components: HashSet<TypeId>) -> Self {
        Player::new(id, cm, components)
    }

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

    fn preloaded_components(
        &self,
    ) -> &DashMap<TypeId, Box<dyn Component<Self> + Send + Sync + 'static>> {
        &self.component_preloads
    }

    fn dirty(&self) -> bool {
        self.dirty
    }

    fn dirty_mut(&mut self) -> &mut bool {
        &mut self.dirty
    }
}
