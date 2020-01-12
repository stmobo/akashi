// Example players API built on top of Akashi.

use actix_web::{web, HttpResponse, Scope};
use failure::Error;
use serde::Deserialize;

use akashi::{Entity, EntityManager, Player, Snowflake};

use crate::models::{PlayerModel, ResourceA};
use crate::utils;
use crate::utils::{player_not_found, BadTransactionError, Pagination, SnowflakeGeneratorState};

// GET /players
async fn list_players(
    query: web::Query<Pagination>,
    entity_manager: web::Data<EntityManager>,
) -> Result<HttpResponse, Error> {
    let players: Vec<PlayerModel> = web::block(move || -> Result<Vec<PlayerModel>, Error> {
        let keys = entity_manager.keys::<Player>(query.page, query.limit)?;

        let vals: Vec<PlayerModel> = keys
            .iter()
            .filter_map(|key| -> Option<PlayerModel> {
                let handle = entity_manager.load::<Player>(*key).ok()?;

                handle
                    .get()
                    .and_then(|pl| PlayerModel::new(pl, &*entity_manager).ok())
            })
            .collect();
        Ok(vals)
    })
    .await
    .map_err(utils::convert_blocking_err)?;

    Ok(HttpResponse::Ok().json(players))
}

// GET /players/{playerid}
async fn get_player(
    path: web::Path<(Snowflake,)>,
    entity_manager: web::Data<EntityManager>,
) -> Result<HttpResponse, Error> {
    let id: Snowflake = path.0;
    let r: PlayerModel = web::block(move || -> Result<PlayerModel, Error> {
        let handle = entity_manager.load::<Player>(id)?;

        match handle.get() {
            None => Err(player_not_found(id)),
            Some(r) => PlayerModel::new(r, &*entity_manager),
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
async fn resource_a_transaction(
    path: web::Path<(Snowflake,)>,
    transaction: web::Json<Transaction>,
    entity_manager: web::Data<EntityManager>,
) -> Result<HttpResponse, Error> {
    let player_id = path.0;
    let transaction = transaction.into_inner();

    let res = web::block(move || -> Result<PlayerModel, Error> {
        let mut handle = entity_manager.load_mut::<Player>(player_id)?;
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
                let mut other_handle = entity_manager.load_mut::<Player>(from_pl_id)?;
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
                other_handle.store()?;
            }
        };

        pl.set_component(rsc_a)?;
        let ret = PlayerModel::new(pl, &*entity_manager);
        handle.store()?;

        ret
    })
    .await
    .map_err(utils::convert_blocking_err)?;

    Ok(HttpResponse::Ok().json(res))
}

// DELETE /players/{playerid}
async fn delete_player(
    path: web::Path<(Snowflake,)>,
    entity_manager: web::Data<EntityManager>,
) -> Result<HttpResponse, Error> {
    let id: Snowflake = path.0;

    web::block(move || -> Result<(), Error> {
        let mut handle = entity_manager.load_mut::<Player>(id)?;

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
async fn new_player(
    entity_manager: web::Data<EntityManager>,
    sg: SnowflakeGeneratorState,
) -> Result<HttpResponse, Error> {
    let pl = web::block(move || -> Result<PlayerModel, Error> {
        let mut snowflake_gen = sg
            .lock()
            .map_err(|_e| format_err!("snowflake generator lock poisoned"))?;
        let new_pl: Player = entity_manager.create(snowflake_gen.generate()).unwrap();
        drop(snowflake_gen);

        let mut handle = entity_manager.load_mut::<Player>(new_pl.id())?;

        handle.replace(new_pl);
        handle.store()?;

        // The unwrap shouldn't fail since we just replaced it with new_pl.
        let model = PlayerModel::new(handle.get().unwrap(), &*entity_manager)?;
        Ok(model)
    })
    .await
    .map_err(utils::convert_blocking_err)?;

    Ok(HttpResponse::Ok().json(pl))
}

pub fn bind_routes(
    scope: Scope,
    entity_manager: web::Data<EntityManager>,
    sg: SnowflakeGeneratorState,
) -> Scope {
    scope
        .app_data(entity_manager)
        .app_data(sg)
        .route("/{playerid}", web::get().to(get_player))
        .route("/{playerid}", web::delete().to(delete_player))
        .route(
            "/{playerid}/resource_a",
            web::post().to(resource_a_transaction),
        )
        .route("/new", web::post().to(new_player))
        .route("", web::get().to(list_players))
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::http;
    use futures::executor::block_on;

    use crate::utils;
    use crate::utils::{
        create_new_player, get_body_json, get_body_str, snowflake_generator, ObjectNotFoundError,
    };

    use akashi::SnowflakeGenerator;

    #[test]
    fn test_new_player() {
        let sg = snowflake_generator(0, 0);
        let em = web::Data::new(utils::setup_entity_manager());

        let resp = block_on(new_player(em.clone(), sg)).unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);

        let players = em.keys::<Player>(0, 20).unwrap();
        assert_eq!(players.len(), 1);
    }

    #[test]
    fn test_get_player_exists() {
        let em = web::Data::new(utils::setup_entity_manager());
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);

        let (id, pl) = create_new_player(&*em, &mut snowflake_gen);
        let model = PlayerModel::new(&pl, &*em).unwrap();

        em.store(pl).unwrap();

        let resp = block_on(get_player(web::Path::from((id,)), em)).unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);

        let body: PlayerModel = get_body_json(&resp);
        assert_eq!(model, body);
    }

    #[test]
    fn test_get_player_not_exists() {
        let em = web::Data::new(utils::setup_entity_manager());
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let id = snowflake_gen.generate();

        let resp = block_on(get_player(web::Path::from((id,)), em));
        let _e: ObjectNotFoundError = utils::expect_error(resp);
    }

    #[test]
    fn test_delete_player_exists() {
        let em = web::Data::new(utils::setup_entity_manager());
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);

        let (id, mut pl) = create_new_player(&*em, &mut snowflake_gen);

        let rsc_a: ResourceA = 25.into();
        pl.set_component(rsc_a).unwrap();

        em.store(pl.clone()).unwrap();

        assert_eq!(em.keys::<Player>(0, 20).unwrap().len(), 1);

        let resp = block_on(delete_player(web::Path::from((id,)), em.clone())).unwrap();
        assert_eq!(resp.status(), http::StatusCode::NO_CONTENT);
        assert_eq!(em.keys::<Player>(0, 20).unwrap().len(), 0);

        let cm = em.get_component_manager::<Player>().unwrap();
        let rsc_a: Option<ResourceA> = cm.get_component(&pl).unwrap();
        assert!(rsc_a.is_none());
    }

    #[test]
    fn test_delete_player_not_exists() {
        let em = web::Data::new(utils::setup_entity_manager());
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let id = snowflake_gen.generate();

        let resp = block_on(delete_player(web::Path::from((id,)), em));
        let _e: ObjectNotFoundError = utils::expect_error(resp);
    }

    #[test]
    fn test_player_transaction_add() {
        let em = web::Data::new(utils::setup_entity_manager());
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let (id, pl) = create_new_player(&*em, &mut snowflake_gen);

        em.store(pl).unwrap();

        let resp = block_on(resource_a_transaction(
            web::Path::from((id,)),
            web::Json(Transaction::Add(10)),
            em.clone(),
        ))
        .unwrap();

        assert_eq!(resp.status(), http::StatusCode::OK);

        let handle = em.load::<Player>(id).unwrap();

        let stored_pl = PlayerModel::new(handle.get().unwrap(), &*em).unwrap();
        let resp_pl: PlayerModel = get_body_json(&resp);

        assert_eq!(resp_pl, stored_pl);
        assert_eq!(stored_pl.resource_a, 10);
    }

    #[test]
    fn test_player_transaction_sub() {
        let em = web::Data::new(utils::setup_entity_manager());
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let (id, mut pl) = create_new_player(&*em, &mut snowflake_gen);

        pl.set_component::<ResourceA>(50.into()).unwrap();

        em.store(pl).unwrap();

        let resp = block_on(resource_a_transaction(
            web::Path::from((id,)),
            web::Json(Transaction::Sub(25)),
            em.clone(),
        ))
        .unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);

        let handle = em.load::<Player>(id).unwrap();

        let stored_pl = PlayerModel::new(handle.get().unwrap(), &*em).unwrap();
        let resp_pl: PlayerModel = get_body_json(&resp);

        assert_eq!(resp_pl, stored_pl);
        assert_eq!(stored_pl.resource_a, 25);
    }

    #[test]
    fn test_player_transaction_sub_validate() {
        let em = web::Data::new(utils::setup_entity_manager());
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let (id, mut pl) = create_new_player(&*em, &mut snowflake_gen);

        pl.set_component::<ResourceA>(50.into()).unwrap();
        let expected_player = PlayerModel::new(&pl, &*em).unwrap();

        em.store(pl).unwrap();

        let resp = block_on(resource_a_transaction(
            web::Path::from((id,)),
            web::Json(Transaction::Sub(60)),
            em.clone(),
        ));
        let _e: BadTransactionError = utils::expect_error(resp);

        let handle = em.load::<Player>(id).unwrap();
        let stored_pl = PlayerModel::new(handle.get().unwrap(), &*em).unwrap();

        assert_eq!(stored_pl, expected_player);
        assert_eq!(stored_pl.resource_a, 50);
    }

    #[test]
    fn test_player_transaction_set() {
        let em = web::Data::new(utils::setup_entity_manager());
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let (id, pl) = create_new_player(&*em, &mut snowflake_gen);

        em.store(pl).unwrap();

        let resp = block_on(resource_a_transaction(
            web::Path::from((id,)),
            web::Json(Transaction::Set(100)),
            em.clone(),
        ))
        .unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);

        let handle = em.load::<Player>(id).unwrap();

        let stored_pl = PlayerModel::new(handle.get().unwrap(), &*em).unwrap();
        let resp_pl: PlayerModel = get_body_json(&resp);

        assert_eq!(resp_pl, stored_pl);
        assert_eq!(stored_pl.resource_a, 100);
    }

    #[test]
    fn test_player_transaction_transfer() {
        let em = web::Data::new(utils::setup_entity_manager());
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let (id_1, mut pl_1) = create_new_player(&*em, &mut snowflake_gen);
        let (id_2, mut pl_2) = create_new_player(&*em, &mut snowflake_gen);

        pl_1.set_component::<ResourceA>(110.into()).unwrap();
        pl_2.set_component::<ResourceA>(0.into()).unwrap();

        em.store(pl_1).unwrap();
        em.store(pl_2).unwrap();

        let resp = block_on(resource_a_transaction(
            web::Path::from((id_2,)),
            web::Json(Transaction::TransferFrom((id_1, 50))),
            em.clone(),
        ))
        .unwrap();

        assert_eq!(resp.status(), http::StatusCode::OK);

        {
            let handle = em.load::<Player>(id_2).unwrap();

            let stored_pl = PlayerModel::new(handle.get().unwrap(), &*em).unwrap();
            let resp_pl: PlayerModel = get_body_json(&resp);

            assert_eq!(resp_pl, stored_pl);
            assert_eq!(stored_pl.resource_a, 50);
        }

        {
            let handle = em.load::<Player>(id_1).unwrap();
            let stored_pl = PlayerModel::new(handle.get().unwrap(), &*em).unwrap();

            assert_eq!(stored_pl.resource_a, 60);
        }
    }

    #[test]
    fn test_player_transaction_transfer_validate() {
        let em = web::Data::new(utils::setup_entity_manager());
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let (id_1, mut pl_1) = create_new_player(&*em, &mut snowflake_gen);
        let (id_2, mut pl_2) = create_new_player(&*em, &mut snowflake_gen);

        pl_1.set_component::<ResourceA>(50.into()).unwrap();
        pl_2.set_component::<ResourceA>(0.into()).unwrap();

        let model_1 = PlayerModel::new(&pl_1, &*em).unwrap();
        let model_2 = PlayerModel::new(&pl_2, &*em).unwrap();

        em.store(pl_1).unwrap();
        em.store(pl_2).unwrap();

        let resp = block_on(resource_a_transaction(
            web::Path::from((id_2,)),
            web::Json(Transaction::TransferFrom((id_1, 60))),
            em.clone(),
        ));
        let _e: BadTransactionError = utils::expect_error(resp);

        {
            let handle = em.load::<Player>(id_1).unwrap();
            let stored_pl = PlayerModel::new(handle.get().unwrap(), &*em).unwrap();

            assert_eq!(stored_pl, model_1);
        }

        {
            let handle = em.load::<Player>(id_2).unwrap();
            let stored_pl = PlayerModel::new(handle.get().unwrap(), &*em).unwrap();

            assert_eq!(stored_pl, model_2);
        }
    }

    #[test]
    fn test_list_players_empty() {
        let em = web::Data::new(utils::setup_entity_manager());
        let query = web::Query::<Pagination>::from_query("?page=0&limit=20").unwrap();

        let resp = block_on(list_players(query, em)).unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);

        let body = get_body_str(&resp);
        assert_eq!(body, "[]");
    }

    #[test]
    fn test_list_players_nonempty() {
        let em = web::Data::new(utils::setup_entity_manager());
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let (_id, pl) = create_new_player(&*em, &mut snowflake_gen);

        let query = web::Query::<Pagination>::from_query("?page=0&limit=20").unwrap();
        let model = PlayerModel::new(&pl, &*em).unwrap();

        em.store(pl).unwrap();

        let resp = block_on(list_players(query, em)).unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);

        let body: Vec<PlayerModel> = get_body_json(&resp);
        assert_eq!(body, vec![model]);
    }
}
