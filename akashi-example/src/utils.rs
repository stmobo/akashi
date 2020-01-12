use actix_web::error::BlockingError;
use actix_web::http::{header, StatusCode};
use actix_web::{error, web, HttpResponse};
use failure::{Error, Fail};
use serde::Deserialize;

use std::sync::Mutex;

use crate::models::{CardName, CardType, CardValue, ResourceA};

use akashi::components::InventoryBackendWrapper;
use akashi::local_storage::{LocalComponentStorage, LocalEntityStorage};
use akashi::{Card, EntityManager, Player, Snowflake, SnowflakeGenerator};

#[cfg(test)]
use std::any::type_name;

#[cfg(test)]
use actix_web::dev;

pub type SnowflakeGeneratorState = web::Data<Mutex<SnowflakeGenerator>>;

pub fn setup_entity_manager() -> EntityManager {
    let mut ent_mgr = EntityManager::new();

    ent_mgr
        .register_entity(LocalEntityStorage::<Player>::new())
        .unwrap();

    ent_mgr
        .register_entity(LocalEntityStorage::<Card>::new())
        .unwrap();

    ent_mgr
        .register_component("CardName", LocalComponentStorage::<Card, CardName>::new())
        .expect("initialization failed");
    ent_mgr
        .register_component("CardValue", LocalComponentStorage::<Card, CardValue>::new())
        .expect("initialization failed");
    ent_mgr
        .register_component("CardType", LocalComponentStorage::<Card, CardType>::new())
        .expect("initialization failed");

    ent_mgr
        .register_component(
            "Inventory",
            InventoryBackendWrapper::new(LocalComponentStorage::<Player, Vec<Snowflake>>::new()),
        )
        .expect("initialization failed");

    ent_mgr
        .register_component(
            "ResourceA",
            LocalComponentStorage::<Player, ResourceA>::new(),
        )
        .expect("initialization failed");

    ent_mgr
}

#[cfg(test)]
pub fn snowflake_generator(group_id: u64, worker_id: u64) -> SnowflakeGeneratorState {
    web::Data::new(Mutex::new(SnowflakeGenerator::new(group_id, worker_id)))
}

#[cfg(test)]
pub fn create_new_player(
    ent_mgr: &EntityManager,
    snowflake_gen: &mut SnowflakeGenerator,
) -> (Snowflake, Player) {
    let pl: Player = ent_mgr.create(snowflake_gen.generate()).unwrap();
    let id = pl.id();

    (id, pl)
}

#[cfg(test)]
pub fn create_new_card(
    ent_mgr: &EntityManager,
    snowflake_gen: &mut SnowflakeGenerator,
) -> (Snowflake, Card) {
    let card: Card = ent_mgr.create(snowflake_gen.generate()).unwrap();
    let id = card.id();

    (id, card)
}

pub fn convert_blocking_err(e: BlockingError<Error>) -> Error {
    match e {
        BlockingError::Error(inside) => inside,
        BlockingError::Canceled => ThreadCancelledError.into(),
    }
}

#[derive(Deserialize)]
#[serde(default)]
pub struct Pagination {
    pub page: u64,
    pub limit: u64,
}

impl Pagination {
    pub fn new() -> Pagination {
        Pagination { page: 0, limit: 20 }
    }
}

impl Default for Pagination {
    fn default() -> Pagination {
        Pagination::new()
    }
}

#[derive(Fail, Debug)]
#[fail(display = "Could not find {} with ID {}", obj_type, id)]
pub struct ObjectNotFoundError {
    obj_type: &'static str,
    id: Snowflake,
}

impl ObjectNotFoundError {
    pub fn new(obj_type: &'static str, id: Snowflake) -> ObjectNotFoundError {
        ObjectNotFoundError { obj_type, id }
    }
}

pub fn player_not_found(id: Snowflake) -> Error {
    ObjectNotFoundError::new("player", id).into()
}

pub fn card_not_found(id: Snowflake) -> Error {
    ObjectNotFoundError::new("card", id).into()
}

impl error::ResponseError for ObjectNotFoundError {
    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(StatusCode::NOT_FOUND)
            .set_header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(self.to_string())
    }
}

#[derive(Fail, Debug)]
#[fail(display = "Invalid transaction: {}", _0)]
pub struct BadTransactionError(String);

impl error::ResponseError for BadTransactionError {
    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(StatusCode::BAD_REQUEST)
            .set_header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(self.to_string())
    }
}

impl BadTransactionError {
    pub fn new(msg: String) -> BadTransactionError {
        BadTransactionError(msg)
    }
}

#[derive(Fail, Debug)]
#[fail(display = "Thread cancelled")]
pub struct ThreadCancelledError;

#[cfg(test)]
pub fn get_body_str(resp: &HttpResponse) -> &str {
    let body = resp.body().as_ref().unwrap();
    match body {
        dev::Body::Bytes(body) => {
            std::str::from_utf8(body).expect("Could not deserialize body bytes as UTF-8")
        }
        _ => panic!(format!("Expected body bytes, got {:?}", body)),
    }
}

#[cfg(test)]
pub fn get_body_json<'a, T: Deserialize<'a>>(resp: &'a HttpResponse) -> T {
    let s = get_body_str(resp);
    serde_json::from_str(s).expect("Could not deserialize JSON response")
}

#[cfg(test)]
pub fn expect_error<E: Fail>(resp: Result<HttpResponse, Error>) -> E {
    let err = resp.expect_err("expected error, got valid response");
    match err.downcast() {
        Ok(v) => v,
        Err(e) => panic!("expected {}, got {:?}", type_name::<E>(), e),
    }
}
