use failure::Error;
use serde::Deserialize;

use actix_web::{web, HttpResponse, Scope};

use akashi::components::inventory::InventoryWriteRental;
use akashi::components::Inventory;
use akashi::{Card, Entity, EntityManager, Player, Snowflake};

use crate::models::{CardModel, CardName, CardType, CardValue};
use crate::utils;
use crate::utils::SnowflakeGeneratorState;

type Result<T> = std::result::Result<T, Error>;

// GET /inventories/{playerid}
async fn get_inventory(
    path: web::Path<(Snowflake,)>,
    entity_manager: web::Data<EntityManager>,
) -> Result<HttpResponse> {
    let id: Snowflake = path.0;

    let val = web::block(move || -> Result<Vec<CardModel>> {
        let handle = entity_manager.load(id)?;

        let pl_ref: &Player = handle.get().ok_or_else(|| utils::player_not_found(id))?;
        let inv: Option<Inventory> = pl_ref.get_component()?;

        Ok(match inv {
            None => vec![],
            Some(mut v) => {
                let mut model: Vec<CardModel> = Vec::new();
                model.reserve(v.len());
                let ids: Vec<Snowflake> = v.iter_ids().copied().collect();

                for id in ids {
                    let card = v
                        .get(id, &*entity_manager)
                        .ok_or_else(|| format_err!("could not find card with ID {}", id))?;

                    model.push(CardModel::new(card)?);
                }

                model
            }
        })
    })
    .await
    .map_err(utils::convert_blocking_err)?;

    Ok(HttpResponse::Ok().json(val))
}

#[derive(Deserialize)]
struct NewCardOptions {
    card_type: CardType,
    name: String,
    value: f64,
}

#[derive(Deserialize)]
#[serde(tag = "from", content = "options")]
enum InventoryAddOptions {
    Existing(CardModel),
    New(NewCardOptions),
}

// POST /inventories/{playerid}
async fn add_to_inventory(
    path: web::Path<(Snowflake,)>,
    opts: web::Json<InventoryAddOptions>,
    sg: SnowflakeGeneratorState,
    entity_manager: web::Data<EntityManager>,
) -> Result<HttpResponse> {
    let pl_id = path.0;
    let opts = opts.into_inner();

    let res = web::block(move || -> Result<Vec<CardModel>> {
        let mut handle = entity_manager.load_mut(pl_id)?;
        let player: &mut Player = handle
            .get_mut()
            .ok_or_else(|| utils::player_not_found(pl_id))?;

        let mut inv: Inventory = player
            .get_component()?
            .unwrap_or_else(|| Inventory::empty());

        let new_card: Card = match opts {
            InventoryAddOptions::Existing(c) => c.as_card(&*entity_manager)?,
            InventoryAddOptions::New(card_opts) => {
                let mut snowflake_gen = sg
                    .lock()
                    .map_err(|_e| format_err!("snowflake generator lock poisoned"))?;
                let mut c: Card = entity_manager.create(snowflake_gen.generate()).unwrap();

                drop(snowflake_gen);

                c.set_component(card_opts.card_type)?;
                c.set_component(CardName::new(card_opts.name))?;
                c.set_component(CardValue::new(card_opts.value))?;

                entity_manager.store(c.clone())?;
                c
            }
        };

        let mut model: Vec<CardModel> = Vec::new();
        model.reserve(inv.len() + 1);

        let ids: Vec<Snowflake> = inv.iter_ids().copied().collect();
        for id in ids {
            let card = inv
                .get(id, &*entity_manager)
                .ok_or_else(|| format_err!("could not find card with ID {}", id))?;

            model.push(CardModel::new(card)?);
        }

        model.push(CardModel::new(&new_card)?);
        inv.insert(new_card, &*entity_manager)?;

        player.set_component(inv)?;
        handle.store()?;

        Ok(model)
    })
    .await
    .map_err(utils::convert_blocking_err)?;

    Ok(HttpResponse::Ok().json(res))
}

// GET /inventories/{playerid}/{cardid}
async fn get_card(
    path: web::Path<(Snowflake, Snowflake)>,
    entity_manager: web::Data<EntityManager>,
) -> Result<HttpResponse> {
    let pl_id = path.0;
    let card_id = path.1;

    let res: CardModel = web::block(move || -> Result<CardModel> {
        let handle = entity_manager.load(pl_id)?;
        let player: &Player = handle.get().ok_or_else(|| utils::player_not_found(pl_id))?;

        let mut inv: Inventory = player
            .get_component()?
            .ok_or_else(|| utils::card_not_found(card_id))?;

        let card: &Card = inv
            .get(card_id, &*entity_manager)
            .ok_or_else(|| utils::card_not_found(card_id))?;

        CardModel::new(card)
    })
    .await
    .map_err(utils::convert_blocking_err)?;

    Ok(HttpResponse::Ok().json(res))
}

// DELETE /inventories/{playerid}/{cardid}
async fn delete_card(
    path: web::Path<(Snowflake, Snowflake)>,
    entity_manager: web::Data<EntityManager>,
) -> Result<HttpResponse> {
    let pl_id = path.0;
    let card_id = path.1;

    let res: CardModel = web::block(move || -> Result<CardModel> {
        let mut handle = entity_manager.load_mut(pl_id)?;
        let player: &mut Player = handle
            .get_mut()
            .ok_or_else(|| utils::player_not_found(pl_id))?;

        let mut inv: Inventory = player
            .get_component()?
            .ok_or_else(|| utils::card_not_found(card_id))?;

        let card = inv
            .remove(card_id, &*entity_manager)
            .ok_or_else(|| utils::card_not_found(card_id))?;

        player.set_component(inv)?;
        CardModel::new(&card)
    })
    .await
    .map_err(utils::convert_blocking_err)?;

    Ok(HttpResponse::Ok().json(res))
}

#[derive(Deserialize, Debug, Clone)]
struct CardMoveOptions {
    to: Snowflake,
}

// POST /inventories/{playerid}/{cardid}/move
async fn move_card(
    path: web::Path<(Snowflake, Snowflake)>,
    query: web::Query<CardMoveOptions>,
    entity_manager: web::Data<EntityManager>,
) -> Result<HttpResponse> {
    let src_player_id = path.0;
    let card_id = path.1;
    let opts = query.into_inner();
    let dest_player_id = opts.to;

    web::block(move || -> Result<()> {
        let mut src_handle = entity_manager.load_mut(src_player_id)?;
        let src_player: &mut Player = src_handle
            .get_mut()
            .ok_or_else(|| utils::player_not_found(src_player_id))?;

        let mut src_inv: Inventory = src_player
            .get_component()?
            .ok_or_else(|| utils::card_not_found(card_id))?;

        let card = src_inv
            .remove(card_id, &*entity_manager)
            .ok_or_else(|| utils::card_not_found(card_id))?;

        let card = InventoryWriteRental::into_head(card);

        let mut dest_handle = entity_manager.load_mut(dest_player_id)?;
        let dest_player: &mut Player = dest_handle
            .get_mut()
            .ok_or_else(|| utils::player_not_found(dest_player_id))?;

        let mut dest_inv: Inventory = dest_player
            .get_component()?
            .unwrap_or_else(|| Inventory::empty());

        dest_inv.insert_handle(*card);

        dest_player.set_component(dest_inv)?;
        src_player.set_component(src_inv)?;

        dest_handle.store()?;

        Ok(())
    })
    .await
    .map_err(utils::convert_blocking_err)?;

    Ok(HttpResponse::NoContent().finish())
}

pub fn bind_routes(
    scope: Scope,
    entity_manager: web::Data<EntityManager>,
    sg: SnowflakeGeneratorState,
) -> Scope {
    scope
        .app_data(entity_manager)
        .app_data(sg)
        .route("/{invid}/{cardid}/move", web::post().to(move_card))
        .route("/{invid}/{cardid}", web::get().to(get_card))
        .route("/{invid}/{cardid}", web::delete().to(delete_card))
        .route("/{invid}", web::post().to(add_to_inventory))
        .route("/{invid}", web::get().to(get_inventory))
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::http;
    use futures::executor::block_on;
    use std::sync::Mutex;

    use crate::utils;
    use crate::utils::{create_new_card, create_new_player, get_body_json, ObjectNotFoundError};
    use akashi::SnowflakeGenerator;

    #[test]
    fn test_get_inventory() {
        let em = web::Data::new(utils::setup_entity_manager());
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);

        let (pl_id, mut pl) = create_new_player(&*em, &mut snowflake_gen);
        let (_, mut card) = create_new_card(&*em, &mut snowflake_gen);
        let mut inv: Inventory = Inventory::empty();

        card.set_component(CardName::new("foo".to_owned())).unwrap();
        card.set_component(CardValue::new(15.0)).unwrap();
        card.set_component(CardType::TypeA).unwrap();

        let expected = vec![CardModel::new(&card).unwrap()];

        inv.insert(card, &*em).unwrap();
        pl.set_component(inv).unwrap();
        em.store(pl).unwrap();

        let resp = block_on(get_inventory(web::Path::from((pl_id,)), em)).unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);

        let body: Vec<CardModel> = get_body_json(&resp);
        assert_eq!(body, expected);
    }

    #[test]
    fn test_get_inventory_not_exists() {
        let em = web::Data::new(utils::setup_entity_manager());
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);

        let resp = block_on(get_inventory(
            web::Path::from((snowflake_gen.generate(),)),
            em,
        ));
        let _e: ObjectNotFoundError = utils::expect_error(resp);
    }

    #[test]
    fn test_add_new_card() {
        let em = web::Data::new(utils::setup_entity_manager());
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);

        let (pl_id, pl) = create_new_player(&*em, &mut snowflake_gen);
        em.store(pl).unwrap();

        let resp = block_on(add_to_inventory(
            web::Path::from((pl_id,)),
            web::Json(InventoryAddOptions::New(NewCardOptions {
                card_type: CardType::TypeA,
                name: "bar".to_owned(),
                value: 13.0,
            })),
            web::Data::new(Mutex::new(snowflake_gen)),
            em.clone(),
        ))
        .unwrap();

        assert_eq!(resp.status(), http::StatusCode::OK);

        let body: Vec<CardModel> = get_body_json(&resp);

        let handle = em.load(pl_id).unwrap();
        let pl: &Player = handle.get().unwrap();

        let mut inv: Inventory = pl.get_component().unwrap().unwrap();

        let ids: Vec<Snowflake> = inv.iter_ids().copied().collect();
        let mut expected: Vec<CardModel> = Vec::with_capacity(ids.len());

        for id in ids {
            let card = inv.get(id, &*em).unwrap();
            expected.push(CardModel::new(card).unwrap())
        }

        assert_eq!(body, expected);
        assert_eq!(inv.len(), 1);

        let card = &expected[0];
        assert_eq!(card.card_type, CardType::TypeA);
        assert_eq!(card.name, "bar");
        assert_eq!(card.value, 13.0);
    }

    #[test]
    fn test_add_existing_card() {
        let em = web::Data::new(utils::setup_entity_manager());
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);

        let (pl_id, pl) = create_new_player(&*em, &mut snowflake_gen);
        em.store(pl).unwrap();

        let model = CardModel {
            id: snowflake_gen.generate(),
            card_type: CardType::TypeB,
            name: "baz".to_owned(),
            value: 500.0,
        };

        let resp = block_on(add_to_inventory(
            web::Path::from((pl_id,)),
            web::Json(InventoryAddOptions::Existing(model.clone())),
            web::Data::new(Mutex::new(snowflake_gen)),
            em.clone(),
        ))
        .unwrap();

        assert_eq!(resp.status(), http::StatusCode::OK);

        let body: Vec<CardModel> = get_body_json(&resp);

        let handle = em.load(pl_id).unwrap();
        let pl: &Player = handle.get().unwrap();

        let mut inv: Inventory = pl.get_component().unwrap().unwrap();
        let ids: Vec<Snowflake> = inv.iter_ids().copied().collect();
        let mut stored_inv: Vec<CardModel> = Vec::with_capacity(ids.len());

        for id in ids {
            let card = inv.get(id, &*em).unwrap();
            stored_inv.push(CardModel::new(card).unwrap())
        }

        assert_eq!(body, stored_inv);
        assert_eq!(stored_inv.len(), 1);
        assert_eq!(stored_inv[0], model);
    }

    #[test]
    fn test_get_card() {
        let em = web::Data::new(utils::setup_entity_manager());
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);

        let (pl_id, mut pl) = create_new_player(&*em, &mut snowflake_gen);
        let mut inv = Inventory::empty();

        let (card_id, mut card) = create_new_card(&*em, &mut snowflake_gen);
        card.set_component(CardName::new("foobar".to_owned()))
            .unwrap();
        card.set_component(CardValue::new(333.0)).unwrap();
        card.set_component(CardType::TypeC).unwrap();
        let expected = CardModel::new(&card).unwrap();

        inv.insert(card, &*em).unwrap();

        pl.set_component(inv).unwrap();
        em.store(pl).unwrap();

        let resp = block_on(get_card(web::Path::from((pl_id, card_id)), em)).unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);

        let body: CardModel = get_body_json(&resp);
        assert_eq!(body, expected);
    }

    #[test]
    fn test_get_card_not_exists() {
        let em = web::Data::new(utils::setup_entity_manager());
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);

        let (pl_id, mut pl) = create_new_player(&*em, &mut snowflake_gen);
        let inv = Inventory::empty();

        pl.set_component(inv).unwrap();
        em.store(pl).unwrap();

        let resp = block_on(get_card(
            web::Path::from((snowflake_gen.generate(), snowflake_gen.generate())),
            em.clone(),
        ));
        let _e: ObjectNotFoundError = utils::expect_error(resp);

        let resp = block_on(get_card(
            web::Path::from((pl_id, snowflake_gen.generate())),
            em,
        ));
        let _e: ObjectNotFoundError = utils::expect_error(resp);
    }

    #[test]
    fn test_delete_card() {
        let em = web::Data::new(utils::setup_entity_manager());
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);

        let (pl_id, mut pl) = create_new_player(&*em, &mut snowflake_gen);
        let mut inv = Inventory::empty();

        let (card_id, mut card) = create_new_card(&*em, &mut snowflake_gen);
        card.set_component(CardName::new("foobar".to_owned()))
            .unwrap();
        card.set_component(CardValue::new(333.0)).unwrap();
        card.set_component(CardType::TypeC).unwrap();
        let expected = CardModel::new(&card).unwrap();

        inv.insert(card.clone(), &*em).unwrap();
        assert_eq!(inv.len(), 1);

        pl.set_component(inv).unwrap();
        em.store(pl).unwrap();

        let resp = block_on(delete_card(web::Path::from((pl_id, card_id)), em.clone())).unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);

        let body: CardModel = get_body_json(&resp);
        assert_eq!(body, expected);

        let handle = em.load(pl_id).unwrap();
        let pl: &Player = handle.get().unwrap();

        let inv: Inventory = pl.get_component().unwrap().unwrap();
        assert_eq!(inv.len(), 0);
    }

    #[test]
    fn test_delete_card_not_exists() {
        let em = web::Data::new(utils::setup_entity_manager());
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);

        let (pl_id, mut pl) = create_new_player(&*em, &mut snowflake_gen);
        let inv = Inventory::empty();
        pl.set_component(inv).unwrap();

        let resp = block_on(delete_card(
            web::Path::from((snowflake_gen.generate(), snowflake_gen.generate())),
            em.clone(),
        ));
        let _e: ObjectNotFoundError = utils::expect_error(resp);

        let resp = block_on(delete_card(
            web::Path::from((pl_id, snowflake_gen.generate())),
            em,
        ));
        let _e: ObjectNotFoundError = utils::expect_error(resp);
    }

    #[test]
    fn test_move_card() {
        let em = web::Data::new(utils::setup_entity_manager());
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);

        let (src_pl_id, mut src_pl) = create_new_player(&*em, &mut snowflake_gen);
        let (dest_pl_id, dest_pl) = create_new_player(&*em, &mut snowflake_gen);

        let (card_id, mut card) = create_new_card(&*em, &mut snowflake_gen);
        card.set_component(CardName::new("foobar".to_owned()))
            .unwrap();
        card.set_component(CardValue::new(333.0)).unwrap();
        card.set_component(CardType::TypeC).unwrap();
        let expected = CardModel::new(&card).unwrap();

        let mut src_inv = Inventory::empty();
        src_inv.insert(card, &*em).unwrap();

        assert_eq!(src_inv.len(), 1);
        src_pl.set_component(src_inv).unwrap();
        em.store(src_pl).unwrap();
        em.store(dest_pl).unwrap();

        let query_str = format!("to={}", dest_pl_id);
        let query = web::Query::<CardMoveOptions>::from_query(query_str.as_str()).unwrap();

        let resp = block_on(move_card(
            web::Path::from((src_pl_id, card_id)),
            query,
            em.clone(),
        ))
        .unwrap();

        assert_eq!(resp.status(), http::StatusCode::NO_CONTENT);

        let src_handle = em.load(src_pl_id).unwrap();
        let src_pl: &Player = src_handle.get().unwrap();

        let dest_handle = em.load(dest_pl_id).unwrap();
        let dest_pl: &Player = dest_handle.get().unwrap();

        let src_inv: Inventory = src_pl.get_component().unwrap().unwrap();
        let mut dest_inv: Inventory = dest_pl.get_component().unwrap().unwrap();

        assert_eq!(src_inv.len(), 0);
        assert_eq!(dest_inv.len(), 1);

        let loaded_card_id: Snowflake;
        {
            loaded_card_id = *dest_inv.iter_ids().nth(0).unwrap();
        }

        let loaded_card = dest_inv.get(loaded_card_id, &*em).unwrap();
        let new_model = CardModel::new(loaded_card).unwrap();

        assert_eq!(expected, new_model);
    }

    #[test]
    fn test_move_card_nonexistent_dest() {
        let em = web::Data::new(utils::setup_entity_manager());
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);

        let (_card_id, mut card) = create_new_card(&*em, &mut snowflake_gen);
        card.set_component(CardName::new("foobar".to_owned()))
            .unwrap();
        card.set_component(CardValue::new(333.0)).unwrap();
        card.set_component(CardType::TypeC).unwrap();
        let expected = CardModel::new(&card).unwrap();
        let card_id = card.id();

        let (src_pl_id, mut src_pl) = create_new_player(&*em, &mut snowflake_gen);
        let mut src_inv = Inventory::empty();
        src_inv.insert(card, &*em).unwrap();
        assert_eq!(src_inv.len(), 1);

        src_pl.set_component(src_inv).unwrap();

        let query = web::Query::<CardMoveOptions>::from_query("to=1").unwrap();
        let resp = block_on(move_card(
            web::Path::from((src_pl_id, card_id)),
            query,
            em.clone(),
        ));
        let _e: ObjectNotFoundError = utils::expect_error(resp);

        let mut src_inv: Inventory = src_pl.get_component().unwrap().unwrap();
        assert_eq!(src_inv.len(), 1);

        let loaded_card_id: Snowflake;
        {
            loaded_card_id = *src_inv.iter_ids().nth(0).unwrap();
        }

        let loaded_card = src_inv.get(loaded_card_id, &*em).unwrap();
        let new_model = CardModel::new(loaded_card).unwrap();
        assert_eq!(expected, new_model);
    }
}
