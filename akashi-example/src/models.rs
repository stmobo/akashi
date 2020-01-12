// Data models used in the API exposed by this example game.
use failure::Error;
use serde::{Deserialize, Serialize};

use akashi::components::{Inventory, Resource};
use akashi::{Card, Component, Entity, EntityManager, Player, Snowflake};

use failure::format_err;

#[derive(Debug, Clone)]
pub struct ResourceA(pub Resource);

impl Component<Player> for ResourceA {}

impl From<i64> for ResourceA {
    fn from(val: i64) -> ResourceA {
        ResourceA(Resource::new(val, Some(0), None))
    }
}

impl Default for ResourceA {
    fn default() -> ResourceA {
        ResourceA(Resource::new(0, Some(0), None))
    }
}

/// Player data exposed by the game API.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerModel {
    pub id: Snowflake,
    pub resource_a: i64,
    pub cards: Vec<CardModel>,
}

impl PlayerModel {
    pub fn new(pl: &Player, entity_manager: &EntityManager) -> Result<PlayerModel, Error> {
        let rsc_a: Option<ResourceA> = pl.get_component()?;
        let inv: Option<Inventory> = pl.get_component()?;
        let mut inv_model: Vec<CardModel> = Vec::new();

        if let Some(mut v) = inv {
            inv_model.reserve(v.len());
            let ids: Vec<Snowflake> = v.iter_ids().copied().collect();
            for id in ids {
                let card = v
                    .get(id, entity_manager)
                    .ok_or_else(|| format_err!("could not find card with ID {}", id))?;
                inv_model.push(CardModel::new(card)?);
            }
        }

        Ok(PlayerModel {
            id: pl.id(),
            resource_a: rsc_a.map_or(0, |r| r.0.val()),
            cards: inv_model,
        })
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum CardType {
    TypeA,
    TypeB,
    TypeC,
    TypeD,
}

impl Component<Card> for CardType {}

#[derive(Debug, Clone)]
pub struct CardName(String);

impl CardName {
    pub fn new(name: String) -> CardName {
        CardName(name)
    }
}

impl Component<Card> for CardName {}

#[derive(Debug, Clone)]
pub struct CardValue(f64);

impl CardValue {
    pub fn new(val: f64) -> CardValue {
        CardValue(val)
    }
}

impl Component<Card> for CardValue {}

/// Card data exposed by the game API.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CardModel {
    pub id: Snowflake,
    pub card_type: CardType,
    pub name: String,
    pub value: f64,
}

impl CardModel {
    pub fn new(card: &Card) -> Result<CardModel, Error> {
        let name: Option<CardName> = card.get_component()?;
        let value: Option<CardValue> = card.get_component()?;
        let card_type: CardType = card.get_component()?.expect("found card with no type");

        Ok(CardModel {
            id: card.id(),
            name: name.map_or_else(|| String::from(""), |r| r.0),
            value: value.map_or(1.0, |r| r.0),
            card_type,
        })
    }

    pub fn as_card(self, entity_manager: &EntityManager) -> Result<Card, Error> {
        let mut card: Card = entity_manager.create(self.id).unwrap();
        let name = CardName(self.name);
        let value = CardValue(self.value);

        card.set_component(name)?;
        card.set_component(value)?;
        card.set_component(self.card_type)?;

        Ok(card)
    }
}
