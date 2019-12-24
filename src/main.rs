use actix_web::{web, App, HttpServer};
use std::cell::RefCell;
use std::sync::{Arc, Mutex};

use akashi::local_storage::{LocalStoreBackend, SharedLocalStore};
use akashi::router;
use akashi::snowflake::SnowflakeGenerator;

fn main() {
    let shared_store = web::Data::new(SharedLocalStore::new());
    let ctr: Arc<Mutex<u64>> = Arc::new(Mutex::new(0));

    HttpServer::new(move || {
        let id: u64;
        let ctr_ref = ctr.clone();

        {
            let mut r = ctr_ref.lock().unwrap();
            id = *r;
            *r += 1;
        }

        // println!("thread {} starting", id);

        let snowflake_gen = SnowflakeGenerator::new(0, id);
        let scope = web::scope("/players")
            .register_data(shared_store.clone())
            .data(RefCell::new(snowflake_gen));
        let scope = router::bind_routes::<SharedLocalStore, LocalStoreBackend>(scope);

        App::new().service(scope)
    })
    .bind("127.0.0.1:8088")
    .unwrap()
    .run()
    .unwrap();
}
