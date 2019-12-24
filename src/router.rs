use actix_web::{error, web, HttpResponse, Result, Scope};
use std::cell::RefCell;
use std::ops::DerefMut;

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

    let pl_ref = store.load(id).map_err(error::ErrorInternalServerError)?;
    {
        let handle = pl_ref.lock().unwrap();
        match handle.get() {
            None => Ok(HttpResponse::NotFound()
                .content_type("plain/text")
                .body(format!("Could not find player {}", id))),
            Some(r) => Ok(HttpResponse::Ok().json(r)),
        }
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

    let pl_ref = store.load(id).map_err(error::ErrorInternalServerError)?;
    {
        let mut handle = pl_ref.lock().unwrap();
        if !handle.exists() {
            return Ok(HttpResponse::NotFound()
                .content_type("plain/text")
                .body(format!("Could not find player {}", id)));
        }

        let pl = handle.get_mut().unwrap();

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
                        return Ok(HttpResponse::BadRequest().content_type("plain/text").body(
                            format!(
                            "invalid transaction (attempted to subtract {} from {} of resource {})",
                            count, cur, id
                        ),
                        ));
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

                    let mut other_pl_handle = other_ref.lock().unwrap();
                    let other_pl = match other_pl_handle.get_mut() {
                        None => {
                            return Ok(HttpResponse::NotFound()
                                .content_type("plain/text")
                                .body(format!("Could not find player {}", other_id)));
                        }
                        Some(r) => r,
                    };

                    let cur_a = pl.get_resource(*rsc_id).unwrap_or(0);
                    let cur_b = other_pl.get_resource(*rsc_id).unwrap_or(0);

                    if *count > cur_b {
                        return Ok(HttpResponse::BadRequest().content_type("plain/text").body(
                            format!(
                            "invalid transaction (attempted to subtract {} from {} of resource {})",
                            count, cur_b, rsc_id
                        ),
                        ));
                    }

                    pl.set_resource(*rsc_id, cur_a + count);
                    other_pl.set_resource(*rsc_id, cur_b - count);

                    other_pl_handle
                        .store()
                        .map_err(error::ErrorInternalServerError)?;
                }
            }
        }

        handle.store().map_err(error::ErrorInternalServerError)?;
        Ok(HttpResponse::Ok().json(handle.get()))
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

    let wrapper = store.load(id).map_err(error::ErrorInternalServerError)?;
    let mut handle = wrapper.lock().unwrap();

    if !handle.exists() {
        return Ok(HttpResponse::NotFound()
            .content_type("plain/text")
            .body(format!("Could not find player {}", id)));
    }

    handle.delete().map_err(error::ErrorInternalServerError)?;
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

    let wrapper = store
        .load(*pl.id())
        .map_err(error::ErrorInternalServerError)?;
    let mut handle = wrapper.lock().unwrap();

    handle.replace(pl);
    handle.store().map_err(error::ErrorInternalServerError)?;
    Ok(HttpResponse::Ok().json(handle.get()))
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

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{dev, http, test};

    use crate::local_storage::SharedLocalStore;

    fn snowflake_generator(group_id: u64, worker_id: u64) -> SnowflakeGeneratorState {
        web::Data::new(RefCell::new(SnowflakeGenerator::new(group_id, worker_id)))
    }

    fn store() -> web::Data<SharedLocalStore> {
        web::Data::new(SharedLocalStore::new())
    }

    fn get_body_str(resp: &HttpResponse) -> &str {
        let body = resp.body().as_ref().unwrap();
        match body {
            dev::Body::Bytes(body) => {
                std::str::from_utf8(body).expect("Could not deserialize body bytes as UTF-8")
            }
            _ => panic!(format!("Expected body bytes, got {:?}", body)),
        }
    }

    fn get_body_json<'a, T: Deserialize<'a>>(resp: &'a HttpResponse) -> T {
        let s = get_body_str(resp);
        serde_json::from_str(s).expect("Could not deserialize JSON response")
    }

    #[test]
    fn test_new_player() {
        let shared_store = store();
        let sg = snowflake_generator(0, 0);

        let resp = new_player(shared_store.clone(), sg).unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);

        let players = shared_store.players();
        assert_eq!(players.keys(0, 20).unwrap().len(), 1);
    }

    #[test]
    fn test_get_player_exists() {
        let shared_store = SharedLocalStore::new();
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);

        let store = shared_store.get_store();
        let pl = Player::empty(&mut snowflake_gen);
        let id = *pl.id();

        {
            let wrapper = store.load(id).unwrap();
            let mut handle = wrapper.lock().unwrap();
            handle.replace(pl.clone());
            handle.store().unwrap();
        }

        let resp = get_player(web::Path::from((id,)), web::Data::new(shared_store)).unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);

        let body: Player = get_body_json(&resp);
        assert_eq!(pl, body);
    }

    #[test]
    fn test_get_player_not_exists() {
        let shared_store = store();
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let id = snowflake_gen.generate();

        let resp = get_player(web::Path::from((id,)), shared_store).unwrap();
        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_delete_player_exists() {
        let shared_store = store();
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);

        let players = shared_store.players();
        let pl = Player::empty(&mut snowflake_gen);
        let id = *pl.id();

        {
            let wrapper = players.load(id).unwrap();
            let mut handle = wrapper.lock().unwrap();
            handle.replace(pl.clone());
            handle.store().unwrap();
        }

        assert_eq!(players.keys(0, 20).unwrap().len(), 1);

        let resp = delete_player(web::Path::from((id,)), shared_store.clone()).unwrap();
        assert_eq!(resp.status(), http::StatusCode::NO_CONTENT);
        assert_eq!(shared_store.players().keys(0, 20).unwrap().len(), 0);
    }

    #[test]
    fn test_delete_player_not_exists() {
        let shared_store = store();
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let id = snowflake_gen.generate();

        let resp = delete_player(web::Path::from((id,)), shared_store).unwrap();
        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_list_players_empty() {
        let shared_store = store();
        let query = web::Query::<Pagination>::from_query("?page=0&limit=20").unwrap();

        let resp = list_players(query, shared_store).unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);

        let body = get_body_str(&resp);
        assert_eq!(body, "[]");
    }

    #[test]
    fn test_list_players_nonempty() {
        let shared_store = store();
        let query = web::Query::<Pagination>::from_query("?page=0&limit=20").unwrap();
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);

        let store = shared_store.get_store();
        let pl = Player::empty(&mut snowflake_gen);
        let id = *pl.id();
        {
            let wrapper = store.load(id).unwrap();
            let mut handle = wrapper.lock().unwrap();
            handle.replace(pl);
            handle.store().unwrap();
        }

        let resp = list_players(query, shared_store).unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);

        let body: Vec<Snowflake> = get_body_json(&resp);
        assert_eq!(body, vec![id]);
    }
}
