use serde::Deserialize;

use actix_web::web;
use std::cell::RefCell;

use crate::snowflake::SnowflakeGenerator;

#[cfg(test)]
use actix_web::{dev, HttpResponse};

#[cfg(test)]
use crate::local_storage::SharedLocalStore;

pub type SnowflakeGeneratorState = web::Data<RefCell<SnowflakeGenerator>>;

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

pub type BoxedError = Box<dyn std::error::Error + Send>;
pub type Result<T> = std::result::Result<T, BoxedError>;

#[cfg(test)]
pub fn snowflake_generator(group_id: u64, worker_id: u64) -> SnowflakeGeneratorState {
    web::Data::new(RefCell::new(SnowflakeGenerator::new(group_id, worker_id)))
}

#[cfg(test)]
pub fn store() -> web::Data<SharedLocalStore> {
    web::Data::new(SharedLocalStore::new())
}

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
