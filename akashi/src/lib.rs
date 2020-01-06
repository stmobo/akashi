#[macro_use]
extern crate failure;
extern crate chashmap;
extern crate downcast_rs;
extern crate failure_derive;

pub mod card;
pub mod ecs;
pub mod local_storage;
pub mod player;
pub mod resources;
pub mod snowflake;
pub mod store;
mod util;

pub use card::{Card, Inventory};
pub use ecs::{Component, ComponentManager, ComponentStore, Entity};
pub use player::Player;
pub use resources::Resource;
pub use snowflake::{Snowflake, SnowflakeGenerator};
pub use store::{Store, StoreBackend, StoreHandle};
