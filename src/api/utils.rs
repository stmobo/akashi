use actix_web::http::{header, StatusCode};
use actix_web::{error, web, HttpResponse};
use failure::Fail;
use serde::Deserialize;

use std::cell::RefCell;
use std::fmt;

use crate::snowflake::SnowflakeGenerator;

#[cfg(test)]
use actix_web::dev;

#[cfg(test)]
use crate::local_storage::SharedLocalStore;

#[cfg(test)]
use crate::card::Card;

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

pub type BoxedError = Box<dyn std::error::Error + Sync + Send>;
pub type Result<T> = std::result::Result<T, APIError>;

#[derive(Fail, Debug)]
pub enum APIError {
    NotFound(String),
    BadTransaction(String),
    ThreadCancelled,
    Other(BoxedError),
}

impl fmt::Display for APIError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status = <Self as error::ResponseError>::status_code(self);
        match self {
            APIError::NotFound(v) => write!(f, "error {}: {}", status, v),
            APIError::BadTransaction(v) => write!(f, "error {}: {}", status, v),
            APIError::ThreadCancelled => write!(f, "error {}: internal thread cancelled", status),
            APIError::Other(v) => write!(f, "error {}: {}", status, v),
        }
    }
}

impl error::ResponseError for APIError {
    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code())
            .set_header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(self.to_string())
    }

    fn status_code(&self) -> StatusCode {
        match self {
            APIError::NotFound(_v) => StatusCode::NOT_FOUND,
            APIError::BadTransaction(_v) => StatusCode::BAD_REQUEST,
            APIError::ThreadCancelled => StatusCode::INTERNAL_SERVER_ERROR,
            APIError::Other(_v) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<BoxedError> for APIError {
    fn from(err: BoxedError) -> Self {
        APIError::Other(err)
    }
}

type BlockingAPIError = error::BlockingError<APIError>;
impl From<BlockingAPIError> for APIError {
    fn from(err: error::BlockingError<APIError>) -> Self {
        match err {
            BlockingAPIError::Error(e) => e,
            BlockingAPIError::Canceled => APIError::ThreadCancelled,
        }
    }
}

impl APIError {
    pub fn not_found(msg: String) -> APIError {
        APIError::NotFound(msg)
    }
}

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

#[cfg(test)]
pub fn generate_random_card(snowflake_gen: &mut SnowflakeGenerator) -> Card {
    let type_id = snowflake_gen.generate();
    Card::generate(snowflake_gen, type_id)
}

#[cfg(test)]
pub fn expect_not_found(resp: Result<HttpResponse>) {
    let resp: APIError = match resp {
        Ok(v) => panic!(
            "expected APIError, got response with status code {}",
            v.status()
        ),
        Err(v) => v,
    };

    match resp {
        APIError::NotFound(_v) => {}
        _ => panic!("expected APIError::NotFound, got {:?}", resp),
    };
}
