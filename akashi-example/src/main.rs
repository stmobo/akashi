#[macro_use]
extern crate failure;

use actix_rt;
use actix_web::{web, App, HttpServer};

use std::sync::{Arc, Mutex};

mod inventory;
mod models;
mod player;
mod utils;

use akashi::local_storage::SharedLocalStore;
use akashi::SnowflakeGenerator;

const BIND_URL: &str = "127.0.0.1:8088";

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    let shared_store = web::Data::new(SharedLocalStore::new());
    let cm = web::Data::new(utils::new_component_manager(&shared_store));
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

        let snowflake_gen = web::Data::new(Mutex::new(SnowflakeGenerator::new(0, id)));

        println!("Started thread {}!", id);
        let players_scope = player::bind_routes(
            web::scope("/players"),
            shared_store.clone(),
            snowflake_gen.clone(),
            cm.clone(),
        );

        let inv_scope = inventory::bind_routes(
            web::scope("/inventories"),
            shared_store.clone(),
            snowflake_gen.clone(),
            cm.clone(),
        );

        App::new().service(players_scope).service(inv_scope)
    })
    .bind("127.0.0.1:8088")
    .unwrap()
    .run()
    .await
}
