//! A representation of an in-game card.

use std::any::TypeId;
use std::collections::HashSet;
use std::sync::Arc;

use crate::ecs::{ComponentManager, Entity};
use crate::snowflake::{Snowflake, SnowflakeGenerator};

/// Represents a tradable card.
///
/// Strictly speaking, this is just a bare-bones [`Entity`].  
#[derive(Clone, Debug)]
pub struct Card {
    id: Snowflake,
    component_manager: Arc<ComponentManager<Card>>,
    components_attached: HashSet<TypeId>,
    dirty: bool,
}

impl Card {
    /// Create a new `Card` instance.
    pub fn new(
        id: Snowflake,
        component_manager: Arc<ComponentManager<Card>>,
        components_attached: HashSet<TypeId>,
    ) -> Card {
        Card {
            id,
            component_manager,
            components_attached,
            dirty: false,
        }
    }

    /// Create an 'empty' `Card` instance with a random ID.
    pub fn generate(
        snowflake_gen: &mut SnowflakeGenerator,
        component_manager: Arc<ComponentManager<Card>>,
    ) -> Card {
        Card {
            id: snowflake_gen.generate(),
            component_manager,
            components_attached: HashSet::new(),
            dirty: false,
        }
    }

    /// Get this `Card`'s unique ID.
    pub fn id(&self) -> Snowflake {
        self.id
    }

    /// Get a reference to this `Card`'s associated [`ComponentManager`].
    pub fn component_manager(&self) -> &ComponentManager<Card> {
        &self.component_manager
    }
}

impl PartialEq for Card {
    fn eq(&self, other: &Self) -> bool {
        (self.id == other.id) && Arc::ptr_eq(&self.component_manager, &other.component_manager)
    }
}

impl Entity for Card {
    fn new(id: Snowflake, cm: Arc<ComponentManager<Card>>, components: HashSet<TypeId>) -> Card {
        Card::new(id, cm, components)
    }

    fn id(&self) -> Snowflake {
        self.id()
    }

    fn component_manager(&self) -> &ComponentManager<Card> {
        &self.component_manager
    }

    fn components_attached(&self) -> &HashSet<TypeId> {
        &self.components_attached
    }

    fn components_attached_mut(&mut self) -> &mut HashSet<TypeId> {
        &mut self.components_attached
    }

    fn dirty(&self) -> bool {
        self.dirty
    }

    fn dirty_mut(&mut self) -> &mut bool {
        &mut self.dirty
    }
}

mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[allow(unused_imports)]
    use crate::snowflake::SnowflakeGenerator;

    #[test]
    fn test_card_generate() {
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let cm = Arc::new(ComponentManager::new());

        let card1 = Card::generate(&mut snowflake_gen, cm.clone());
        let card2 = Card::generate(&mut snowflake_gen, cm);

        assert_ne!(card1.id(), card2.id());
    }
}
