//! Implementations of basic components for use in games.

pub mod card_type;
pub mod inventory;
pub mod resource;

pub use card_type::{AttachedCardType, CardType, CardTypeLayer};
pub use inventory::Inventory;
pub use resource::{
    InvalidAddition, InvalidSet, InvalidSoftCapAdjustment, InvalidSubtraction, Resource,
};

// pub mod card_text;
// pub use card_text::CardText;
