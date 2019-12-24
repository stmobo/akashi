use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::card::Inventory;
use crate::metadata::MetadataAttached;
use crate::resources::{ResourceCount, ResourceID};
use crate::snowflake::{Snowflake, SnowflakeGenerator};

#[derive(Clone, Serialize, Deserialize)]
pub struct Player {
    id: Snowflake,
    resources: HashMap<ResourceID, u64>,
    inventory: Inventory,
    locked_cards: Inventory,
}

impl Player {
    pub fn new(
        id: Snowflake,
        resources: HashMap<ResourceID, u64>,
        inventory: Inventory,
        locked_cards: Inventory,
    ) -> Player {
        Player {
            id,
            resources,
            inventory,
            locked_cards,
        }
    }

    pub fn empty(snowflake_gen: &mut SnowflakeGenerator) -> Player {
        Player {
            id: snowflake_gen.generate(),
            resources: HashMap::new(),
            inventory: Inventory::empty(snowflake_gen.generate()),
            locked_cards: Inventory::empty(snowflake_gen.generate()),
        }
    }

    pub fn id(&self) -> &Snowflake {
        &self.id
    }

    pub fn inventory(&self) -> &Inventory {
        &self.inventory
    }

    pub fn inventory_mut(&mut self) -> &mut Inventory {
        &mut self.inventory
    }

    pub fn locked_inventory(&self) -> &Inventory {
        &self.locked_cards
    }

    pub fn locked_inventory_mut(&mut self) -> &mut Inventory {
        &mut self.locked_cards
    }

    pub fn resources(&self) -> &HashMap<ResourceID, ResourceCount> {
        &self.resources
    }

    pub fn resources_mut(&mut self) -> &mut HashMap<ResourceID, ResourceCount> {
        &mut self.resources
    }

    pub fn get_resource(&self, id: ResourceID) -> Option<ResourceCount> {
        match self.resources.get(&id) {
            None => None,
            Some(val) => Some(*val),
        }
    }

    pub fn set_resource(&mut self, id: ResourceID, count: ResourceCount) -> Option<ResourceCount> {
        self.resources.insert(id, count)
    }
}

impl MetadataAttached for Player {}
