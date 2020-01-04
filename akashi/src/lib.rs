#[macro_use]
extern crate failure;
extern crate failure_derive;

pub mod card;
pub mod component;
pub mod local_storage;
pub mod player;
pub mod resources;
pub mod snowflake;
pub mod store;

pub use snowflake::{Snowflake, SnowflakeGenerator};
pub use player::Player;
pub use card::{Card, Inventory};
pub use component::{Component, ComponentsAttached, ComponentStore, ComponentManager};
pub use store::{Store, StoreHandle, StoreBackend};
pub use resources::Resource;
