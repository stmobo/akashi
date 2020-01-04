// Example players API built on top of Akashi.

use actix_web::{web, HttpResponse, Scope};
use failure::Error;
use serde::Deserialize;

use akashi::{Player, Snowflake, ComponentManager, ComponentsAttached};
use akashi::store::{SharedStore, Store, StoreBackend};

use crate::utils;
use crate::utils::{ObjectNotFoundError, BadTransactionError, Pagination, SnowflakeGeneratorState, player_not_found};
use crate::models::{PlayerModel, ResourceA};

// GET /players
async fn list_players<T, U>(
    query: web::Query<Pagination>,
    shared_store: web::Data<T>,
) -> Result<HttpResponse, Error>
where
    T: SharedStore<Player, U> + Send + Sync + 'static,
    U: StoreBackend<Player> + Send + Sync + 'static,
{
    let players: Vec<PlayerModel> = web::block(move || -> Result<Vec<PlayerModel>, Error> {
        let store: &Store<Player, U> = shared_store.get_store();
        let keys = store.keys(query.page, query.limit)?;

        let vals: Vec<PlayerModel> = keys
            .iter()
            .filter_map(|key| -> Option<PlayerModel> {
                let wrapper = store.load(*key).ok()?;
                let handle = wrapper.lock().ok()?;

                handle.get().and_then(|pl| PlayerModel::new(pl).ok())
            })
            .collect();
        Ok(vals)
    })
    .await.map_err(utils::convert_blocking_err)?;

    Ok(HttpResponse::Ok().json(players))
}

// GET /players/{playerid}
async fn get_player<T, U>(
    path: web::Path<(Snowflake,)>,
    shared_store: web::Data<T>,
) -> Result<HttpResponse, Error>
where
    T: SharedStore<Player, U> + Send + Sync + 'static,
    U: StoreBackend<Player> + Send + Sync + 'static,
{
    let id: Snowflake = path.0;

    let r: PlayerModel = web::block(move || -> Result<PlayerModel, Error> {
        let store: &Store<Player, U> = shared_store.get_store();
        let wrapper = store.load(id)?;
        let handle = wrapper.lock().map_err(|_e| format_err!("failed to lock wrapper"))?;

        match handle.get() {
            None => Err(player_not_found(id)),
            Some(r) => PlayerModel::new(r),
        }
    })
    .await.map_err(utils::convert_blocking_err)?;

    Ok(HttpResponse::Ok().json(r))
}

#[derive(Deserialize)]
#[serde(tag = "op", content = "d")]
enum Transaction {
    Add(u64),
    Sub(u64),
    Set(u64),
    TransferFrom((Snowflake, u64)),
}

// POST /players/{playerid}/resources
async fn player_resource_transaction<T, U>(
    path: web::Path<(Snowflake,)>,
    shared_store: web::Data<T>,
    transactions: web::Json<Vec<Transaction>>,
) -> Result<HttpResponse, Error>
where
    T: SharedStore<Player, U> + Send + Sync + 'static,
    U: StoreBackend<Player> + Send + Sync + 'static,
{
    Ok(HttpResponse::NotImplemented().finish())
}

// DELETE /players/{playerid}
async fn delete_player<T, U>(
    path: web::Path<(Snowflake,)>,
    shared_store: web::Data<T>,
) -> Result<HttpResponse, Error>
where
    T: SharedStore<Player, U> + Send + Sync + 'static,
    U: StoreBackend<Player> + Send + Sync + 'static,
{
    let id: Snowflake = path.0;

    web::block(move || -> Result<(), Error> {
        let store: &Store<Player, U> = shared_store.get_store();
        let wrapper = store.load(id)?;
        let mut handle = wrapper.lock().map_err(|_e| format_err!("failed to lock wrapper"))?;

        if !handle.exists() {
            Err(player_not_found(id))
        } else {
            handle.delete()?;
            Ok(())
        }
    })
    .await.map_err(utils::convert_blocking_err)?;

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
    let pl = web::block(move || -> Result<PlayerModel, Error> {
        let new_pl: Player;

        {
            let mut snowflake_gen = sg.lock().map_err(|_e| format_err!("snowflake generator lock poisoned"))?;
            new_pl = Player::empty(&mut snowflake_gen, cm.into_inner());
        }

        let store: &Store<Player, U> = shared_store.get_store();

        let wrapper = store.load(new_pl.id())?;
        let mut handle = wrapper.lock().map_err(|_e| format_err!("failed to lock wrapper"))?;

        handle.replace(new_pl);
        handle.store()?;

        // The unwrap shouldn't fail since we just replaced it with new_pl.
        let model = PlayerModel::new(handle.get().unwrap())?;
        Ok(model)
    })
    .await.map_err(utils::convert_blocking_err)?;

    Ok(HttpResponse::Ok().json(pl))
}

pub fn bind_routes<T, U>(scope: Scope, store: web::Data<T>, sg: SnowflakeGeneratorState, cm: web::Data<ComponentManager>) -> Scope
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
    use std::sync::Arc;
    use actix_web::http;
    use futures::executor::block_on;

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
        let cm = utils::new_component_manager(&shared_store);
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);

        let players = shared_store.players();
        let pl = Player::empty(&mut snowflake_gen, Arc::new(cm));
        let id = pl.id();
        let model = PlayerModel::new(&pl).unwrap();

        players.store(id, pl).unwrap();

        let resp = block_on(get_player(
            web::Path::from((id,)),
            web::Data::new(shared_store),
        ))
        .unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);

        let body: PlayerModel = get_body_json(&resp);
        assert_eq!(model, body);
    }

    #[test]
    fn test_get_player_not_exists() {
        let shared_store = store();
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let id = snowflake_gen.generate();

        let resp = block_on(get_player(web::Path::from((id,)), shared_store));
        let _e: ObjectNotFoundError = utils::expect_error(resp);
    }

    #[test]
    fn test_delete_player_exists() {
        let shared_store = store();
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);

        let players = shared_store.players();
        let pl = Player::empty(&mut snowflake_gen, Arc::new(utils::new_component_manager(&shared_store)));
        let id = pl.id();
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
        let _e: ObjectNotFoundError = utils::expect_error(resp);
    }

    #[test]
    fn test_player_transaction_add() {
        let shared_store = store();
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let pl = Player::empty(&mut snowflake_gen, Arc::new(utils::new_component_manager(&shared_store)));

        let players = shared_store.players();
        let id = pl.id();
        players.store(id, pl).unwrap();

        let resp = block_on(player_resource_transaction(
            web::Path::from((id,)),
            shared_store.clone(),
            web::Json(vec![Transaction::Add(10)]),
        ))
        .unwrap();

        assert_eq!(resp.status(), http::StatusCode::OK);

        {
            let wrapper = players.load(id).unwrap();
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
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let pl = Player::empty(&mut snowflake_gen, Arc::new(utils::new_component_manager(&shared_store)));

        pl.set_component::<ResourceA>(50.into()).unwrap();

        let players = shared_store.players();
        let id = pl.id();
        players.store(id, pl.clone()).unwrap();

        let resp = block_on(player_resource_transaction(
            web::Path::from((id,)),
            shared_store.clone(),
            web::Json(vec![Transaction::Sub(25)]),
        ))
        .unwrap();

        assert_eq!(resp.status(), http::StatusCode::OK);

        {
            let wrapper = players.load(id).unwrap();
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
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let pl = Player::empty(&mut snowflake_gen, Arc::new(utils::new_component_manager(&shared_store)));

        pl.set_component::<ResourceA>(50.into()).unwrap();
        let expected_player = PlayerModel::new(&pl).unwrap(); 

        let players = shared_store.players();
        let id = pl.id();
        players.store(id, pl).unwrap();

        let resp = block_on(player_resource_transaction(
            web::Path::from((id,)),
            shared_store.clone(),
            web::Json(vec![Transaction::Sub(60)]),
        ));
        let _e: BadTransactionError = utils::expect_error(resp);

        {
            let wrapper = players.load(id).unwrap();
            let handle = wrapper.lock().unwrap();
            let stored_pl = PlayerModel::new(handle.get().unwrap()).unwrap(); 

            assert_eq!(stored_pl, expected_player);
            assert_eq!(stored_pl.resource_a, 50);
        }
    }

    #[test]
    fn test_player_transaction_set() {
        let shared_store = store();
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let pl = Player::empty(&mut snowflake_gen, Arc::new(utils::new_component_manager(&shared_store)));

        let players = shared_store.players();
        let id = pl.id();
        players.store(id, pl).unwrap();

        let resp = block_on(player_resource_transaction(
            web::Path::from((id,)),
            shared_store.clone(),
            web::Json(vec![Transaction::Set(100)]),
        ))
        .unwrap();

        assert_eq!(resp.status(), http::StatusCode::OK);

        {
            let wrapper = players.load(id).unwrap();
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
        let cm = Arc::new(utils::new_component_manager(&shared_store));
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let pl_1 = Player::empty(&mut snowflake_gen, cm.clone());
        let pl_2 = Player::empty(&mut snowflake_gen, cm);

        pl_1.set_component::<ResourceA>(110.into()).unwrap();
        pl_2.set_component::<ResourceA>(0.into()).unwrap();

        let players = shared_store.players();
        let id_1 = pl_1.id();
        let id_2 = pl_2.id();

        players.store(id_1, pl_1.clone()).unwrap();
        players.store(id_2, pl_2.clone()).unwrap();

        let resp = block_on(player_resource_transaction(
            web::Path::from((id_2,)),
            shared_store.clone(),
            web::Json(vec![Transaction::TransferFrom((id_1, 50))]),
        ))
        .unwrap();

        assert_eq!(resp.status(), http::StatusCode::OK);

        {
            let wrapper = players.load(id_2).unwrap();
            let handle = wrapper.lock().unwrap();

            let stored_pl = PlayerModel::new(handle.get().unwrap()).unwrap(); 
            let resp_pl: PlayerModel = get_body_json(&resp);

            assert_eq!(resp_pl, stored_pl);
            assert_eq!(stored_pl.resource_a, 50);
        }

        {
            let wrapper = players.load(id_1).unwrap();
            let handle = wrapper.lock().unwrap();
            let stored_pl = PlayerModel::new(handle.get().unwrap()).unwrap(); 

            assert_eq!(stored_pl.resource_a, 60);
        }
    }

    #[test]
    fn test_player_transaction_transfer_validate() {
        let shared_store = store();
        let cm = Arc::new(utils::new_component_manager(&shared_store));
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let pl_1 = Player::empty(&mut snowflake_gen, cm.clone());
        let pl_2 = Player::empty(&mut snowflake_gen, cm);

        pl_1.set_component::<ResourceA>(50.into()).unwrap();
        pl_2.set_component::<ResourceA>(0.into()).unwrap();

        let players = shared_store.players();
        let id_1 = pl_1.id();
        let id_2 = pl_2.id();

        let model_1 = PlayerModel::new(&pl_1).unwrap();
        let model_2 = PlayerModel::new(&pl_2).unwrap();

        players.store(id_1, pl_1.clone()).unwrap();
        players.store(id_2, pl_2.clone()).unwrap();

        let resp = block_on(player_resource_transaction(
            web::Path::from((id_2,)),
            shared_store.clone(),
            web::Json(vec![Transaction::TransferFrom((id_1, 60))]),
        ));
        let _e: BadTransactionError = utils::expect_error(resp);

        {
            let wrapper = players.load(id_1).unwrap();
            let handle = wrapper.lock().unwrap();
            let stored_pl = PlayerModel::new(handle.get().unwrap()).unwrap(); 

            assert_eq!(stored_pl, model_1);
        }

        {
            let wrapper = players.load(id_2).unwrap();
            let handle = wrapper.lock().unwrap();
            let stored_pl = PlayerModel::new(handle.get().unwrap()).unwrap(); 

            assert_eq!(stored_pl, model_2);
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
        let cm = Arc::new(utils::new_component_manager(&shared_store));
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);

        let players = shared_store.players();
        let pl = Player::empty(&mut snowflake_gen, cm);
        let id = pl.id();
        let model = PlayerModel::new(&pl).unwrap();

        players.store(id, pl).unwrap();

        let resp = block_on(list_players(query, shared_store)).unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);

        let body: Vec<PlayerModel> = get_body_json(&resp);
        assert_eq!(body, vec![model]);
    }
}
