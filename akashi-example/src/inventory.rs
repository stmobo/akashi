use failure::Error;
use serde::Deserialize;

use actix_web::{web, HttpResponse, Scope};

use akashi::store::{SharedStore, Store, StoreBackend};
use akashi::{
    Card, ComponentManager, ComponentStore, ComponentsAttached, Inventory, Player, Snowflake,
};

use crate::models::{CardModel, CardName, CardType, CardValue};
use crate::utils;
use crate::utils::{BadTransactionError, ObjectNotFoundError, Pagination, SnowflakeGeneratorState};

type Result<T> = std::result::Result<T, Error>;

// GET /inventories/{playerid}
async fn get_inventory<T, U>(
    path: web::Path<(Snowflake,)>,
    shared_store: web::Data<T>,
) -> Result<HttpResponse>
where
    T: SharedStore<Player, U> + Send + Sync + 'static,
    U: StoreBackend<Player> + Send + Sync + 'static,
{
    let id: Snowflake = path.0;

    let val = web::block(move || -> Result<Vec<CardModel>> {
        let store: &Store<Player, U> = shared_store.get_store();
        let wrapper = store.load(id)?;
        let handle = wrapper
            .lock()
            .map_err(|_e| format_err!("wrapper lock poisoned"))?;

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
    cm: web::Data<ComponentManager>,
) -> Result<HttpResponse>
where
    T: SharedStore<Player, U> + SharedStore<Card, U> + Send + Sync + 'static,
    U: StoreBackend<Player> + StoreBackend<Card> + Send + Sync + 'static,
{
    let pl_id = path.0;
    let cm = cm.into_inner();
    let opts = opts.into_inner();

    let res = web::block(move || -> Result<Vec<CardModel>> {
        let players: &Store<Player, U> = shared_store.get_store();
        let wrapper = players.load(pl_id)?;
        let handle = wrapper
            .lock()
            .map_err(|_e| format_err!("player handle lock poisoned"))?;
        let player = handle.get().ok_or_else(|| utils::player_not_found(pl_id))?;

        let mut inv: Inventory = player
            .get_component()?
            .unwrap_or_else(|| Inventory::empty(pl_id));

        let new_card: Card = match opts {
            InventoryAddOptions::Existing(c) => c.as_card(cm.clone())?,
            InventoryAddOptions::New(card_opts) => {
                let c: Card;

                {
                    let mut snowflake_gen = sg
                        .lock()
                        .map_err(|_e| format_err!("snowflake generator lock poisoned"))?;
                    c = Card::generate(&mut snowflake_gen, cm.clone());
                }

                c.set_component(card_opts.card_type)?;
                c.set_component(CardName::new(card_opts.name))?;
                c.set_component(CardValue::new(card_opts.value))?;

                let cards: &Store<Card, U> = shared_store.get_store();
                cards.store(c.id(), c.clone())?;
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
) -> Result<HttpResponse>
where
    T: SharedStore<Player, U> + Send + Sync + 'static,
    U: StoreBackend<Player> + Send + Sync + 'static,
{
    let pl_id = path.0;
    let card_id = path.1;

    let res: CardModel = web::block(move || -> Result<CardModel> {
        let players: &Store<Player, U> = shared_store.get_store();
        let wrapper = players.load(pl_id)?;
        let handle = wrapper
            .lock()
            .map_err(|_e| format_err!("player handle lock poisoned"))?;
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
) -> Result<HttpResponse>
where
    T: SharedStore<Player, U> + Send + Sync + 'static,
    U: StoreBackend<Player> + Send + Sync + 'static,
{
    let pl_id = path.0;
    let card_id = path.1;

    let res: CardModel = web::block(move || -> Result<CardModel> {
        let players: &Store<Player, U> = shared_store.get_store();
        let wrapper = players.load(pl_id)?;
        let handle = wrapper
            .lock()
            .map_err(|_e| format_err!("player handle lock poisoned"))?;
        let player = handle.get().ok_or_else(|| utils::player_not_found(pl_id))?;

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
) -> Result<HttpResponse>
where
    T: SharedStore<Player, U> + Send + Sync + 'static,
    U: StoreBackend<Player> + Send + Sync + 'static,
{
    let src_player_id = path.0;
    let card_id = path.1;
    let opts = query.into_inner();
    let dest_player_id = opts.to;

    web::block(move || -> Result<()> {
        let players: &Store<Player, U> = shared_store.get_store();

        let src_wrapper = players.load(src_player_id)?;
        let src_handle = src_wrapper
            .lock()
            .map_err(|_e| format_err!("player handle lock poisoned"))?;
        let src_player = src_handle
            .get()
            .ok_or_else(|| utils::player_not_found(src_player_id))?;

        let mut src_inv: Inventory = src_player
            .get_component()?
            .ok_or_else(|| utils::card_not_found(card_id))?;
        let card: Card = src_inv
            .remove(card_id)
            .ok_or_else(|| utils::card_not_found(card_id))?;

        let dest_wrapper = players.load(dest_player_id)?;
        let dest_handle = dest_wrapper
            .lock()
            .map_err(|_e| format_err!("player handle lock poisoned"))?;
        let dest_player = dest_handle
            .get()
            .ok_or_else(|| utils::player_not_found(dest_player_id))?;

        let mut dest_inv: Inventory = dest_player
            .get_component()?
            .unwrap_or_else(|| Inventory::empty(dest_player_id));
        dest_inv.insert(card);

        dest_player.set_component(dest_inv)?;
        src_player.set_component(src_inv)?;

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
    cm: web::Data<ComponentManager>,
) -> Scope
where
    T: SharedStore<Player, U> + SharedStore<Card, U> + Send + Sync + 'static,
    U: StoreBackend<Player> + StoreBackend<Card> + Send + Sync + 'static,
{
    scope
        .app_data(store)
        .app_data(sg)
        .app_data(cm)
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
    use crate::utils::{get_body_json, snowflake_generator, store};
    use akashi::local_storage::SharedLocalStore;
    use akashi::SnowflakeGenerator;

    #[test]
    fn test_get_inventory() {
        let shared_store = SharedLocalStore::new();
        let cm = Arc::new(utils::new_component_manager(&shared_store));
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);

        let (pl_id, pl) = utils::create_new_player(&shared_store, &mut snowflake_gen, cm.clone());
        let mut inv: Inventory = pl
            .get_component()
            .unwrap()
            .unwrap_or_else(|| Inventory::empty(pl.id()));

        let card = Card::generate(&mut snowflake_gen, cm.clone());
        card.set_component(CardName::new("foo".to_owned())).unwrap();
        card.set_component(CardValue::new(15.0)).unwrap();
        card.set_component(CardType::TypeA).unwrap();

        let expected = vec![CardModel::new(&card).unwrap()];

        inv.insert(card);
        pl.set_component(inv).unwrap();

        let resp = block_on(get_inventory(
            web::Path::from((pl_id,)),
            web::Data::new(shared_store),
        ))
        .unwrap();

        assert_eq!(resp.status(), http::StatusCode::OK);

        let body: Vec<CardModel> = get_body_json(&resp);
        assert_eq!(body, expected);
    }

    #[test]
    fn test_get_inventory_not_exists() {
        let shared_store = SharedLocalStore::new();
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);

        let resp = block_on(get_inventory(
            web::Path::from((snowflake_gen.generate(),)),
            web::Data::new(shared_store),
        ));
        let _e: ObjectNotFoundError = utils::expect_error(resp);
    }

    #[test]
    fn test_add_new_card() {
        let shared_store = store();
        let cm = web::Data::new(utils::new_component_manager(&shared_store));
        let sg = snowflake_generator(0, 0);
        let inv: Inventory;

        let (pl_id, pl) = utils::create_new_player(
            &shared_store,
            &mut sg.lock().unwrap(),
            cm.clone().into_inner(),
        );
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
            cm,
        ))
        .unwrap();

        assert_eq!(resp.status(), http::StatusCode::OK);

        let body: Vec<CardModel> = get_body_json(&resp);

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

        let wrapper = card_store.load(card.id).unwrap();
        let handle = wrapper.lock().unwrap();
        let stored_card = CardModel::new(handle.get().unwrap()).unwrap();

        assert_eq!(*card, stored_card);
    }

    #[test]
    fn test_add_existing_card() {
        let shared_store = store();
        let cm = web::Data::new(utils::new_component_manager(&shared_store));
        let sg = snowflake_generator(0, 0);

        let (pl_id, pl) = utils::create_new_player(
            &shared_store,
            &mut sg.lock().unwrap(),
            cm.clone().into_inner(),
        );

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
            cm,
        ))
        .unwrap();

        assert_eq!(resp.status(), http::StatusCode::OK);

        let body: Vec<CardModel> = get_body_json(&resp);
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
        let cm = Arc::new(utils::new_component_manager(&shared_store));
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);

        let (pl_id, pl) = utils::create_new_player(&shared_store, &mut snowflake_gen, cm.clone());
        let mut inv = Inventory::empty(pl_id);

        let card = Card::generate(&mut snowflake_gen, cm.clone());
        card.set_component(CardName::new("foobar".to_owned()))
            .unwrap();
        card.set_component(CardValue::new(333.0)).unwrap();
        card.set_component(CardType::TypeC).unwrap();
        let expected = CardModel::new(&card).unwrap();

        let card_id = card.id();
        inv.insert(card);

        pl.set_component(inv).unwrap();

        let resp = block_on(get_card(web::Path::from((pl_id, card_id)), shared_store)).unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);

        let body: CardModel = get_body_json(&resp);
        assert_eq!(body, expected);
    }

    #[test]
    fn test_get_card_not_exists() {
        let shared_store = store();
        let cm = Arc::new(utils::new_component_manager(&shared_store));
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);

        let (pl_id, pl) = utils::create_new_player(&shared_store, &mut snowflake_gen, cm.clone());
        let inv = Inventory::empty(pl_id);
        pl.set_component(inv).unwrap();

        let resp = block_on(get_card(
            web::Path::from((snowflake_gen.generate(), snowflake_gen.generate())),
            shared_store.clone(),
        ));
        let _e: ObjectNotFoundError = utils::expect_error(resp);

        let resp = block_on(get_card(
            web::Path::from((pl_id, snowflake_gen.generate())),
            shared_store,
        ));
        let _e: ObjectNotFoundError = utils::expect_error(resp);
    }

    #[test]
    fn test_delete_card() {
        let shared_store = store();
        let cm = Arc::new(utils::new_component_manager(&shared_store));
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);

        let (pl_id, pl) = utils::create_new_player(&shared_store, &mut snowflake_gen, cm.clone());
        let mut inv = Inventory::empty(pl_id);

        let card = Card::generate(&mut snowflake_gen, cm.clone());
        card.set_component(CardName::new("foobar".to_owned()))
            .unwrap();
        card.set_component(CardValue::new(333.0)).unwrap();
        card.set_component(CardType::TypeC).unwrap();
        let expected = CardModel::new(&card).unwrap();
        let card_id = card.id();

        inv.insert(card.clone());
        assert_eq!(inv.len(), 1);

        pl.set_component(inv).unwrap();

        let resp = block_on(delete_card(
            web::Path::from((pl_id, card_id)),
            shared_store.clone(),
        ))
        .unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);

        let body: CardModel = get_body_json(&resp);
        assert_eq!(body, expected);

        let inv: Inventory = pl.get_component().unwrap().unwrap();
        assert_eq!(inv.len(), 0);
    }

    #[test]
    fn test_delete_card_not_exists() {
        let shared_store = store();
        let cm = Arc::new(utils::new_component_manager(&shared_store));
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);

        let (pl_id, pl) = utils::create_new_player(&shared_store, &mut snowflake_gen, cm.clone());
        let inv = Inventory::empty(pl_id);
        pl.set_component(inv).unwrap();

        let resp = block_on(delete_card(
            web::Path::from((snowflake_gen.generate(), snowflake_gen.generate())),
            shared_store.clone(),
        ));
        let _e: ObjectNotFoundError = utils::expect_error(resp);

        let resp = block_on(delete_card(
            web::Path::from((pl_id, snowflake_gen.generate())),
            shared_store,
        ));
        let _e: ObjectNotFoundError = utils::expect_error(resp);
    }

    #[test]
    fn test_move_card() {
        let shared_store = store();
        let cm = Arc::new(utils::new_component_manager(&shared_store));
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);

        let card = Card::generate(&mut snowflake_gen, cm.clone());
        card.set_component(CardName::new("foobar".to_owned()))
            .unwrap();
        card.set_component(CardValue::new(333.0)).unwrap();
        card.set_component(CardType::TypeC).unwrap();
        let expected = CardModel::new(&card).unwrap();
        let card_id = card.id();

        let (src_pl_id, src_pl) =
            utils::create_new_player(&shared_store, &mut snowflake_gen, cm.clone());
        let mut src_inv = Inventory::empty(src_pl_id);
        src_inv.insert(card);

        assert_eq!(src_inv.len(), 1);
        src_pl.set_component(src_inv).unwrap();

        let (dest_pl_id, dest_pl) =
            utils::create_new_player(&shared_store, &mut snowflake_gen, cm.clone());

        let query_str = format!("to={}", dest_pl_id);
        let query = web::Query::<CardMoveOptions>::from_query(query_str.as_str()).unwrap();

        let resp = block_on(move_card(
            web::Path::from((src_pl_id, card_id)),
            shared_store.clone(),
            query,
        ))
        .unwrap();

        assert_eq!(resp.status(), http::StatusCode::NO_CONTENT);

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
        let cm = Arc::new(utils::new_component_manager(&shared_store));
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);

        let card = Card::generate(&mut snowflake_gen, cm.clone());
        card.set_component(CardName::new("foobar".to_owned()))
            .unwrap();
        card.set_component(CardValue::new(333.0)).unwrap();
        card.set_component(CardType::TypeC).unwrap();
        let expected = CardModel::new(&card).unwrap();
        let card_id = card.id();

        let (src_pl_id, src_pl) =
            utils::create_new_player(&shared_store, &mut snowflake_gen, cm.clone());
        let mut src_inv = Inventory::empty(src_pl_id);
        src_inv.insert(card);
        assert_eq!(src_inv.len(), 1);

        src_pl.set_component(src_inv).unwrap();

        let query = web::Query::<CardMoveOptions>::from_query("to=1").unwrap();
        let resp = block_on(move_card(
            web::Path::from((src_pl_id, card_id)),
            shared_store.clone(),
            query,
        ));
        let _e: ObjectNotFoundError = utils::expect_error(resp);

        let src_inv: Inventory = src_pl.get_component().unwrap().unwrap();
        assert_eq!(src_inv.len(), 1);

        let loaded_card = src_inv.iter().nth(0).unwrap();
        let new_model = CardModel::new(loaded_card).unwrap();
        assert_eq!(expected, new_model);
    }
}
