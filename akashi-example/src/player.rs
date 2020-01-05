// Example players API built on top of Akashi.

use actix_web::{web, HttpResponse, Scope};
use failure::Error;
use serde::Deserialize;

use akashi::store::{SharedStore, Store, StoreBackend};
use akashi::{ComponentManager, ComponentsAttached, Player, Snowflake};

use crate::models::{PlayerModel, ResourceA};
use crate::utils;
use crate::utils::{
    player_not_found, BadTransactionError, ObjectNotFoundError, Pagination, SnowflakeGeneratorState,
};

// GET /players
async fn list_players<T, U>(
    query: web::Query<Pagination>,
    shared_store: web::Data<T>,
    cm: web::Data<ComponentManager>,
) -> Result<HttpResponse, Error>
where
    T: SharedStore<Player, U> + Send + Sync + 'static,
    U: StoreBackend<Player> + Send + Sync + 'static,
{
    let cm = cm.into_inner();
    let players: Vec<PlayerModel> = web::block(move || -> Result<Vec<PlayerModel>, Error> {
        let store: &Store<Player, U> = shared_store.get_store();
        let keys = store.keys(query.page, query.limit)?;

        let vals: Vec<PlayerModel> = keys
            .iter()
            .filter_map(|key| -> Option<PlayerModel> {
                let wrapper = store.load(*key, cm.clone()).ok()?;
                let handle = wrapper.lock().ok()?;

                handle.get().and_then(|pl| PlayerModel::new(pl).ok())
            })
            .collect();
        Ok(vals)
    })
    .await
    .map_err(utils::convert_blocking_err)?;

    Ok(HttpResponse::Ok().json(players))
}

// GET /players/{playerid}
async fn get_player<T, U>(
    path: web::Path<(Snowflake,)>,
    shared_store: web::Data<T>,
    cm: web::Data<ComponentManager>,
) -> Result<HttpResponse, Error>
where
    T: SharedStore<Player, U> + Send + Sync + 'static,
    U: StoreBackend<Player> + Send + Sync + 'static,
{
    let id: Snowflake = path.0;
    let cm = cm.into_inner();

    let r: PlayerModel = web::block(move || -> Result<PlayerModel, Error> {
        let store: &Store<Player, U> = shared_store.get_store();
        let wrapper = store.load(id, cm)?;
        let handle = wrapper
            .lock()
            .map_err(|_e| format_err!("failed to lock wrapper"))?;

        match handle.get() {
            None => Err(player_not_found(id)),
            Some(r) => PlayerModel::new(r),
        }
    })
    .await
    .map_err(utils::convert_blocking_err)?;

    Ok(HttpResponse::Ok().json(r))
}

#[derive(Deserialize)]
#[serde(tag = "op", content = "d")]
enum Transaction {
    Add(i64),
    Sub(i64),
    Set(i64),
    TransferFrom((Snowflake, i64)),
}

// POST /players/{playerid}/resource_a
async fn resource_a_transaction<T, U>(
    path: web::Path<(Snowflake,)>,
    shared_store: web::Data<T>,
    transaction: web::Json<Transaction>,
    cm: web::Data<ComponentManager>,
) -> Result<HttpResponse, Error>
where
    T: SharedStore<Player, U> + Send + Sync + 'static,
    U: StoreBackend<Player> + Send + Sync + 'static,
{
    let player_id = path.0;
    let transaction = transaction.into_inner();
    let cm = cm.into_inner();

    let res = web::block(move || -> Result<PlayerModel, Error> {
        let store: &Store<Player, U> = shared_store.get_store();
        let wrapper = store.load(player_id, cm.clone())?;
        let mut handle = wrapper
            .lock()
            .map_err(|_e| format_err!("failed to lock wrapper"))?;
        let pl = handle
            .get_mut()
            .ok_or_else(|| player_not_found(player_id))?;

        let mut rsc_a: ResourceA = pl.get_component()?.unwrap_or_default();
        match transaction {
            Transaction::Add(val) => rsc_a
                .0
                .checked_add(val.into())
                .map_err(|e| BadTransactionError::new(e.to_string()))?,
            Transaction::Sub(val) => rsc_a
                .0
                .checked_sub(val.into())
                .map_err(|e| BadTransactionError::new(e.to_string()))?,
            Transaction::Set(val) => rsc_a
                .0
                .checked_set(val.into())
                .map_err(|e| BadTransactionError::new(e.to_string()))?,
            Transaction::TransferFrom((from_pl_id, val)) => {
                let other_wrapper = store.load(from_pl_id, cm.clone())?;
                let mut other_handle = other_wrapper
                    .lock()
                    .map_err(|_e| format_err!("failed to lock wrapper"))?;
                let other_pl = other_handle
                    .get_mut()
                    .ok_or_else(|| player_not_found(from_pl_id))?;

                let mut other_rsc_a: ResourceA = other_pl.get_component()?.unwrap_or_default();
                other_rsc_a
                    .0
                    .checked_sub(val.into())
                    .map_err(|e| BadTransactionError::new(e.to_string()))?;
                rsc_a
                    .0
                    .checked_add(val.into())
                    .map_err(|e| BadTransactionError::new(e.to_string()))?;

                other_pl.set_component(other_rsc_a)?;
            }
        };

        pl.set_component(rsc_a)?;
        PlayerModel::new(pl)
    })
    .await
    .map_err(utils::convert_blocking_err)?;

    Ok(HttpResponse::Ok().json(res))
}

// DELETE /players/{playerid}
async fn delete_player<T, U>(
    path: web::Path<(Snowflake,)>,
    shared_store: web::Data<T>,
    cm: web::Data<ComponentManager>,
) -> Result<HttpResponse, Error>
where
    T: SharedStore<Player, U> + Send + Sync + 'static,
    U: StoreBackend<Player> + Send + Sync + 'static,
{
    let id: Snowflake = path.0;
    let cm = cm.into_inner();

    web::block(move || -> Result<(), Error> {
        let store: &Store<Player, U> = shared_store.get_store();
        let wrapper = store.load(id, cm)?;
        let mut handle = wrapper
            .lock()
            .map_err(|_e| format_err!("failed to lock wrapper"))?;

        if !handle.exists() {
            Err(player_not_found(id))
        } else {
            handle.delete()?;
            Ok(())
        }
    })
    .await
    .map_err(utils::convert_blocking_err)?;

    Ok(HttpResponse::NoContent().finish())
}

// POST /players/new
async fn new_player<T, U>(
    shared_store: web::Data<T>,
    sg: SnowflakeGeneratorState,
    cm: web::Data<ComponentManager>,
) -> Result<HttpResponse, Error>
where
    T: SharedStore<Player, U> + Send + Sync + 'static,
    U: StoreBackend<Player> + Send + Sync + 'static,
{
    let cm = cm.into_inner();
    let pl = web::block(move || -> Result<PlayerModel, Error> {
        let new_pl: Player;

        {
            let mut snowflake_gen = sg
                .lock()
                .map_err(|_e| format_err!("snowflake generator lock poisoned"))?;
            new_pl = Player::empty(&mut snowflake_gen, cm.clone());
        }

        let store: &Store<Player, U> = shared_store.get_store();

        let wrapper = store.load(new_pl.id(), cm.clone())?;
        let mut handle = wrapper
            .lock()
            .map_err(|_e| format_err!("failed to lock wrapper"))?;

        handle.replace(new_pl);
        handle.store()?;

        // The unwrap shouldn't fail since we just replaced it with new_pl.
        let model = PlayerModel::new(handle.get().unwrap())?;
        Ok(model)
    })
    .await
    .map_err(utils::convert_blocking_err)?;

    Ok(HttpResponse::Ok().json(pl))
}

pub fn bind_routes<T, U>(
    scope: Scope,
    store: web::Data<T>,
    sg: SnowflakeGeneratorState,
    cm: web::Data<ComponentManager>,
) -> Scope
where
    T: SharedStore<Player, U> + Send + Sync + 'static,
    U: StoreBackend<Player> + Send + Sync + 'static,
{
    scope
        .app_data(store)
        .app_data(sg)
        .app_data(cm)
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
    use std::sync::Arc;

    use crate::utils;
    use crate::utils::{get_body_json, get_body_str, snowflake_generator, store};

    use akashi::local_storage::SharedLocalStore;
    use akashi::SnowflakeGenerator;

    #[test]
    fn test_new_player() {
        let shared_store = store();
        let sg = snowflake_generator(0, 0);
        let cm = web::Data::new(utils::new_component_manager(&shared_store));

        let resp = block_on(new_player(shared_store.clone(), sg, cm)).unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);

        let players = shared_store.players();
        assert_eq!(players.keys(0, 20).unwrap().len(), 1);
    }

    #[test]
    fn test_get_player_exists() {
        let shared_store = SharedLocalStore::new();
        let cm = web::Data::new(utils::new_component_manager(&shared_store));
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);

        let players = shared_store.players();
        let pl = Player::empty(&mut snowflake_gen, cm.clone().into_inner());
        let id = pl.id();
        let model = PlayerModel::new(&pl).unwrap();

        players.store(id, pl, cm.clone().into_inner()).unwrap();

        let resp = block_on(get_player(
            web::Path::from((id,)),
            web::Data::new(shared_store),
            cm,
        ))
        .unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);

        let body: PlayerModel = get_body_json(&resp);
        assert_eq!(model, body);
    }

    #[test]
    fn test_get_player_not_exists() {
        let shared_store = store();
        let cm = web::Data::new(utils::new_component_manager(&shared_store));
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let id = snowflake_gen.generate();

        let resp = block_on(get_player(web::Path::from((id,)), shared_store, cm));
        let _e: ObjectNotFoundError = utils::expect_error(resp);
    }

    #[test]
    fn test_delete_player_exists() {
        let shared_store = store();
        let cm = web::Data::new(utils::new_component_manager(&shared_store));
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);

        let players = shared_store.players();
        let pl = Player::empty(&mut snowflake_gen, cm.clone().into_inner());
        let id = pl.id();
        players
            .store(id, pl.clone(), cm.clone().into_inner())
            .unwrap();

        assert_eq!(players.keys(0, 20).unwrap().len(), 1);

        let resp = block_on(delete_player(
            web::Path::from((id,)),
            shared_store.clone(),
            cm,
        ))
        .unwrap();
        assert_eq!(resp.status(), http::StatusCode::NO_CONTENT);
        assert_eq!(shared_store.players().keys(0, 20).unwrap().len(), 0);
    }

    #[test]
    fn test_delete_player_not_exists() {
        let shared_store = store();
        let cm = web::Data::new(utils::new_component_manager(&shared_store));
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let id = snowflake_gen.generate();

        let resp = block_on(delete_player(web::Path::from((id,)), shared_store, cm));
        let _e: ObjectNotFoundError = utils::expect_error(resp);
    }

    #[test]
    fn test_player_transaction_add() {
        let shared_store = store();
        let cm = web::Data::new(utils::new_component_manager(&shared_store));
        let acm = cm.clone().into_inner();
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let pl = Player::empty(&mut snowflake_gen, acm.clone());

        let players = shared_store.players();
        let id = pl.id();
        players.store(id, pl, acm.clone()).unwrap();

        let resp = block_on(resource_a_transaction(
            web::Path::from((id,)),
            shared_store.clone(),
            web::Json(Transaction::Add(10)),
            cm,
        ))
        .unwrap();

        assert_eq!(resp.status(), http::StatusCode::OK);

        {
            let wrapper = players.load(id, acm).unwrap();
            let handle = wrapper.lock().unwrap();

            let stored_pl = PlayerModel::new(handle.get().unwrap()).unwrap();
            let resp_pl: PlayerModel = get_body_json(&resp);

            assert_eq!(resp_pl, stored_pl);
            assert_eq!(stored_pl.resource_a, 10);
        }
    }

    #[test]
    fn test_player_transaction_sub() {
        let shared_store = store();
        let cm = web::Data::new(utils::new_component_manager(&shared_store));
        let acm = cm.clone().into_inner();
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let mut pl = Player::empty(&mut snowflake_gen, acm.clone());

        pl.set_component::<ResourceA>(50.into()).unwrap();

        let players = shared_store.players();
        let id = pl.id();
        players.store(id, pl.clone(), acm.clone()).unwrap();

        let resp = block_on(resource_a_transaction(
            web::Path::from((id,)),
            shared_store.clone(),
            web::Json(Transaction::Sub(25)),
            cm,
        ))
        .unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);

        {
            let wrapper = players.load(id, acm).unwrap();
            let handle = wrapper.lock().unwrap();

            let stored_pl = PlayerModel::new(handle.get().unwrap()).unwrap();
            let resp_pl: PlayerModel = get_body_json(&resp);

            assert_eq!(resp_pl, stored_pl);
            assert_eq!(stored_pl.resource_a, 25);
        }
    }

    #[test]
    fn test_player_transaction_sub_validate() {
        let shared_store = store();
        let cm = web::Data::new(utils::new_component_manager(&shared_store));
        let acm = cm.clone().into_inner();
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let mut pl = Player::empty(&mut snowflake_gen, acm.clone());

        pl.set_component::<ResourceA>(50.into()).unwrap();
        let expected_player = PlayerModel::new(&pl).unwrap();

        let players = shared_store.players();
        let id = pl.id();
        players.store(id, pl, acm.clone()).unwrap();

        let resp = block_on(resource_a_transaction(
            web::Path::from((id,)),
            shared_store.clone(),
            web::Json(Transaction::Sub(60)),
            cm,
        ));
        let _e: BadTransactionError = utils::expect_error(resp);

        {
            let wrapper = players.load(id, acm).unwrap();
            let handle = wrapper.lock().unwrap();
            let stored_pl = PlayerModel::new(handle.get().unwrap()).unwrap();

            assert_eq!(stored_pl, expected_player);
            assert_eq!(stored_pl.resource_a, 50);
        }
    }

    #[test]
    fn test_player_transaction_set() {
        let shared_store = store();
        let cm = web::Data::new(utils::new_component_manager(&shared_store));
        let acm = cm.clone().into_inner();
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let pl = Player::empty(&mut snowflake_gen, acm.clone());

        let players = shared_store.players();
        let id = pl.id();
        players.store(id, pl, acm.clone()).unwrap();

        let resp = block_on(resource_a_transaction(
            web::Path::from((id,)),
            shared_store.clone(),
            web::Json(Transaction::Set(100)),
            cm,
        ))
        .unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);

        {
            let wrapper = players.load(id, acm).unwrap();
            let handle = wrapper.lock().unwrap();

            let stored_pl = PlayerModel::new(handle.get().unwrap()).unwrap();
            let resp_pl: PlayerModel = get_body_json(&resp);

            assert_eq!(resp_pl, stored_pl);
            assert_eq!(stored_pl.resource_a, 100);
        }
    }

    #[test]
    fn test_player_transaction_transfer() {
        let shared_store = store();
        let cm = web::Data::new(utils::new_component_manager(&shared_store));
        let acm = cm.clone().into_inner();
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let mut pl_1 = Player::empty(&mut snowflake_gen, acm.clone());
        let mut pl_2 = Player::empty(&mut snowflake_gen, acm.clone());

        pl_1.set_component::<ResourceA>(110.into()).unwrap();
        pl_2.set_component::<ResourceA>(0.into()).unwrap();

        let players = shared_store.players();
        let id_1 = pl_1.id();
        let id_2 = pl_2.id();

        players.store(id_1, pl_1.clone(), acm.clone()).unwrap();
        players.store(id_2, pl_2.clone(), acm.clone()).unwrap();

        let resp = block_on(resource_a_transaction(
            web::Path::from((id_2,)),
            shared_store.clone(),
            web::Json(Transaction::TransferFrom((id_1, 50))),
            cm.clone(),
        ))
        .unwrap();

        assert_eq!(resp.status(), http::StatusCode::OK);

        {
            let wrapper = players.load(id_2, acm.clone()).unwrap();
            let handle = wrapper.lock().unwrap();

            let stored_pl = PlayerModel::new(handle.get().unwrap()).unwrap();
            let resp_pl: PlayerModel = get_body_json(&resp);

            assert_eq!(resp_pl, stored_pl);
            assert_eq!(stored_pl.resource_a, 50);
        }

        {
            let wrapper = players.load(id_1, acm.clone()).unwrap();
            let handle = wrapper.lock().unwrap();
            let stored_pl = PlayerModel::new(handle.get().unwrap()).unwrap();

            assert_eq!(stored_pl.resource_a, 60);
        }
    }

    #[test]
    fn test_player_transaction_transfer_validate() {
        let shared_store = store();
        let cm = web::Data::new(utils::new_component_manager(&shared_store));
        let acm = cm.clone().into_inner();
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let mut pl_1 = Player::empty(&mut snowflake_gen, acm.clone());
        let mut pl_2 = Player::empty(&mut snowflake_gen, acm.clone());

        pl_1.set_component::<ResourceA>(50.into()).unwrap();
        pl_2.set_component::<ResourceA>(0.into()).unwrap();

        let players = shared_store.players();
        let id_1 = pl_1.id();
        let id_2 = pl_2.id();

        let model_1 = PlayerModel::new(&pl_1).unwrap();
        let model_2 = PlayerModel::new(&pl_2).unwrap();

        players.store(id_1, pl_1.clone(), acm.clone()).unwrap();
        players.store(id_2, pl_2.clone(), acm.clone()).unwrap();

        let resp = block_on(resource_a_transaction(
            web::Path::from((id_2,)),
            shared_store.clone(),
            web::Json(Transaction::TransferFrom((id_1, 60))),
            cm.clone(),
        ));
        let _e: BadTransactionError = utils::expect_error(resp);

        {
            let wrapper = players.load(id_1, acm.clone()).unwrap();
            let handle = wrapper.lock().unwrap();
            let stored_pl = PlayerModel::new(handle.get().unwrap()).unwrap();

            assert_eq!(stored_pl, model_1);
        }

        {
            let wrapper = players.load(id_2, acm.clone()).unwrap();
            let handle = wrapper.lock().unwrap();
            let stored_pl = PlayerModel::new(handle.get().unwrap()).unwrap();

            assert_eq!(stored_pl, model_2);
        }
    }

    #[test]
    fn test_list_players_empty() {
        let shared_store = store();
        let cm = web::Data::new(utils::new_component_manager(&shared_store));
        let query = web::Query::<Pagination>::from_query("?page=0&limit=20").unwrap();

        let resp = block_on(list_players(query, shared_store, cm)).unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);

        let body = get_body_str(&resp);
        assert_eq!(body, "[]");
    }

    #[test]
    fn test_list_players_nonempty() {
        let shared_store = store();
        let cm = web::Data::new(utils::new_component_manager(&shared_store));
        let acm = cm.clone().into_inner();
        let query = web::Query::<Pagination>::from_query("?page=0&limit=20").unwrap();
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);

        let players = shared_store.players();
        let pl = Player::empty(&mut snowflake_gen, acm.clone());
        let id = pl.id();
        let model = PlayerModel::new(&pl).unwrap();

        players.store(id, pl, acm.clone()).unwrap();

        let resp = block_on(list_players(query, shared_store, cm)).unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);

        let body: Vec<PlayerModel> = get_body_json(&resp);
        assert_eq!(body, vec![model]);
    }
}
