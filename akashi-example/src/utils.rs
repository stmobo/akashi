use actix_web::error::BlockingError;
use actix_web::http::{header, StatusCode};
use actix_web::{error, web, HttpResponse};
use failure::{Error, Fail};
use serde::Deserialize;

use std::sync::Mutex;

use crate::models::{CardName, CardType, CardValue, ResourceA};

use akashi::local_storage::{LocalComponentStorage, LocalInventoryStore, SharedLocalStore};
use akashi::{Card, ComponentManager, Player, Snowflake, SnowflakeGenerator};

#[cfg(test)]
use std::any::type_name;

#[cfg(test)]
use std::sync::Arc;

#[cfg(test)]
use actix_web::dev;

pub type SnowflakeGeneratorState = web::Data<Mutex<SnowflakeGenerator>>;

pub fn card_component_manager() -> ComponentManager<Card> {
    let mut cm = ComponentManager::new();
    cm.register_component("CardName", LocalComponentStorage::<Card, CardName>::new());
    cm.register_component("CardValue", LocalComponentStorage::<Card, CardValue>::new());
    cm.register_component("CardType", LocalComponentStorage::<Card, CardType>::new());
    cm
}

pub fn player_component_manager(shared_store: &SharedLocalStore) -> ComponentManager<Player> {
    let mut cm = ComponentManager::new();
    cm.register_component(
        "Inventory",
        LocalInventoryStore::new(shared_store.backend()),
    );
    cm.register_component(
        "ResourceA",
        LocalComponentStorage::<Player, ResourceA>::new(),
    );

    cm
}

#[cfg(test)]
pub fn snowflake_generator(group_id: u64, worker_id: u64) -> SnowflakeGeneratorState {
    web::Data::new(Mutex::new(SnowflakeGenerator::new(group_id, worker_id)))
}

#[cfg(test)]
pub fn store() -> web::Data<SharedLocalStore> {
    web::Data::new(SharedLocalStore::new())
}

#[cfg(test)]
pub fn create_new_player(
    shared_store: &SharedLocalStore,
    snowflake_gen: &mut SnowflakeGenerator,
    cm: Arc<ComponentManager<Player>>,
) -> (Snowflake, Player) {
    let players = shared_store.players();
    let pl = Player::empty(snowflake_gen, cm);
    let pl_id = pl.id();
    players.store(pl.clone()).unwrap();

    (pl_id, pl)
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
