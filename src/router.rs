use actix_web::{error, web, HttpResponse, Result, Scope};
use std::cell::RefCell;
use std::ops::{Deref, DerefMut};

use serde::Deserialize;

use crate::player::Player;
use crate::resources::{ResourceCount, ResourceID};
use crate::snowflake::{Snowflake, SnowflakeGenerator};
use crate::store::{SharedStore, Store, StoreBackend};

type SnowflakeGeneratorState = web::Data<RefCell<SnowflakeGenerator>>;

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
#[serde(tag = "op", content = "d")]
enum Transaction {
    Add((ResourceID, ResourceCount)),
    Sub((ResourceID, ResourceCount)),
    Set((ResourceID, ResourceCount)),
    TransferFrom((Snowflake, ResourceID, ResourceCount)),
}

// POST /players/{playerid}/resources
fn player_resource_transaction<T, U>(
    path: web::Path<(Snowflake,)>,
    shared_store: web::Data<T>,
    transactions: web::Json<Vec<Transaction>>,
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
        let mut pl = pl_ref.lock().unwrap();

        for transaction in transactions.iter() {
            match transaction {
                Transaction::Add(data) => {
                    let (id, count) = data;
                    let cur = pl.get_resource(*id).unwrap_or(0);

                    pl.set_resource(*id, cur + count);
                }
                Transaction::Sub(data) => {
                    let (id, count) = data;
                    let cur = pl.get_resource(*id).unwrap_or(0);

                    if *count > cur {
                        return Ok(HttpResponse::BadRequest()
                            .content_type("plain/text")
                            .body(format!(
                            "invalid transaction (attempted to subtract {} from {} of resource {})",
                            count, cur, id
                        )));
                    }

                    pl.set_resource(*id, cur - count);
                }
                Transaction::Set(data) => {
                    let (id, count) = data;
                    pl.set_resource(*id, *count);
                }
                Transaction::TransferFrom(data) => {
                    let (other_id, rsc_id, count) = data;
                    let other_ref = store
                        .load(*other_id)
                        .map_err(error::ErrorInternalServerError)?;
                    let mut other_pl = other_ref.lock().unwrap();

                    let cur_a = pl.get_resource(*rsc_id).unwrap_or(0);
                    let cur_b = other_pl.get_resource(*rsc_id).unwrap_or(0);

                    if *count > cur_b {
                        return Ok(HttpResponse::BadRequest()
                            .content_type("plain/text")
                            .body(format!(
                            "invalid transaction (attempted to subtract {} from {} of resource {})",
                            count, cur_b, rsc_id
                        )));
                    }

                    pl.set_resource(*rsc_id, cur_a + count);
                    other_pl.set_resource(*rsc_id, cur_b - count);

                    let r: &Player = other_pl.deref();
                    store
                        .store(*other_id, r)
                        .map_err(error::ErrorInternalServerError)?;
                }
            }
        }

        let r: &Player = pl.deref();
        store
            .store(id, r)
            .map_err(error::ErrorInternalServerError)?;
        Ok(HttpResponse::Ok().json(r))
    }
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
        .route(
            "/{playerid}/resources",
            web::post().to(player_resource_transaction::<T, U>),
        )
        .route("/{playerid}", web::get().to(get_player::<T, U>))
        .route("/{playerid}", web::delete().to(delete_player::<T, U>))
        .route("/new", web::post().to(new_player::<T, U>))
        .route("", web::get().to(list_players::<T, U>))
}
