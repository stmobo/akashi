//! Implementations of basic components for use in games.

pub mod card_type;
pub mod inventory;
pub mod resource;

#[doc(inline)]
pub use card_type::{AttachedCardType, CardType, CardTypeLayer};

#[doc(inline)]
pub use inventory::{Inventory, InventoryBackendWrapper};

#[doc(inline)]
pub use resource::Resource;

pub use resource::{InvalidAddition, InvalidSet, InvalidSoftCapAdjustment, InvalidSubtraction};

// pub mod card_text;
// pub use card_text::CardText;
