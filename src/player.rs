use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::metadata::MetadataAttached;
use crate::resources::{ResourceCount, ResourceID};
use crate::snowflake::{Snowflake, SnowflakeGenerator};

#[derive(Clone, Serialize, Deserialize, Eq, PartialEq, Debug)]
pub struct Player {
    id: Snowflake,
    resources: HashMap<ResourceID, u64>,
    inventories: HashMap<String, Snowflake>,
}

impl Player {
    pub fn new(
        id: Snowflake,
        resources: HashMap<ResourceID, u64>,
        inventories: HashMap<String, Snowflake>,
    ) -> Player {
        Player {
            id,
            resources,
            inventories,
        }
    }

    pub fn empty(snowflake_gen: &mut SnowflakeGenerator) -> Player {
        Player {
            id: snowflake_gen.generate(),
            resources: HashMap::new(),
            inventories: HashMap::new(),
        }
    }

    pub fn id(&self) -> &Snowflake {
        &self.id
    }

    pub fn get_inventory(&self, name: &str) -> Option<&Snowflake> {
        self.inventories.get(name)
    }

    pub fn attach_inventory(&mut self, name: &str, id: Snowflake) -> Option<Snowflake> {
        self.inventories.insert(String::from(name), id)
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
