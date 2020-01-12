//! Card inventories as [`Player`] components.

use crate::card::Card;
use crate::ecs::entity_store::handle_ref::*;
use crate::ecs::entity_store::{StoreHandle, StoreReference};
use crate::ecs::EntityManager;
use crate::ecs::{Component, ComponentAdapter};
use crate::player::Player;
use crate::snowflake::Snowflake;
use crate::util::Result;

use std::collections::{HashMap, HashSet};

rental! {
    pub mod inventory_rental {
        use super::*;

        #[rental_mut(deref_mut_suffix)]
        pub struct InventoryWriteRental {
            #[subrental = 2]
            prefix: Box<HandleWriteRef<StoreReference<StoreHandle<Card>>, StoreHandle<Card>>>,
            card: &'prefix_1 mut Card,
        }
    }
}

use inventory_rental::InventoryWriteRental;

/// Represents a collection of [`Card`] entities.
///
/// `Inventory` also implements `Component<Player>`, so you can attach
/// instances to [`Players`](Player) (given appropriate storage code).
pub struct Inventory {
    card_cache: HashMap<Snowflake, InventoryWriteRental>,
    ids: HashSet<Snowflake>,
}

impl Inventory {
    /// Creates a new, empty `Inventory`.
    pub fn empty() -> Inventory {
        Inventory {
            card_cache: HashMap::new(),
            ids: HashSet::new(),
        }
    }

    /// Adds a [`Card`] to this inventory.
    ///
    /// Returns `true` if a card with this ID was not previously in this
    /// inventory, and `false` otherwise.
    pub fn insert(&mut self, card: Card, entity_manager: &EntityManager) -> Result<bool> {
        let id = card.id();
        let handle = entity_manager.insert(card)?;
        let rental =
            InventoryWriteRental::new(Box::new(handle), |handle| handle.suffix.get_mut().unwrap());

        self.card_cache.insert(id, rental);
        Ok(self.ids.insert(id))
    }

    /// Checks to see if this inventory contains the given [`Card`] ID.
    pub fn contains(&self, id: Snowflake) -> bool {
        self.ids.contains(&id)
    }

    /// Removes a [`Card`] from this inventory by ID and returns it,
    /// if any.
    ///
    /// Returns whether the ID was present in the inventory to begin with.
    pub fn remove(&mut self, id: Snowflake) -> bool {
        self.card_cache.remove(&id);
        self.ids.remove(&id)
    }

    /// Checks to see if this inventory is empty.
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// Gets how many IDs are stored in this inventory.
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    /// Iterates over all IDs in this inventory.
    pub fn iter_ids<'a>(&'a self) -> impl Iterator<Item = &'a Snowflake> + '_ {
        self.ids.iter()
    }

    /// Gets a card from this inventory by ID.
    pub fn get<'a, 'b: 'a>(
        &'a mut self,
        id: Snowflake,
        entity_manager: &'b EntityManager,
    ) -> Option<&'a mut InventoryWriteRental> {
        if self.card_cache.contains_key(&id) {
            self.card_cache.get_mut(&id)
        } else {
            let handle = entity_manager.load_mut::<Card>(id).ok()?;
            if handle.exists() {
                let rental = InventoryWriteRental::new(Box::new(handle), |handle| {
                    handle.suffix.get_mut().unwrap()
                });

                self.card_cache.insert(id, rental);
                self.card_cache.get_mut(&id)
            } else {
                None
            }
        }
    }
}

impl Component<Player> for Inventory {}

impl Default for Inventory {
    fn default() -> Inventory {
        Inventory::empty()
    }
}

impl Component<Player> for Vec<Snowflake> {}

impl From<Vec<Snowflake>> for Inventory {
    fn from(ids: Vec<Snowflake>) -> Inventory {
        let mut id_set = HashSet::with_capacity(ids.len());
        for id in ids.into_iter() {
            id_set.insert(id);
        }

        Inventory {
            ids: id_set,
            card_cache: HashMap::new(),
        }
    }
}

impl From<Inventory> for Vec<Snowflake> {
    fn from(inv: Inventory) -> Vec<Snowflake> {
        inv.iter_ids().copied().collect()
    }
}

/// Acts as a [`ComponentBackend`] for [`Inventories`](Inventory) by wrapping
/// another [`ComponentBackend`].
///
/// The wrapped storage type needs to implement loading and storing lists of
/// card type IDs via the `ComponentBackend<Card, Vec<Snowflake>>` trait.
pub type InventoryBackendWrapper<W> = ComponentAdapter<Player, Vec<Snowflake>, Inventory, W>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::{Component, Entity};
    use crate::local_storage::{LocalComponentStorage, LocalEntityStorage};
    use crate::snowflake::SnowflakeGenerator;

    #[test]
    fn test_inv() {
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let mut ent_mgr = EntityManager::new();

        #[derive(Clone)]
        struct TestComponent(u64);
        impl Component<Card> for TestComponent {};

        ent_mgr
            .register_entity(LocalEntityStorage::<Card>::new())
            .unwrap();

        ent_mgr
            .register_component(
                "TestComponent",
                LocalComponentStorage::<Card, TestComponent>::new(),
            )
            .unwrap();

        let mut inv = Inventory::empty();

        assert_eq!(inv.len(), 0);
        assert!(inv.is_empty());

        let card: Card = ent_mgr.create(snowflake_gen.generate()).unwrap();
        let id = card.id();

        assert!(inv.insert(card, &ent_mgr).unwrap());
        assert!(inv.contains(id));
        assert_eq!(inv.len(), 1);

        let r = inv.get(id, &ent_mgr).unwrap();
        assert_eq!(r.id(), id);

        r.set_component(TestComponent(50)).unwrap();
        drop(r);

        assert!(inv.remove(id));
    }

    #[test]
    fn test_inv_wrapper() {
        use crate::Player;

        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let mut ent_mgr = EntityManager::new();

        ent_mgr
            .register_entity(LocalEntityStorage::<Card>::new())
            .unwrap();

        ent_mgr
            .register_entity(LocalEntityStorage::<Player>::new())
            .unwrap();

        ent_mgr
            .register_component(
                "Inventory",
                InventoryBackendWrapper::new(
                    LocalComponentStorage::<Player, Vec<Snowflake>>::new()
                )
            )
            .unwrap();

        let mut ids: Vec<Snowflake> = (0..5).map(|_| snowflake_gen.generate()).collect();
        ids.sort_unstable();

        let mut player: Player = ent_mgr.create(snowflake_gen.generate()).unwrap();
        let player_id = player.id();

        player
            .set_component::<Inventory>(ids.clone().into())
            .unwrap();
        ent_mgr.store(player).unwrap();

        let handle = ent_mgr.load::<Player>(player_id).unwrap();
        let player = handle.get().unwrap();

        let inv: Inventory = player.get_component().unwrap().unwrap();
        let mut loaded_ids: Vec<Snowflake> = inv.into();
        loaded_ids.sort_unstable();

        assert_eq!(ids, loaded_ids);
    }
}
