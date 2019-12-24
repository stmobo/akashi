pub type ResourceID = u64;
pub type ResourceCount = u64;

pub struct ResourceType {
    id: ResourceID,
    name: String,
}

impl ResourceType {
    pub fn new(id: ResourceID, name: &str) -> ResourceType {
        ResourceType {
            id,
            name: String::from(name),
        }
    }

    pub fn id(&self) -> ResourceID {
        self.id
    }

    pub fn name(&self) -> &str {
        self.name.as_str()
    }
}

pub trait ResourceDataStore {
    fn get(&self, id: ResourceID) -> Option<&ResourceType>;
}

pub struct LocalResourceStore {
    types: Vec<ResourceType>,
}

impl ResourceDataStore for LocalResourceStore {
    fn get(&self, id: ResourceID) -> Option<&ResourceType> {
        self.types.get(id as usize)
    }
}

impl LocalResourceStore {
    pub fn new(types: Vec<ResourceType>) -> LocalResourceStore {
        LocalResourceStore { types }
    }
}
