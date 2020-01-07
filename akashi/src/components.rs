//! Implementations of basic components for use in games.

pub mod resource;
pub use resource::{
    InvalidAddition, InvalidSet, InvalidSoftCapAdjustment, InvalidSubtraction, Resource,
};

pub mod card_type;
pub use card_type::{AttachedCardType, CardType, CardTypeLayer};

// pub mod card_text;
// pub use card_text::CardText;
