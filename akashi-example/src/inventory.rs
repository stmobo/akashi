use failure::Error;
use serde::Deserialize;

use actix_web::{web, HttpResponse, Scope};

use akashi::components::Inventory;
use akashi::store::SharedStore;
use akashi::{Card, ComponentManager, Entity, Player, Snowflake, Store, StoreBackend};

use crate::models::{CardModel, CardName, CardType, CardValue};
use crate::utils;
use crate::utils::SnowflakeGeneratorState;

type Result<T> = std::result::Result<T, Error>;

// GET /inventories/{playerid}
async fn get_inventory<T, U>(
    path: web::Path<(Snowflake,)>,
    shared_store: web::Data<T>,
    cm: web::Data<ComponentManager<Player>>,
) -> Result<HttpResponse>
where
    T: SharedStore<Player, U> + Send + Sync + 'static,
    U: StoreBackend<Player> + Send + Sync + 'static,
{
    let id: Snowflake = path.0;

    let val = web::block(move || -> Result<Vec<CardModel>> {
        let store: &Store<Player, U> = shared_store.get_store();
        let handle = store.load(id, cm.into_inner())?;

        let pl_ref: &Player = handle.get().ok_or_else(|| utils::player_not_found(id))?;
        let inv: Option<Inventory> = pl_ref.get_component()?;

        Ok(match inv {
            None => vec![],
            Some(v) => {
                let mut model: Vec<CardModel> = Vec::new();
                model.reserve(v.len());

                for card in v.iter() {
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
async fn add_to_inventory<T, U>(
    path: web::Path<(Snowflake,)>,
    opts: web::Json<InventoryAddOptions>,
    shared_store: web::Data<T>,
    sg: SnowflakeGeneratorState,
    pl_cm: web::Data<ComponentManager<Player>>,
    card_cm: web::Data<ComponentManager<Card>>,
) -> Result<HttpResponse>
where
    T: SharedStore<Player, U> + SharedStore<Card, U> + Send + Sync + 'static,
    U: StoreBackend<Player> + StoreBackend<Card> + Send + Sync + 'static,
{
    let pl_id = path.0;
    let pl_cm = pl_cm.into_inner();
    let card_cm = card_cm.into_inner();
    let opts = opts.into_inner();

    let res = web::block(move || -> Result<Vec<CardModel>> {
        let players: &Store<Player, U> = shared_store.get_store();
        let mut handle = players.load_mut(pl_id, pl_cm.clone())?;

        let player = handle
            .get_mut()
            .ok_or_else(|| utils::player_not_found(pl_id))?;

        let mut inv: Inventory = player
            .get_component()?
            .unwrap_or_else(|| Inventory::empty());

        let new_card: Card = match opts {
            InventoryAddOptions::Existing(c) => c.as_card(card_cm.clone())?,
            InventoryAddOptions::New(card_opts) => {
                let mut c: Card;

                {
                    let mut snowflake_gen = sg
                        .lock()
                        .map_err(|_e| format_err!("snowflake generator lock poisoned"))?;
                    c = Card::generate(&mut snowflake_gen, card_cm.clone());
                }

                c.set_component(card_opts.card_type)?;
                c.set_component(CardName::new(card_opts.name))?;
                c.set_component(CardValue::new(card_opts.value))?;

                let cards: &Store<Card, U> = shared_store.get_store();
                cards.store(c.clone())?;
                c
            }
        };

        let mut model: Vec<CardModel> = Vec::new();
        model.reserve(inv.len() + 1);
        for card in inv.iter() {
            model.push(CardModel::new(card)?);
        }

        model.push(CardModel::new(&new_card)?);
        inv.insert(new_card);

        player.set_component(inv)?;
        handle.store()?;

        Ok(model)
    })
    .await
    .map_err(utils::convert_blocking_err)?;

    Ok(HttpResponse::Ok().json(res))
}

// GET /inventories/{playerid}/{cardid}
async fn get_card<T, U>(
    path: web::Path<(Snowflake, Snowflake)>,
    shared_store: web::Data<T>,
    cm: web::Data<ComponentManager<Player>>,
) -> Result<HttpResponse>
where
    T: SharedStore<Player, U> + Send + Sync + 'static,
    U: StoreBackend<Player> + Send + Sync + 'static,
{
    let pl_id = path.0;
    let card_id = path.1;
    let cm = cm.into_inner();

    let res: CardModel = web::block(move || -> Result<CardModel> {
        let players: &Store<Player, U> = shared_store.get_store();
        let handle = players.load(pl_id, cm)?;
        let player = handle.get().ok_or_else(|| utils::player_not_found(pl_id))?;

        let inv: Inventory = player
            .get_component()?
            .ok_or_else(|| utils::card_not_found(card_id))?;
        let card: &Card = inv
            .get(card_id)
            .ok_or_else(|| utils::card_not_found(card_id))?;

        CardModel::new(card)
    })
    .await
    .map_err(utils::convert_blocking_err)?;

    Ok(HttpResponse::Ok().json(res))
}

// DELETE /inventories/{playerid}/{cardid}
async fn delete_card<T, U>(
    path: web::Path<(Snowflake, Snowflake)>,
    shared_store: web::Data<T>,
    cm: web::Data<ComponentManager<Player>>,
) -> Result<HttpResponse>
where
    T: SharedStore<Player, U> + Send + Sync + 'static,
    U: StoreBackend<Player> + Send + Sync + 'static,
{
    let pl_id = path.0;
    let card_id = path.1;
    let cm = cm.into_inner();

    let res: CardModel = web::block(move || -> Result<CardModel> {
        let players: &Store<Player, U> = shared_store.get_store();
        let mut handle = players.load_mut(pl_id, cm)?;
        let player = handle
            .get_mut()
            .ok_or_else(|| utils::player_not_found(pl_id))?;

        let mut inv: Inventory = player
            .get_component()?
            .ok_or_else(|| utils::card_not_found(card_id))?;
        let card: Card = inv
            .remove(card_id)
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
async fn move_card<T, U>(
    path: web::Path<(Snowflake, Snowflake)>,
    shared_store: web::Data<T>,
    query: web::Query<CardMoveOptions>,
    cm: web::Data<ComponentManager<Player>>,
) -> Result<HttpResponse>
where
    T: SharedStore<Player, U> + Send + Sync + 'static,
    U: StoreBackend<Player> + Send + Sync + 'static,
{
    let src_player_id = path.0;
    let card_id = path.1;
    let opts = query.into_inner();
    let dest_player_id = opts.to;
    let cm = cm.into_inner();

    web::block(move || -> Result<()> {
        let players: &Store<Player, U> = shared_store.get_store();

        let mut src_handle = players.load_mut(src_player_id, cm.clone())?;
        let src_player = src_handle
            .get_mut()
            .ok_or_else(|| utils::player_not_found(src_player_id))?;

        let mut src_inv: Inventory = src_player
            .get_component()?
            .ok_or_else(|| utils::card_not_found(card_id))?;
        let card: Card = src_inv
            .remove(card_id)
            .ok_or_else(|| utils::card_not_found(card_id))?;

        let mut dest_handle = players.load_mut(dest_player_id, cm.clone())?;
        let dest_player = dest_handle
            .get_mut()
            .ok_or_else(|| utils::player_not_found(dest_player_id))?;

        let mut dest_inv: Inventory = dest_player
            .get_component()?
            .unwrap_or_else(|| Inventory::empty());
        dest_inv.insert(card);

        dest_player.set_component(dest_inv)?;
        src_player.set_component(src_inv)?;

        dest_handle.store()?;

        Ok(())
    })
    .await
    .map_err(utils::convert_blocking_err)?;

    Ok(HttpResponse::NoContent().finish())
}

pub fn bind_routes<T, U>(
    scope: Scope,
    store: web::Data<T>,
    sg: SnowflakeGeneratorState,
    pl_cm: web::Data<ComponentManager<Player>>,
    card_cm: web::Data<ComponentManager<Card>>,
) -> Scope
where
    T: SharedStore<Player, U> + SharedStore<Card, U> + Send + Sync + 'static,
    U: StoreBackend<Player> + StoreBackend<Card> + Send + Sync + 'static,
{
    scope
        .app_data(store)
        .app_data(sg)
        .app_data(pl_cm)
        .app_data(card_cm)
        .route("/{invid}/{cardid}/move", web::post().to(move_card::<T, U>))
        .route("/{invid}/{cardid}", web::get().to(get_card::<T, U>))
        .route("/{invid}/{cardid}", web::delete().to(delete_card::<T, U>))
        .route("/{invid}", web::post().to(add_to_inventory::<T, U>))
        .route("/{invid}", web::get().to(get_inventory::<T, U>))
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::http;
    use futures::executor::block_on;
    use std::sync::Arc;

    use crate::utils;
    use crate::utils::{get_body_json, snowflake_generator, store, ObjectNotFoundError};
    use akashi::local_storage::SharedLocalStore;
    use akashi::SnowflakeGenerator;

    #[test]
    fn test_get_inventory() {
        let shared_store = SharedLocalStore::new();
        let pl_cm = web::Data::new(utils::player_component_manager(&shared_store));
        let arc_pl_cm = pl_cm.clone().into_inner();

        let card_cm = web::Data::new(utils::card_component_manager());
        let arc_card_cm = card_cm.clone().into_inner();

        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);

        let (pl_id, mut pl) =
            utils::create_new_player(&shared_store, &mut snowflake_gen, arc_pl_cm.clone());
        let mut inv: Inventory = Inventory::empty();

        let mut card = Card::generate(&mut snowflake_gen, arc_card_cm);
        card.set_component(CardName::new("foo".to_owned())).unwrap();
        card.set_component(CardValue::new(15.0)).unwrap();
        card.set_component(CardType::TypeA).unwrap();

        let expected = vec![CardModel::new(&card).unwrap()];

        inv.insert(card);
        pl.set_component(inv).unwrap();
        shared_store.players().store(pl).unwrap();

        let resp = block_on(get_inventory(
            web::Path::from((pl_id,)),
            web::Data::new(shared_store),
            pl_cm,
        ))
        .unwrap();

        assert_eq!(resp.status(), http::StatusCode::OK);

        let body: Vec<CardModel> = get_body_json(&resp);
        assert_eq!(body, expected);
    }

    #[test]
    fn test_get_inventory_not_exists() {
        let shared_store = SharedLocalStore::new();
        let pl_cm = web::Data::new(utils::player_component_manager(&shared_store));

        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);

        let resp = block_on(get_inventory(
            web::Path::from((snowflake_gen.generate(),)),
            web::Data::new(shared_store),
            pl_cm,
        ));
        let _e: ObjectNotFoundError = utils::expect_error(resp);
    }

    #[test]
    fn test_add_new_card() {
        let shared_store = store();
        let pl_cm = web::Data::new(utils::player_component_manager(&shared_store));
        let arc_pl_cm = pl_cm.clone().into_inner();

        let card_cm = web::Data::new(utils::card_component_manager());
        let arc_card_cm = card_cm.clone().into_inner();
        let sg = snowflake_generator(0, 0);

        let (pl_id, _pl) =
            utils::create_new_player(&shared_store, &mut sg.lock().unwrap(), arc_pl_cm.clone());
        let card_store = shared_store.cards();

        let resp = block_on(add_to_inventory(
            web::Path::from((pl_id,)),
            web::Json(InventoryAddOptions::New(NewCardOptions {
                card_type: CardType::TypeA,
                name: "bar".to_owned(),
                value: 13.0,
            })),
            shared_store.clone(),
            sg,
            pl_cm.clone(),
            card_cm.clone(),
        ))
        .unwrap();

        assert_eq!(resp.status(), http::StatusCode::OK);

        let body: Vec<CardModel> = get_body_json(&resp);

        let handle = shared_store.players().load(pl_id, arc_pl_cm).unwrap();
        let pl = handle.get().unwrap();

        let inv: Inventory = pl.get_component().unwrap().unwrap();
        let expected: Vec<CardModel> = inv
            .iter()
            .map(|card| CardModel::new(card).unwrap())
            .collect();

        assert_eq!(body, expected);
        assert_eq!(inv.len(), 1);

        let card = &expected[0];
        assert_eq!(card.card_type, CardType::TypeA);
        assert_eq!(card.name, "bar");
        assert_eq!(card.value, 13.0);

        let handle = card_store.load(card.id, arc_card_cm).unwrap();
        let stored_card = CardModel::new(handle.get().unwrap()).unwrap();

        assert_eq!(*card, stored_card);
    }

    #[test]
    fn test_add_existing_card() {
        let shared_store = store();
        let pl_cm = web::Data::new(utils::player_component_manager(&shared_store));
        let card_cm = web::Data::new(utils::card_component_manager());
        let arc_pl_cm = pl_cm.clone().into_inner();

        let sg = snowflake_generator(0, 0);

        let (pl_id, _pl) =
            utils::create_new_player(&shared_store, &mut sg.lock().unwrap(), arc_pl_cm.clone());

        let model = CardModel {
            id: sg.lock().unwrap().generate(),
            card_type: CardType::TypeB,
            name: "baz".to_owned(),
            value: 500.0,
        };

        let resp = block_on(add_to_inventory(
            web::Path::from((pl_id,)),
            web::Json(InventoryAddOptions::Existing(model.clone())),
            shared_store.clone(),
            sg,
            pl_cm,
            card_cm,
        ))
        .unwrap();

        assert_eq!(resp.status(), http::StatusCode::OK);

        let body: Vec<CardModel> = get_body_json(&resp);

        let handle = shared_store.players().load(pl_id, arc_pl_cm).unwrap();
        let pl = handle.get().unwrap();

        let inv: Inventory = pl.get_component().unwrap().unwrap();
        let stored_inv: Vec<CardModel> = inv
            .iter()
            .map(|card| CardModel::new(card).unwrap())
            .collect();

        assert_eq!(body, stored_inv);
        assert_eq!(stored_inv.len(), 1);
        assert_eq!(stored_inv[0], model);
    }

    #[test]
    fn test_get_card() {
        let shared_store = store();
        let pl_cm = web::Data::new(utils::player_component_manager(&shared_store));
        let arc_card_cm = Arc::new(utils::card_component_manager());
        let arc_pl_cm = pl_cm.clone().into_inner();
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);

        let (pl_id, mut pl) =
            utils::create_new_player(&shared_store, &mut snowflake_gen, arc_pl_cm);
        let mut inv = Inventory::empty();

        let mut card = Card::generate(&mut snowflake_gen, arc_card_cm);
        card.set_component(CardName::new("foobar".to_owned()))
            .unwrap();
        card.set_component(CardValue::new(333.0)).unwrap();
        card.set_component(CardType::TypeC).unwrap();
        let expected = CardModel::new(&card).unwrap();

        let card_id = card.id();
        inv.insert(card);

        pl.set_component(inv).unwrap();
        shared_store.players().store(pl).unwrap();

        let resp = block_on(get_card(
            web::Path::from((pl_id, card_id)),
            shared_store,
            pl_cm,
        ))
        .unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);

        let body: CardModel = get_body_json(&resp);
        assert_eq!(body, expected);
    }

    #[test]
    fn test_get_card_not_exists() {
        let shared_store = store();
        let pl_cm = web::Data::new(utils::player_component_manager(&shared_store));
        let arc_pl_cm = pl_cm.clone().into_inner();

        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);

        let (pl_id, mut pl) =
            utils::create_new_player(&shared_store, &mut snowflake_gen, arc_pl_cm);
        let inv = Inventory::empty();
        pl.set_component(inv).unwrap();
        shared_store.players().store(pl).unwrap();

        let resp = block_on(get_card(
            web::Path::from((snowflake_gen.generate(), snowflake_gen.generate())),
            shared_store.clone(),
            pl_cm.clone(),
        ));
        let _e: ObjectNotFoundError = utils::expect_error(resp);

        let resp = block_on(get_card(
            web::Path::from((pl_id, snowflake_gen.generate())),
            shared_store,
            pl_cm,
        ));
        let _e: ObjectNotFoundError = utils::expect_error(resp);
    }

    #[test]
    fn test_delete_card() {
        let shared_store = store();
        let pl_cm = web::Data::new(utils::player_component_manager(&shared_store));
        let arc_card_cm = Arc::new(utils::card_component_manager());
        let arc_pl_cm = pl_cm.clone().into_inner();

        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);

        let (pl_id, mut pl) =
            utils::create_new_player(&shared_store, &mut snowflake_gen, arc_pl_cm.clone());
        let mut inv = Inventory::empty();

        let mut card = Card::generate(&mut snowflake_gen, arc_card_cm.clone());
        card.set_component(CardName::new("foobar".to_owned()))
            .unwrap();
        card.set_component(CardValue::new(333.0)).unwrap();
        card.set_component(CardType::TypeC).unwrap();
        let expected = CardModel::new(&card).unwrap();
        let card_id = card.id();

        inv.insert(card.clone());
        assert_eq!(inv.len(), 1);

        pl.set_component(inv).unwrap();
        shared_store.players().store(pl).unwrap();

        let resp = block_on(delete_card(
            web::Path::from((pl_id, card_id)),
            shared_store.clone(),
            pl_cm,
        ))
        .unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);

        let body: CardModel = get_body_json(&resp);
        assert_eq!(body, expected);

        let handle = shared_store.players().load(pl_id, arc_pl_cm).unwrap();
        let pl = handle.get().unwrap();

        let inv: Inventory = pl.get_component().unwrap().unwrap();
        assert_eq!(inv.len(), 0);
    }

    #[test]
    fn test_delete_card_not_exists() {
        let shared_store = store();
        let pl_cm = web::Data::new(utils::player_component_manager(&shared_store));
        let arc_pl_cm = pl_cm.clone().into_inner();

        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);

        let (pl_id, mut pl) =
            utils::create_new_player(&shared_store, &mut snowflake_gen, arc_pl_cm);
        let inv = Inventory::empty();
        pl.set_component(inv).unwrap();

        let resp = block_on(delete_card(
            web::Path::from((snowflake_gen.generate(), snowflake_gen.generate())),
            shared_store.clone(),
            pl_cm.clone(),
        ));
        let _e: ObjectNotFoundError = utils::expect_error(resp);

        let resp = block_on(delete_card(
            web::Path::from((pl_id, snowflake_gen.generate())),
            shared_store,
            pl_cm,
        ));
        let _e: ObjectNotFoundError = utils::expect_error(resp);
    }

    #[test]
    fn test_move_card() {
        let shared_store = store();
        let pl_cm = web::Data::new(utils::player_component_manager(&shared_store));
        let arc_card_cm = Arc::new(utils::card_component_manager());
        let arc_pl_cm = pl_cm.clone().into_inner();

        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);

        let mut card = Card::generate(&mut snowflake_gen, arc_card_cm);
        card.set_component(CardName::new("foobar".to_owned()))
            .unwrap();
        card.set_component(CardValue::new(333.0)).unwrap();
        card.set_component(CardType::TypeC).unwrap();
        let expected = CardModel::new(&card).unwrap();
        let card_id = card.id();

        let (src_pl_id, mut src_pl) =
            utils::create_new_player(&shared_store, &mut snowflake_gen, arc_pl_cm.clone());
        let mut src_inv = Inventory::empty();
        src_inv.insert(card);

        assert_eq!(src_inv.len(), 1);
        src_pl.set_component(src_inv).unwrap();
        shared_store.players().store(src_pl).unwrap();

        let (dest_pl_id, _dest_pl) =
            utils::create_new_player(&shared_store, &mut snowflake_gen, arc_pl_cm.clone());

        let query_str = format!("to={}", dest_pl_id);
        let query = web::Query::<CardMoveOptions>::from_query(query_str.as_str()).unwrap();

        let resp = block_on(move_card(
            web::Path::from((src_pl_id, card_id)),
            shared_store.clone(),
            query,
            pl_cm,
        ))
        .unwrap();

        assert_eq!(resp.status(), http::StatusCode::NO_CONTENT);

        let src_handle = shared_store
            .players()
            .load(src_pl_id, arc_pl_cm.clone())
            .unwrap();
        let src_pl = src_handle.get().unwrap();

        let dest_handle = shared_store
            .players()
            .load(dest_pl_id, arc_pl_cm.clone())
            .unwrap();
        let dest_pl = dest_handle.get().unwrap();

        let src_inv: Inventory = src_pl.get_component().unwrap().unwrap();
        let dest_inv: Inventory = dest_pl.get_component().unwrap().unwrap();

        assert_eq!(src_inv.len(), 0);
        assert_eq!(dest_inv.len(), 1);

        let loaded_card = dest_inv.iter().nth(0).unwrap();
        let new_model = CardModel::new(loaded_card).unwrap();

        assert_eq!(expected, new_model);
    }

    #[test]
    fn test_move_card_nonexistent_dest() {
        let shared_store = store();
        let pl_cm = web::Data::new(utils::player_component_manager(&shared_store));
        let arc_card_cm = Arc::new(utils::card_component_manager());
        let arc_pl_cm = pl_cm.clone().into_inner();
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);

        let mut card = Card::generate(&mut snowflake_gen, arc_card_cm);
        card.set_component(CardName::new("foobar".to_owned()))
            .unwrap();
        card.set_component(CardValue::new(333.0)).unwrap();
        card.set_component(CardType::TypeC).unwrap();
        let expected = CardModel::new(&card).unwrap();
        let card_id = card.id();

        let (src_pl_id, mut src_pl) =
            utils::create_new_player(&shared_store, &mut snowflake_gen, arc_pl_cm);
        let mut src_inv = Inventory::empty();
        src_inv.insert(card);
        assert_eq!(src_inv.len(), 1);

        src_pl.set_component(src_inv).unwrap();

        let query = web::Query::<CardMoveOptions>::from_query("to=1").unwrap();
        let resp = block_on(move_card(
            web::Path::from((src_pl_id, card_id)),
            shared_store.clone(),
            query,
            pl_cm,
        ));
        let _e: ObjectNotFoundError = utils::expect_error(resp);

        let src_inv: Inventory = src_pl.get_component().unwrap().unwrap();
        assert_eq!(src_inv.len(), 1);

        let loaded_card = src_inv.iter().nth(0).unwrap();
        let new_model = CardModel::new(loaded_card).unwrap();
        assert_eq!(expected, new_model);
    }
}
