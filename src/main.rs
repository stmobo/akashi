use actix_rt;
use actix_web::{web, App, HttpServer};

use std::cell::RefCell;
use std::sync::{Arc, Mutex};

use akashi::api;
use akashi::local_storage::{LocalStoreBackend, SharedLocalStore};
use akashi::snowflake::SnowflakeGenerator;

const BIND_URL: &str = "127.0.0.1:8088";

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    let shared_store = web::Data::new(SharedLocalStore::new());
    let ctr: Arc<Mutex<u64>> = Arc::new(Mutex::new(0));

    println!("Akashi starting on {}...", BIND_URL);

    HttpServer::new(move || {
        let id: u64;
        let ctr_ref = ctr.clone();

        {
            let mut r = ctr_ref.lock().unwrap();
            id = *r;
            *r += 1;
        }

        println!("Started thread {}!", id);

        let players_scope = web::scope("/players")
            .app_data(shared_store.clone())
            .data(RefCell::new(SnowflakeGenerator::new(0, id)));
        let players_scope =
            api::player::bind_routes::<SharedLocalStore, LocalStoreBackend>(players_scope);

        let inv_scope = web::scope("/inventories")
            .app_data(shared_store.clone())
            .data(RefCell::new(SnowflakeGenerator::new(1, id)));
        let inv_scope =
            api::inventory::bind_routes::<SharedLocalStore, LocalStoreBackend>(inv_scope);

        App::new().service(players_scope).service(inv_scope)
    })
    .bind("127.0.0.1:8088")
    .unwrap()
    .run()
    .await
}
