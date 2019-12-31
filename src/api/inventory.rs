use serde::Deserialize;
use std::ops::DerefMut;

use actix_web::{web, HttpResponse, Scope};

use crate::card::{Card, Inventory};
use crate::snowflake::Snowflake;
use crate::store::{SharedStore, Store, StoreBackend};

use super::utils::{APIError, Result, SnowflakeGeneratorState};

// GET /inventories/{invid}
async fn get_inventory<T, U>(
    path: web::Path<(Snowflake,)>,
    shared_store: web::Data<T>,
) -> Result<HttpResponse>
where
    T: SharedStore<Inventory, U> + Send + Sync + 'static,
    U: StoreBackend<Inventory> + Send + Sync + 'static,
{
    let id: Snowflake = path.0;

    let val = web::block(move || -> Result<Inventory> {
        let store: &Store<Inventory, U> = shared_store.get_store();
        let inv_ref = store.load(id)?;
        let handle = inv_ref.lock().unwrap();
        match handle.get() {
            None => Err(APIError::not_found(format!(
                "Could not find inventory {}",
                id
            ))),
            Some(v) => Ok(v.clone()),
        }
    })
    .await?;

    Ok(HttpResponse::Ok().json(val))
}

#[derive(Deserialize)]
#[serde(tag = "from", content = "options")]
enum InventoryAddOptions {
    Existing(Card),
    New(Snowflake),
}

// POST /inventories/{invid}
async fn add_to_inventory<T, U>(
    path: web::Path<(Snowflake,)>,
    opts: web::Json<InventoryAddOptions>,
    shared_store: web::Data<T>,
    sg: SnowflakeGeneratorState,
) -> Result<HttpResponse>
where
    T: SharedStore<Inventory, U> + SharedStore<Card, U> + Send + Sync + 'static,
    U: StoreBackend<Inventory> + StoreBackend<Card> + Send + Sync + 'static,
{
    let inv_id = path.0;
    let opts = opts.into_inner();

    let new_card = match opts {
        InventoryAddOptions::Existing(c) => c,
        InventoryAddOptions::New(type_id) => {
            let mut snowflake_gen = sg.borrow_mut();
            let c = Card::generate(snowflake_gen.deref_mut(), type_id);
            let s2 = shared_store.clone();

            web::block(move || -> Result<Card> {
                let cards: &Store<Card, U> = s2.get_store();
                cards.store(*c.id(), c.clone())?;
                Ok(c)
            })
            .await?
        }
    };

    let inv = web::block(move || -> Result<Inventory> {
        let inventories: &Store<Inventory, U> = shared_store.get_store();
        let inv_ref = inventories.load(inv_id)?;
        let mut handle = inv_ref.lock().unwrap();

        match handle.get_mut() {
            None => {
                return Err(APIError::not_found(format!(
                    "Could not find inventory {}",
                    inv_id
                )))
            }
            Some(r) => {
                r.insert(new_card);
            }
        };

        handle.store()?;
        Ok(handle.get().unwrap().clone())
    })
    .await?;

    Ok(HttpResponse::Ok().json(inv))
}

// POST /inventories
async fn create_inventory<T, U>(
    shared_store: web::Data<T>,
    sg: SnowflakeGeneratorState,
) -> Result<HttpResponse>
where
    T: SharedStore<Inventory, U> + Send + Sync + 'static,
    U: StoreBackend<Inventory> + Send + Sync + 'static,
{
    let mut snowflake_gen = sg.borrow_mut();
    let inv = Inventory::empty(snowflake_gen.generate());
    let inv_clone = inv.clone();

    web::block(move || -> Result<()> {
        let inventories: &Store<Inventory, U> = shared_store.get_store();
        inventories.store(*inv_clone.id(), inv_clone)?;
        Ok(())
    })
    .await?;

    Ok(HttpResponse::Ok().json(inv))
}

// GET /inventories/{invid}/{cardid}
async fn get_card<T, U>(
    path: web::Path<(Snowflake, Snowflake)>,
    shared_store: web::Data<T>,
) -> Result<HttpResponse>
where
    T: SharedStore<Inventory, U> + Send + Sync + 'static,
    U: StoreBackend<Inventory> + Send + Sync + 'static,
{
    let inv_id = path.0;
    let card_id = path.1;

    let res: Card = web::block(move || -> Result<Card> {
        let inventories: &Store<Inventory, U> = shared_store.get_store();
        let wrapper = inventories.load(inv_id)?;
        let handle = wrapper.lock().unwrap();

        if let Some(inv) = handle.get() {
            match inv.get(card_id) {
                Some(v) => Ok(v.clone()),
                None => Err(APIError::not_found(format!(
                    "Could not find card {} in inventory {}",
                    card_id, inv_id
                ))),
            }
        } else {
            Err(APIError::not_found(format!(
                "Could not find inventory {}",
                inv_id
            )))
        }
    })
    .await?;

    Ok(HttpResponse::Ok().json(res))
}

// DELETE /inventories/{invid}/{cardid}
async fn delete_card<T, U>(
    path: web::Path<(Snowflake, Snowflake)>,
    shared_store: web::Data<T>,
) -> Result<HttpResponse>
where
    T: SharedStore<Inventory, U> + Send + Sync + 'static,
    U: StoreBackend<Inventory> + Send + Sync + 'static,
{
    let inv_id = path.0;
    let card_id = path.1;

    let res: Card = web::block(move || -> Result<Card> {
        let inventories: &Store<Inventory, U> = shared_store.get_store();
        let wrapper = inventories.load(inv_id)?;
        let mut handle = wrapper.lock().unwrap();

        if let Some(inv) = handle.get_mut() {
            match inv.remove(card_id) {
                None => Err(APIError::not_found(format!(
                    "Could not find card {} in inventory {}",
                    card_id, inv_id
                ))),
                Some(card) => {
                    handle.store()?;
                    Ok(card)
                }
            }
        } else {
            Err(APIError::not_found(format!(
                "Could not find inventory {}",
                inv_id
            )))
        }
    })
    .await?;

    Ok(HttpResponse::Ok().json(res))
}

#[derive(Deserialize, Debug, Clone)]
struct CardMoveOptions {
    to: Snowflake,
}

// POST /inventories/{invid}/{cardid}/move
async fn move_card<T, U>(
    path: web::Path<(Snowflake, Snowflake)>,
    shared_store: web::Data<T>,
    query: web::Query<CardMoveOptions>,
) -> Result<HttpResponse>
where
    T: SharedStore<Inventory, U> + Send + Sync + 'static,
    U: StoreBackend<Inventory> + Send + Sync + 'static,
{
    let from_inv_id = path.0;
    let card_id = path.1;
    let opts = query.into_inner();

    web::block(move || -> Result<()> {
        let inventories: &Store<Inventory, U> = shared_store.get_store();

        let from_wrapper = inventories.load(from_inv_id)?;
        let mut from_handle = from_wrapper.lock().unwrap();
        let from_inv: &mut Inventory;

        if let Some(inv) = from_handle.get_mut() {
            from_inv = inv;
        } else {
            return Err(APIError::not_found(format!(
                "Could not find inventory {}",
                from_inv_id
            )));
        }

        let card: Card;
        if let Some(c) = from_inv.remove(card_id) {
            card = c;
        } else {
            return Err(APIError::not_found(format!(
                "Could not find card {} in inventory {}",
                card_id, from_inv_id
            )));
        }

        let to_wrapper = inventories.load(opts.to)?;
        let mut to_handle = to_wrapper.lock().unwrap();
        let to_inv: &mut Inventory;

        if let Some(inv) = to_handle.get_mut() {
            to_inv = inv;
        } else {
            return Err(APIError::not_found(format!(
                "Could not find inventory {}",
                opts.to
            )));
        }

        to_inv.insert(card);

        // TODO: handle rollback
        from_handle.store()?;
        to_handle.store()?;

        Ok(())
    })
    .await?;

    Ok(HttpResponse::NoContent().finish())
}

pub fn bind_routes<T, U>(scope: Scope) -> Scope
where
    T: SharedStore<Inventory, U> + SharedStore<Card, U> + Send + Sync + 'static,
    U: StoreBackend<Inventory> + StoreBackend<Card> + Send + Sync + 'static,
{
    scope
        .route("/{invid}/{cardid}/move", web::post().to(move_card::<T, U>))
        .route("/{invid}/{cardid}", web::get().to(get_card::<T, U>))
        .route("/{invid}/{cardid}", web::delete().to(delete_card::<T, U>))
        .route("/{invid}", web::post().to(add_to_inventory::<T, U>))
        .route("/{invid}", web::get().to(get_inventory::<T, U>))
        .route("", web::post().to(create_inventory::<T, U>))
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::http;
    use futures::executor::block_on;

    use crate::api::utils;
    use crate::api::utils::{get_body_json, snowflake_generator, store};
    use crate::local_storage::SharedLocalStore;
    use crate::snowflake::SnowflakeGenerator;

    #[test]
    fn test_get_inventory() {
        let shared_store = SharedLocalStore::new();
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);

        let inv = Inventory::empty(snowflake_gen.generate());
        let inv_store = shared_store.inventories();

        inv_store.store(*inv.id(), inv.clone()).unwrap();

        let resp = block_on(get_inventory(
            web::Path::from((*inv.id(),)),
            web::Data::new(shared_store),
        ))
        .unwrap();

        assert_eq!(resp.status(), http::StatusCode::OK);

        let body: Inventory = get_body_json(&resp);
        assert_eq!(body, inv);
    }

    #[test]
    fn test_get_inventory_not_exists() {
        let shared_store = SharedLocalStore::new();
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);

        let resp = block_on(get_inventory(
            web::Path::from((snowflake_gen.generate(),)),
            web::Data::new(shared_store),
        ));
        utils::expect_not_found(resp);
    }

    #[test]
    fn test_create_inventory() {
        let shared_store = store();
        let sg = snowflake_generator(0, 0);

        let resp = block_on(create_inventory(shared_store.clone(), sg)).unwrap();

        let inventories = shared_store.inventories();
        let keys = inventories.keys(0, 20).unwrap();

        assert_eq!(keys.len(), 1);

        let wrapper = inventories.load(keys[0]).unwrap();
        let handle = wrapper.lock().unwrap();

        let body: Inventory = get_body_json(&resp);
        let stored_card = handle.get().unwrap();
        assert_eq!(body, *stored_card);
        assert_eq!(resp.status(), http::StatusCode::OK);
    }

    #[test]
    fn test_add_new_card() {
        let shared_store = store();
        let sg = snowflake_generator(0, 0);
        let type_id: Snowflake;
        let inv: Inventory;

        {
            let mut snowflake_gen = sg.borrow_mut();
            type_id = snowflake_gen.generate();
            inv = Inventory::empty(snowflake_gen.generate());
        }

        let inv_store = shared_store.inventories();
        let card_store = shared_store.cards();

        let inv_id = *inv.id();
        inv_store.store(inv_id, inv).unwrap();

        let resp = block_on(add_to_inventory(
            web::Path::from((inv_id,)),
            web::Json(InventoryAddOptions::New(type_id)),
            shared_store.clone(),
            sg,
        ))
        .unwrap();

        assert_eq!(resp.status(), http::StatusCode::OK);

        let body: Inventory = get_body_json(&resp);
        let wrapper = inv_store.load(inv_id).unwrap();
        let handle = wrapper.lock().unwrap();
        let stored_inv = handle.get().unwrap();

        assert_eq!(body, *stored_inv);
        assert_eq!(stored_inv.len(), 1);

        let card: &Card = stored_inv.iter().nth(0).unwrap();
        assert_eq!(*card.type_id(), type_id);

        let wrapper = card_store.load(*card.id()).unwrap();
        let handle = wrapper.lock().unwrap();
        let stored_card = handle.get().unwrap();

        assert_eq!(*card, *stored_card);
    }

    #[test]
    fn test_add_existing_card() {
        let shared_store = store();
        let sg = snowflake_generator(0, 0);
        let card: Card;
        let inv: Inventory;

        {
            let mut snowflake_gen = sg.borrow_mut();
            card = utils::generate_random_card(snowflake_gen.deref_mut());
            inv = Inventory::empty(snowflake_gen.generate());
        }

        let inv_store = shared_store.inventories();
        let inv_id = *inv.id();
        inv_store.store(inv_id, inv).unwrap();

        let resp = block_on(add_to_inventory(
            web::Path::from((inv_id,)),
            web::Json(InventoryAddOptions::Existing(card.clone())),
            shared_store.clone(),
            sg,
        ))
        .unwrap();

        assert_eq!(resp.status(), http::StatusCode::OK);

        let body: Inventory = get_body_json(&resp);
        let wrapper = inv_store.load(inv_id).unwrap();
        let handle = wrapper.lock().unwrap();
        let stored_inv = handle.get().unwrap();

        assert_eq!(body, *stored_inv);
        assert_eq!(stored_inv.len(), 1);

        let inv_card: &Card = stored_inv.iter().nth(0).unwrap();
        assert_eq!(*inv_card, card);
    }

    #[test]
    fn test_get_card() {
        let shared_store = store();
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let mut inv = Inventory::empty(snowflake_gen.generate());

        let card = utils::generate_random_card(&mut snowflake_gen);
        inv.insert(card.clone());

        let inv_store = shared_store.inventories();
        let inv_id = *inv.id();
        let card_id = *card.id();

        inv_store.store(inv_id, inv).unwrap();
        let resp = block_on(get_card(web::Path::from((inv_id, card_id)), shared_store)).unwrap();

        assert_eq!(resp.status(), http::StatusCode::OK);

        let body: Card = get_body_json(&resp);
        assert_eq!(body, card);
    }

    #[test]
    fn test_get_card_not_exists() {
        let shared_store = store();
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let inv = Inventory::empty(snowflake_gen.generate());
        let inv_id = *inv.id();

        let test_id = snowflake_gen.generate();
        let resp = block_on(get_card(
            web::Path::from((inv_id, test_id)),
            shared_store.clone(),
        ));
        utils::expect_not_found(resp);

        let inv_store = shared_store.inventories();
        inv_store.store(inv_id, inv).unwrap();

        let test_id = snowflake_gen.generate();
        let resp = block_on(get_card(web::Path::from((inv_id, test_id)), shared_store));
        utils::expect_not_found(resp);
    }

    #[test]
    fn test_delete_card() {
        let shared_store = store();
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let mut inv = Inventory::empty(snowflake_gen.generate());
        let card = utils::generate_random_card(&mut snowflake_gen);
        let inv_id = *inv.id();
        let card_id = *card.id();

        inv.insert(card.clone());
        assert_eq!(inv.len(), 1);

        let inv_store = shared_store.inventories();
        inv_store.store(inv_id, inv).unwrap();

        let resp = block_on(delete_card(
            web::Path::from((inv_id, card_id)),
            shared_store.clone(),
        ))
        .unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);

        let body: Card = get_body_json(&resp);
        assert_eq!(body, card);

        let wrapper = inv_store.load(inv_id).unwrap();
        let handle = wrapper.lock().unwrap();
        let inv = handle.get().unwrap();
        assert_eq!(inv.len(), 0);
    }

    #[test]
    fn test_delete_card_not_exists() {
        let shared_store = store();
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let inv = Inventory::empty(snowflake_gen.generate());
        let inv_id = *inv.id();

        let test_id = snowflake_gen.generate();
        let resp = block_on(delete_card(
            web::Path::from((inv_id, test_id)),
            shared_store.clone(),
        ));
        utils::expect_not_found(resp);

        let inv_store = shared_store.inventories();
        inv_store.store(inv_id, inv).unwrap();

        let test_id = snowflake_gen.generate();
        let resp = block_on(delete_card(
            web::Path::from((inv_id, test_id)),
            shared_store,
        ));
        utils::expect_not_found(resp);
    }

    #[test]
    fn test_move_card() {
        let shared_store = store();
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let card = utils::generate_random_card(&mut snowflake_gen);
        let card_id = *card.id();

        let mut src_inv = Inventory::empty(snowflake_gen.generate());
        let src_id = *src_inv.id();
        src_inv.insert(card.clone());
        assert_eq!(src_inv.len(), 1);

        let dest_inv = Inventory::empty(snowflake_gen.generate());
        let dest_id = *dest_inv.id();

        let inv_store = shared_store.inventories();
        inv_store.store(src_id, src_inv).unwrap();
        inv_store.store(dest_id, dest_inv).unwrap();

        let query_str = format!("to={}", dest_id);
        let query = web::Query::<CardMoveOptions>::from_query(query_str.as_str()).unwrap();

        let resp = block_on(move_card(
            web::Path::from((src_id, card_id)),
            shared_store.clone(),
            query,
        ))
        .unwrap();

        assert_eq!(resp.status(), http::StatusCode::NO_CONTENT);

        let src_wrapper = inv_store.load(src_id).unwrap();
        let src_handle = src_wrapper.lock().unwrap();
        let src_inv: &Inventory = src_handle.get().unwrap();

        let dest_wrapper = inv_store.load(dest_id).unwrap();
        let dest_handle = dest_wrapper.lock().unwrap();
        let dest_inv: &Inventory = dest_handle.get().unwrap();

        assert_eq!(src_inv.len(), 0);
        assert_eq!(dest_inv.len(), 1);

        let loaded_card = dest_inv.iter().nth(0).unwrap();
        assert_eq!(card, *loaded_card);
    }

    #[test]
    fn test_move_card_nonexistent_dest() {
        let shared_store = store();
        let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
        let card = utils::generate_random_card(&mut snowflake_gen);
        let card_id = *card.id();

        let mut src_inv = Inventory::empty(snowflake_gen.generate());
        let src_id = *src_inv.id();
        src_inv.insert(card.clone());
        assert_eq!(src_inv.len(), 1);

        let inv_store = shared_store.inventories();
        inv_store.store(src_id, src_inv).unwrap();

        let query = web::Query::<CardMoveOptions>::from_query("to=1").unwrap();

        let resp = block_on(move_card(
            web::Path::from((src_id, card_id)),
            shared_store.clone(),
            query,
        ));
        utils::expect_not_found(resp);

        let src_wrapper = inv_store.load(src_id).unwrap();
        let src_handle = src_wrapper.lock().unwrap();
        let src_inv: &Inventory = src_handle.get().unwrap();

        assert_eq!(src_inv.len(), 1);

        let loaded_card = src_inv.iter().nth(0).unwrap();
        assert_eq!(card, *loaded_card);
    }
}
