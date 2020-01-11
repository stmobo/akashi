//! A library for building collectible card and gacha games.
//!
//! # Architecture
//!
//! Akashi uses an Entity-Component-System architecture (though at the
//! moment only Entities and Components are really implemented).
//!
//! Players and cards, within the Akashi framework, are **entities**:
//! they aren't much more than a unique ID. Functionality is added by
//! attaching various **components** to entities.
//! For example, inventories can be represented as components that
//! are attached to players, while card images and text can be
//! represented as components attached to cards.

#[macro_use]
extern crate failure;

#[macro_use]
extern crate rental;

extern crate dashmap;
extern crate downcast_rs;
extern crate failure_derive;

pub mod card;
pub mod components;
pub mod ecs;
pub mod local_storage;
pub mod player;
pub mod snowflake;
mod util;

#[doc(inline)]
pub use card::Card;

#[doc(inline)]
pub use ecs::{Component, ComponentBackend, ComponentManager, Entity};

#[doc(inline)]
pub use ecs::entity_store::{EntityStore, Store, StoreBackend, StoreHandle};

#[doc(inline)]
pub use player::Player;

#[doc(inline)]
pub use snowflake::{Snowflake, SnowflakeGenerator};
