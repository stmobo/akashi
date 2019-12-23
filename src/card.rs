use crate::snowflake::Snowflake;

pub struct Card {
    id: Snowflake,
    type_id: Snowflake,
    locked: bool,
}

pub struct CardType {
    id: Snowflake,
}
