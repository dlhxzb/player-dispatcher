use common::proto::game_service::game_service_client::GameServiceClient;
use common::proto::game_service::MovingRequest;
use common::DEFAULT_GAME_PORT;

use criterion::{criterion_group, Criterion};
use rand::{thread_rng, Rng};
use tokio::runtime::Builder;
use tonic::IntoRequest;

use std::sync::atomic::{AtomicU64, Ordering};

use crate::LOGINED_PLAYER_ID;

fn from_elem(c: &mut Criterion) {
    let runtime = Builder::new_multi_thread().enable_all().build().unwrap();
    let rpc_cli = runtime
        .block_on(GameServiceClient::connect(format!(
            "http://127.0.0.1:{DEFAULT_GAME_PORT}"
        )))
        .unwrap();
    let mut rng = thread_rng();
    let max_player_id = LOGINED_PLAYER_ID.load(Ordering::Relaxed);
    let acc: AtomicU64 = AtomicU64::new(0);
    let now = std::time::Instant::now();

    c.bench_function("moving", |b| {
        b.to_async(&runtime).iter(|| {
            acc.fetch_add(1, Ordering::Relaxed);
            let mut rpc_cli = rpc_cli.clone();
            let player_id = rng.gen_range(1..max_player_id);
            let dx = rng.gen_range(-5.0..5.0);
            let dy = rng.gen_range(-5.0..5.0);
            async move {
                rpc_cli
                    .moving(MovingRequest { player_id, dx, dy }.into_request())
                    .await
                    .unwrap();
            }
        });
    });

    let elapsed = now.elapsed().as_millis();
    let acc = acc.load(Ordering::Relaxed);
    println!(
        "Moving {acc} times in {elapsed}ms, RPS: {}",
        (acc * 1000) as u128 / elapsed
    );
}

criterion_group! {
    name = moving;
    config = Criterion::default();
    targets = from_elem
}
