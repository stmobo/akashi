use actix_web::{web, HttpResponse, Scope};

use serde::Deserialize;

use crate::player::Player;
use crate::resources::{ResourceCount, ResourceID};
use crate::snowflake::Snowflake;
use crate::store::{SharedStore, Store, StoreBackend};

use super::utils::{APIError, Pagination, Result, SnowflakeGeneratorState};

// GET /players
async fn list_players<T, U>(
    query: web::Query<Pagination>,
    shared_store: web::Data<T>,
) -> Result<HttpResponse>
where
    T: SharedStore<Player, U> + Send + Sync + 'static,
    U: StoreBackend<Player> + Send + Sync + 'static,
{
    let players: Vec<Player> = web::block(move || -> Result<Vec<Player>> {
        let store: &Store<Player, U> = shared_store.get_store();
        let keys = store.keys(query.page, query.limit)?;

        let vals: Vec<Player> = keys
            .iter()
            .filter_map(|key| -> Option<Player> {
                let wrapper = store.load(*key).ok()?;
                let handle = wrapper.lock().ok()?;
                match handle.get() {
                    None => None,
                    Some(pl) => Some(pl.clone()),
                }
            })
            .collect();
        Ok(vals)
    })
    .await?;

    Ok(HttpResponse::Ok().json(players))
}

// GET /players/{playerid}
async fn get_player<T, U>(
    path: web::Path<(Snowflake,)>,
    shared_store: web::Data<T>,
) -> Result<HttpResponse>
where
    T: SharedStore<Player, U> + Send + Sync + 'static,
    U: StoreBackend<Player> + Send + Sync + 'static,
{
    let id: Snowflake = path.0;

    let r: Player = web::block(move || -> Result<Player> {
        let store: &Store<Player, U> = shared_store.get_store();
        let pl_ref = store.load(id)?;

        let handle = pl_ref.lock().unwrap();
        match handle.get() {
            None => Err(APIError::not_found(format!("Could not find player {}", id))),
            Some(r) => Ok(r.clone()),
        }
    })
    .await?;

    Ok(HttpResponse::Ok().json(r))
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
async fn player_resource_transaction<T, U>(
    path: web::Path<(Snowflake,)>,
    shared_store: web::Data<T>,
    transactions: web::Json<Vec<Transaction>>,
) -> Result<HttpResponse>
where
    T: SharedStore<Player, U> + Send + Sync + 'static,
    U: StoreBackend<Player> + Send + Sync + 'static,
{
    let id: Snowflake = path.0;
    let res = web::block(move || -> Result<Player> {
        let store: &Store<Player, U> = shared_store.get_store();
        let pl_ref = store.load(id)?;

        let mut handle = pl_ref.lock().unwrap();
        if !handle.exists() {
            return Err(APIError::not_found(format!("Could not find player {}", id)));
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
                        return Err(APIError::bad_transaction(format!(
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
                    let other_ref = store.load(*other_id)?;

                    let mut other_pl_handle = other_ref.lock().unwrap();
                    let other_pl = match other_pl_handle.get_mut() {
                        None => {
                            return Err(APIError::not_found(format!(
                                "Could not find player {}",
                                other_id
                            )));
                        }
                        Some(r) => r,
                    };

                    let cur_a = pl.get_resource(*rsc_id).unwrap_or(0);
                    let cur_b = other_pl.get_resource(*rsc_id).unwrap_or(0);

                    if *count > cur_b {
                        return Err(APIError::bad_transaction(format!(
                            "invalid transaction (attempted to subtract {} from {} of resource {})",
                            count, cur_b, rsc_id
                        )));
                    }

                    pl.set_resource(*rsc_id, cur_a + count);
                    other_pl.set_resource(*rsc_id, cur_b - count);

                    other_pl_handle.store()?;
                }
            }
        }

        handle.store()?;
        Ok(handle.get().unwrap().clone())
    })
    .await?;

    Ok(HttpResponse::Ok().json(res))
}

// DELETE /players/{playerid}
async fn delete_player<T, U>(
    path: web::Path<(Snowflake,)>,
    shared_store: web::Data<T>,
) -> Result<HttpResponse>
where
    T: SharedStore<Player, U> + Send + Sync + 'static,
    U: StoreBackend<Player> + Send + Sync + 'static,
{
    let id: Snowflake = path.0;

    web::block(move || -> Result<()> {
        let store: &Store<Player, U> = shared_store.get_store();
        let wrapper = store.load(id)?;
        let mut handle = wrapper.lock().unwrap();

        if !handle.exists() {
            Err(APIError::not_found(format!("Could not find player {}", id)))
        } else {
            handle.delete()?;
            Ok(())
        }
    })
    .await?;

    Ok(HttpResponse::NoContent().finish())
}

// POST /players/new
async fn new_player<T, U>(
    shared_store: web::Data<T>,
    sg: SnowflakeGeneratorState,
) -> Result<HttpResponse>
where
    T: SharedStore<Player, U> + Send + Sync + 'static,
    U: StoreBackend<Player> + Send + Sync + 'static,
{
    let pl = web::block(move || -> Result<Player> {
        let new_pl: Player;

        {
            let mut snowflake_gen = sg.lock().expect("snowflake generator lock poisoned");
            new_pl = Player::empty(&mut snowflake_gen);
        }

        let store: &Store<Player, U> = shared_store.get_store();

        let wrapper = store.load(*new_pl.id())?;
        let mut handle = wrapper.lock().unwrap();

        handle.replace(new_pl);
        handle.store()?;

        Ok(handle.get().unwrap().clone())
    })
    .await?;

    Ok(HttpResponse::Ok().json(pl))
}

pub fn bind_routes<T, U>(scope: Scope) -> Scope
where
    T: SharedStore<Player, U> + Send + Sync + 'static,
    U: StoreBackend<Player> + Send + Sync + 'static,
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
    use actix_web::http;
    use futures::executor::block_on;

    use crate::api::utils;
    use crate::api::utils::{get_body_json, get_body_str, snowflake_generator, store};

    use crate::local_storage::SharedLocalStore;
    use crate::snowflake::SnowflakeGenerator;

    #[test]
    fn test_new_player() {
        let shared_store = store();
        let sg = snowflake_generator(0, 0);

        let resp = block_on(new_player(shared_store.clone(), sg)).unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);

        let players = shared_store.players();
        assert_eq!(players.keys(0, 20).unwrap().len(), 1);
    }

    #[test]
    fn test_get_player_exists() {
        let shared_store = SharedLocalStore::new();
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);

        let players = shared_store.players();
        let pl = Player::empty(&mut snowflake_gen);
        let id = *pl.id();
        players.store(id, pl.clone()).unwrap();

        let resp = block_on(get_player(
            web::Path::from((id,)),
            web::Data::new(shared_store),
        ))
        .unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);

        let body: Player = get_body_json(&resp);
        assert_eq!(pl, body);
    }

    #[test]
    fn test_get_player_not_exists() {
        let shared_store = store();
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let id = snowflake_gen.generate();

        let resp = block_on(get_player(web::Path::from((id,)), shared_store));
        utils::expect_not_found(resp);
    }

    #[test]
    fn test_delete_player_exists() {
        let shared_store = store();
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);

        let players = shared_store.players();
        let pl = Player::empty(&mut snowflake_gen);
        let id = *pl.id();
        players.store(id, pl.clone()).unwrap();

        assert_eq!(players.keys(0, 20).unwrap().len(), 1);

        let resp = block_on(delete_player(web::Path::from((id,)), shared_store.clone())).unwrap();
        assert_eq!(resp.status(), http::StatusCode::NO_CONTENT);
        assert_eq!(shared_store.players().keys(0, 20).unwrap().len(), 0);
    }

    #[test]
    fn test_delete_player_not_exists() {
        let shared_store = store();
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let id = snowflake_gen.generate();

        let resp = block_on(delete_player(web::Path::from((id,)), shared_store));
        utils::expect_not_found(resp);
    }

    #[test]
    fn test_player_transaction_add() {
        let shared_store = store();
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let pl = Player::empty(&mut snowflake_gen);

        let players = shared_store.players();
        let id = *pl.id();
        players.store(id, pl).unwrap();

        let resp = block_on(player_resource_transaction(
            web::Path::from((id,)),
            shared_store.clone(),
            web::Json(vec![Transaction::Add((0, 10))]),
        ))
        .unwrap();

        assert_eq!(resp.status(), http::StatusCode::OK);

        {
            let wrapper = players.load(id).unwrap();
            let handle = wrapper.lock().unwrap();

            let stored_pl = handle.get().unwrap().clone();
            let resp_pl: Player = get_body_json(&resp);

            assert_eq!(resp_pl, stored_pl);
            assert_eq!(stored_pl.get_resource(0).unwrap(), 10);
        }
    }

    #[test]
    fn test_player_transaction_sub() {
        let shared_store = store();
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let mut pl = Player::empty(&mut snowflake_gen);

        pl.set_resource(0, 50);

        let players = shared_store.players();
        let id = *pl.id();
        players.store(id, pl.clone()).unwrap();

        let resp = block_on(player_resource_transaction(
            web::Path::from((id,)),
            shared_store.clone(),
            web::Json(vec![Transaction::Sub((0, 25))]),
        ))
        .unwrap();

        assert_eq!(resp.status(), http::StatusCode::OK);

        {
            let wrapper = players.load(id).unwrap();
            let handle = wrapper.lock().unwrap();

            let stored_pl = handle.get().unwrap().clone();
            let resp_pl: Player = get_body_json(&resp);

            assert_eq!(resp_pl, stored_pl);
            assert_eq!(stored_pl.get_resource(0).unwrap(), 25);
        }
    }

    #[test]
    fn test_player_transaction_sub_validate() {
        let shared_store = store();
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let mut pl = Player::empty(&mut snowflake_gen);

        pl.set_resource(0, 50);
        let expected_player = pl.clone();

        let players = shared_store.players();
        let id = *pl.id();
        players.store(id, pl).unwrap();

        let resp = block_on(player_resource_transaction(
            web::Path::from((id,)),
            shared_store.clone(),
            web::Json(vec![Transaction::Sub((0, 60))]),
        ));
        utils::expect_bad_transaction(resp);

        {
            let wrapper = players.load(id).unwrap();
            let handle = wrapper.lock().unwrap();
            let stored_pl = handle.get().unwrap().clone();

            assert_eq!(stored_pl, expected_player);
            assert_eq!(stored_pl.get_resource(0).unwrap(), 50);
        }
    }

    #[test]
    fn test_player_transaction_set() {
        let shared_store = store();
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let pl = Player::empty(&mut snowflake_gen);

        let players = shared_store.players();
        let id = *pl.id();
        players.store(id, pl).unwrap();

        let resp = block_on(player_resource_transaction(
            web::Path::from((id,)),
            shared_store.clone(),
            web::Json(vec![Transaction::Set((0, 100))]),
        ))
        .unwrap();

        assert_eq!(resp.status(), http::StatusCode::OK);

        {
            let wrapper = players.load(id).unwrap();
            let handle = wrapper.lock().unwrap();

            let stored_pl = handle.get().unwrap().clone();
            let resp_pl: Player = get_body_json(&resp);

            assert_eq!(resp_pl, stored_pl);
            assert_eq!(stored_pl.get_resource(0).unwrap(), 100);
        }
    }

    #[test]
    fn test_player_transaction_transfer() {
        let shared_store = store();
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let mut pl_1 = Player::empty(&mut snowflake_gen);
        let mut pl_2 = Player::empty(&mut snowflake_gen);

        pl_1.set_resource(0, 110);
        pl_2.set_resource(0, 0);

        let players = shared_store.players();
        let id_1 = *pl_1.id();
        let id_2 = *pl_2.id();

        players.store(id_1, pl_1.clone()).unwrap();
        players.store(id_2, pl_2.clone()).unwrap();

        let resp = block_on(player_resource_transaction(
            web::Path::from((id_2,)),
            shared_store.clone(),
            web::Json(vec![Transaction::TransferFrom((id_1, 0, 50))]),
        ))
        .unwrap();

        assert_eq!(resp.status(), http::StatusCode::OK);

        {
            let wrapper = players.load(id_2).unwrap();
            let handle = wrapper.lock().unwrap();

            let stored_pl = handle.get().unwrap().clone();
            let resp_pl: Player = get_body_json(&resp);

            assert_eq!(resp_pl, stored_pl);
            assert_eq!(stored_pl.get_resource(0).unwrap(), 50);
        }

        {
            let wrapper = players.load(id_1).unwrap();
            let handle = wrapper.lock().unwrap();
            let stored_pl = handle.get().unwrap().clone();

            assert_eq!(stored_pl.get_resource(0).unwrap(), 60);
        }
    }

    #[test]
    fn test_player_transaction_transfer_validate() {
        let shared_store = store();
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let mut pl_1 = Player::empty(&mut snowflake_gen);
        let mut pl_2 = Player::empty(&mut snowflake_gen);

        pl_1.set_resource(0, 50);
        pl_2.set_resource(0, 0);

        let players = shared_store.players();
        let id_1 = *pl_1.id();
        let id_2 = *pl_2.id();

        players.store(id_1, pl_1.clone()).unwrap();
        players.store(id_2, pl_2.clone()).unwrap();

        let resp = block_on(player_resource_transaction(
            web::Path::from((id_2,)),
            shared_store.clone(),
            web::Json(vec![Transaction::TransferFrom((id_1, 0, 60))]),
        ));
        utils::expect_bad_transaction(resp);

        {
            let wrapper = players.load(id_1).unwrap();
            let handle = wrapper.lock().unwrap();
            let stored_pl = handle.get().unwrap().clone();

            assert_eq!(stored_pl, pl_1);
        }

        {
            let wrapper = players.load(id_2).unwrap();
            let handle = wrapper.lock().unwrap();
            let stored_pl = handle.get().unwrap().clone();

            assert_eq!(stored_pl, pl_2);
        }
    }

    #[test]
    fn test_list_players_empty() {
        let shared_store = store();
        let query = web::Query::<Pagination>::from_query("?page=0&limit=20").unwrap();

        let resp = block_on(list_players(query, shared_store)).unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);

        let body = get_body_str(&resp);
        assert_eq!(body, "[]");
    }

    #[test]
    fn test_list_players_nonempty() {
        let shared_store = store();
        let query = web::Query::<Pagination>::from_query("?page=0&limit=20").unwrap();
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);

        let players = shared_store.players();
        let pl = Player::empty(&mut snowflake_gen);
        let id = *pl.id();
        players.store(id, pl.clone()).unwrap();

        let resp = block_on(list_players(query, shared_store)).unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);

        let body: Vec<Player> = get_body_json(&resp);
        assert_eq!(body, vec![pl]);
    }
}
