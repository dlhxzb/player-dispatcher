use game_server::dispatcher::Dispatcher;
use game_server::util::Config;

use common::proto::game_service::{game_service_client::GameServiceClient, PlayerInfo};
use common::DEFAULT_GAME_PORT;

use criterion::{async_executor, criterion_group, criterion_main, BenchmarkId, Criterion};
use futures::StreamExt;
use tokio::runtime::Builder;
use tonic::IntoRequest;

use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

fn from_elem(c: &mut Criterion) {
    let id = Arc::new(AtomicU64::new(1));
    let runtime = Builder::new_multi_thread().enable_all().build().unwrap();

    let rpc_cli = runtime
        .block_on(GameServiceClient::connect(format!(
            "http://127.0.0.1:{DEFAULT_GAME_PORT}"
        )))
        .unwrap();

    c.bench_function("login", |b| {
        b.to_async(&runtime).iter(|| {
            let mut rpc_cli = rpc_cli.clone();
            let id = id.fetch_add(1, Ordering::Relaxed);
            println!("{id}");
            async move {
                // rpc_cli
                //     .login(
                //         PlayerInfo {
                //             player_id: id as u64,
                //             x: -100.0,
                //             y: id as f32,
                //             money: 99,
                //         }
                //         .into_request(),
                //     )
                //     .await
                //     .unwrap();
            }
        });
    });
}

criterion_group! {
    name = login;
    config = Criterion::default().sample_size(1000);
    targets = from_elem
}
