extern crate num_cpus;
extern crate rayon;

use akashi::ecs::entity_store::{StoreHandle, StoreReference};
use akashi::{Card, ComponentManager, Entity, EntityBackend, Snowflake, SnowflakeGenerator, Store};
use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput};
use crossbeam::queue::SegQueue;
use failure::Error;
use rayon::prelude::*;
use std::sync::Arc;
use std::time::Duration;

const N_ELEMS: u64 = 4 * 1024;

struct NullBackend {}

impl<T> EntityBackend<T> for NullBackend
where
    T: Entity + 'static,
{
    fn exists(&self, _id: Snowflake) -> Result<bool, Error> {
        Ok(false)
    }

    fn load(&self, _id: Snowflake, _cm: Arc<ComponentManager<T>>) -> Result<Option<T>, Error> {
        Ok(None)
    }

    fn store(&self, _id: Snowflake, _obj: &T) -> Result<(), Error> {
        Ok(())
    }

    fn delete(&self, _id: Snowflake) -> Result<(), Error> {
        Ok(())
    }

    fn keys(&self, _page: u64, _limit: u64) -> Result<Vec<Snowflake>, Error> {
        Ok(vec![])
    }
}

pub fn store_test(c: &mut Criterion) {
    let mut group = c.benchmark_group("store_test");
    group.throughput(Throughput::Elements(N_ELEMS));

    let max = num_cpus::get();
    for threads in 1..=max {
        group.bench_with_input(
            BenchmarkId::from_parameter(threads),
            &threads,
            |b, &threads| {
                let mut snowflake_gen = SnowflakeGenerator::new(0, 0);
                let cm: Arc<ComponentManager<Card>> = Arc::new(ComponentManager::new());

                let pool = rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap();

                pool.install(|| {
                    b.iter_batched(
                        || {
                            let cards: Vec<Card> = (0..N_ELEMS)
                                .map(|_x| Card::generate(&mut snowflake_gen, cm.clone()))
                                .collect();

                            let backend = NullBackend {};
                            let store = Store::new(Arc::new(backend));
                            let out_q: SegQueue<StoreReference<StoreHandle<Card>>> =
                                SegQueue::new();

                            (cards, store, Arc::new(out_q))
                        },
                        |(cards, store, out_q)| {
                            cards.into_par_iter().for_each(|card| {
                                let out_q = out_q.clone();

                                let r = store.load_handle(card.id(), cm.clone()).unwrap();
                                let mut lock = r.write();
                                lock.replace(card);
                                lock.store().unwrap();
                                drop(lock);

                                out_q.push(r);
                            });

                            out_q
                        },
                        BatchSize::SmallInput,
                    );
                });
            },
        );
    }
}

criterion_group! {
    name = benches;
    config = Criterion::default().measurement_time(Duration::from_secs(60));
    targets = store_test
}

criterion_main!(benches);
