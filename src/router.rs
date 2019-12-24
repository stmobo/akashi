use actix_web::{error, web, HttpResponse, Result, Scope};
use std::cell::RefCell;
use std::ops::{Deref, DerefMut};

use serde::Deserialize;

use crate::player::Player;
use crate::snowflake::{Snowflake, SnowflakeGenerator};
use crate::store::{SharedStore, Store, StoreBackend};

type SnowflakeGeneratorState = web::Data<RefCell<SnowflakeGenerator>>;

// GET /players/{playerid}
fn get_player<T, U>(
    path: web::Path<(Snowflake,)>,
    shared_store: web::Data<T>,
) -> Result<HttpResponse>
where
    T: SharedStore<Player, U>,
    U: StoreBackend<Player>,
{
    let id: Snowflake = path.0;
    let store: &Store<Player, U> = shared_store.get_store();

    let exists = store.exists(id).map_err(error::ErrorInternalServerError)?;
    if !exists {
        return Ok(HttpResponse::NotFound()
            .content_type("plain/text")
            .body(format!("Could not find player {}", id)));
    }

    let pl_ref = store.load(id).map_err(error::ErrorInternalServerError)?;
    {
        let pl = pl_ref.lock().unwrap();
        let r: &Player = pl.deref();
        Ok(HttpResponse::Ok().json(r))
    }
}

#[derive(Deserialize)]
#[serde(default)]
struct Pagination {
    page: u64,
    limit: u64,
}

impl Pagination {
    fn new() -> Pagination {
        Pagination { page: 0, limit: 20 }
    }
}

impl Default for Pagination {
    fn default() -> Pagination {
        Pagination::new()
    }
}

// GET /players
fn list_players<T, U>(
    query: web::Query<Pagination>,
    shared_store: web::Data<T>,
) -> Result<HttpResponse>
where
    T: SharedStore<Player, U>,
    U: StoreBackend<Player>,
{
    let store: &Store<Player, U> = shared_store.get_store();
    let keys: Vec<Snowflake> = store
        .keys(query.page, query.limit)
        .map_err(error::ErrorInternalServerError)?;
    Ok(HttpResponse::Ok().json(keys))
}

// DELETE /players/{playerid}
fn delete_player<T, U>(
    path: web::Path<(Snowflake,)>,
    shared_store: web::Data<T>,
) -> Result<HttpResponse>
where
    T: SharedStore<Player, U>,
    U: StoreBackend<Player>,
{
    let id: Snowflake = path.0;
    let store: &Store<Player, U> = shared_store.get_store();

    let exists = store.exists(id).map_err(error::ErrorInternalServerError)?;
    if !exists {
        return Ok(HttpResponse::NotFound()
            .content_type("plain/text")
            .body(format!("Could not find player {}", id)));
    }

    store.delete(id).map_err(error::ErrorInternalServerError)?;
    Ok(HttpResponse::NoContent().finish())
}

// POST /players/new
fn new_player<T, U>(shared_store: web::Data<T>, sg: SnowflakeGeneratorState) -> Result<HttpResponse>
where
    T: SharedStore<Player, U>,
    U: StoreBackend<Player>,
{
    let mut snowflake_gen = sg.borrow_mut();
    let store: &Store<Player, U> = shared_store.get_store();
    let pl = Player::empty(snowflake_gen.deref_mut());

    store
        .store(*pl.id(), &pl)
        .map_err(error::ErrorInternalServerError)?;
    Ok(HttpResponse::Ok().json(pl))
}

pub fn bind_routes<T, U>(scope: Scope) -> Scope
where
    T: SharedStore<Player, U> + 'static,
    U: StoreBackend<Player> + 'static,
{
    scope
        .route("/{playerid}", web::get().to(get_player::<T, U>))
        .route("/{playerid}", web::delete().to(delete_player::<T, U>))
        .route("/new", web::post().to(new_player::<T, U>))
        .route("", web::get().to(list_players::<T, U>))
}
