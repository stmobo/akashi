use std::collections::HashMap;

use crate::card::Inventory;
use crate::metadata::MetadataAttached;
use crate::resources::{ResourceCount, ResourceID};
use crate::snowflake::{Snowflake, SnowflakeGenerator};

#[derive(Clone)]
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

    pub fn id(&self) -> Snowflake {
        self.id
    }

    pub fn inventory(&mut self) -> &mut Inventory {
        &mut self.inventory
    }

    pub fn locked_cards(&mut self) -> &mut Inventory {
        &mut self.locked_cards
    }

    pub fn get_resource(&self, id: &ResourceID) -> Option<u64> {
        match self.resources.get(id) {
            None => None,
            Some(val) => Some(*val),
        }
    }

    pub fn set_resource(&mut self, id: &ResourceID, count: &ResourceCount) {
        match self.resources.get_mut(id) {
            None => panic!("Invalid resource ID"),
            Some(r) => *r = *count,
        }
    }
}

impl MetadataAttached for Player {}
