use actix_web::{error, web, HttpResponse, Result, Scope};
use std::ops::DerefMut;

use serde::Deserialize;

use crate::card::{Card, Inventory};
use crate::snowflake::Snowflake;
use crate::store::{SharedStore, Store, StoreBackend};

use super::utils::SnowflakeGeneratorState;

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
    let store: &Store<Inventory, U> = shared_store.get_store();

    let inv_ref = store.load(id).map_err(error::ErrorInternalServerError)?;
    {
        let handle = inv_ref.lock().unwrap();
        match handle.get() {
            None => Ok(HttpResponse::NotFound()
                .content_type("plain/text")
                .body(format!("Could not find inventory {}", id))),
            Some(r) => Ok(HttpResponse::Ok().json(r)),
        }
    }
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
    let mut snowflake_gen = sg.borrow_mut();
    let cards: &Store<Card, U> = shared_store.get_store();
    let opts = opts.into_inner();

    let new_card = match opts {
        InventoryAddOptions::Existing(c) => c,
        InventoryAddOptions::New(type_id) => {
            let c = Card::generate(snowflake_gen.deref_mut(), type_id);
            cards
                .store(*c.id(), c.clone())
                .map_err(error::ErrorInternalServerError)?;
            c
        }
    };

    let inventories: &Store<Inventory, U> = shared_store.get_store();
    {
        let inv_ref = inventories
            .load(inv_id)
            .map_err(error::ErrorInternalServerError)?;
        let mut handle = inv_ref.lock().unwrap();

        match handle.get_mut() {
            None => {
                return Ok(HttpResponse::NotFound()
                    .content_type("plain/text")
                    .body(format!("Could not find inventory {}", inv_id)))
            }
            Some(r) => {
                r.insert(new_card);
            }
        };

        handle.store().map_err(error::ErrorInternalServerError)?;
        Ok(HttpResponse::Ok().json(handle.get().unwrap()))
    }
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
    let inventories: &Store<Inventory, U> = shared_store.get_store();
    let inv = Inventory::empty(snowflake_gen.generate());

    inventories
        .store(*inv.id(), inv.clone())
        .map_err(error::ErrorInternalServerError)?;
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

    let inventories: &Store<Inventory, U> = shared_store.get_store();
    let wrapper = inventories
        .load(inv_id)
        .map_err(error::ErrorInternalServerError)?;
    let handle = wrapper.lock().unwrap();

    let inv: &Inventory;
    if let Some(v) = handle.get() {
        inv = v;
    } else {
        return Ok(HttpResponse::NotFound()
            .content_type("plain/text")
            .body(format!("Could not find inventory {}", inv_id)));
    }

    match inv.get(card_id) {
        None => Ok(HttpResponse::NotFound()
            .content_type("plain/text")
            .body(format!(
                "Could not find card {} in inventory {}",
                card_id, inv_id
            ))),
        Some(card) => Ok(HttpResponse::Ok().json(card)),
    }
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

    let inventories: &Store<Inventory, U> = shared_store.get_store();
    let wrapper = inventories
        .load(inv_id)
        .map_err(error::ErrorInternalServerError)?;
    let mut handle = wrapper.lock().unwrap();

    let inv: &mut Inventory;
    if let Some(v) = handle.get_mut() {
        inv = v;
    } else {
        return Ok(HttpResponse::NotFound()
            .content_type("plain/text")
            .body(format!("Could not find inventory {}", inv_id)));
    }

    match inv.remove(card_id) {
        None => Ok(HttpResponse::NotFound()
            .content_type("plain/text")
            .body(format!(
                "Could not find card {} in inventory {}",
                card_id, inv_id
            ))),
        Some(card) => Ok(HttpResponse::Ok().json(card)),
    }
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
    let inventories: &Store<Inventory, U> = shared_store.get_store();

    let from_wrapper = inventories
        .load(from_inv_id)
        .map_err(error::ErrorInternalServerError)?;
    let mut from_handle = from_wrapper.lock().unwrap();
    let from_inv: &mut Inventory;

    if let Some(inv) = from_handle.get_mut() {
        from_inv = inv;
    } else {
        return Ok(HttpResponse::NotFound()
            .content_type("plain/text")
            .body(format!("Could not find inventory {}", from_inv_id)));
    }

    let card: Card;
    if let Some(c) = from_inv.remove(card_id) {
        card = c;
    } else {
        return Ok(HttpResponse::NotFound()
            .content_type("plain/text")
            .body(format!(
                "Could not find card {} in inventory {}",
                card_id, from_inv_id
            )));
    }

    let to_wrapper = inventories
        .load(opts.to)
        .map_err(error::ErrorInternalServerError)?;
    let mut to_handle = to_wrapper.lock().unwrap();
    let to_inv: &mut Inventory;

    if let Some(inv) = to_handle.get_mut() {
        to_inv = inv;
    } else {
        return Ok(HttpResponse::NotFound()
            .content_type("plain/text")
            .body(format!("Could not find inventory {}", opts.to)));
    }

    to_inv.insert(card);

    // TODO: handle rollback
    from_handle
        .store()
        .map_err(error::ErrorInternalServerError)?;
    to_handle.store().map_err(error::ErrorInternalServerError)?;

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
