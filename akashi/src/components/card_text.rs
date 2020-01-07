//! A simple structure for holding basic card text.

use crate::card::Card;
use crate::ecs::Component;

#[derive(Debug, Clone)]
pub struct CardText {
    title: String,
    subtitle: String,
    description: String,
}

impl CardText {
    pub fn new(title: String, subtitle: String, description: String) -> CardText {
        CardText {
            title,
            subtitle,
            description,
        }
    }

    pub fn title(&self) -> &str {
        self.title.as_str()
    }

    pub fn subtitle(&self) -> &str {
        self.subtitle.as_str()
    }

    pub fn description(&self) -> &str {
        self.description.as_str()
    }
}

impl Component<Card> for CardText {}
