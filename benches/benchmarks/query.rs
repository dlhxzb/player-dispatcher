use common::proto::game_service::game_service_client::GameServiceClient;
use common::proto::game_service::QueryRequest;
use common::{DEFAULT_GAME_PORT, WORLD_X_MAX, WORLD_Y_MAX};

use criterion::{criterion_group, Criterion};
use tokio::runtime::Builder;
use tonic::IntoRequest;

use std::sync::atomic::{AtomicU64, Ordering};

fn query(c: &mut Criterion, x: f32, y: f32) {
    let runtime = Builder::new_multi_thread().enable_all().build().unwrap();
    let rpc_cli = runtime
        .block_on(GameServiceClient::connect(format!(
            "http://127.0.0.1:{DEFAULT_GAME_PORT}"
        )))
        .unwrap();
    let acc: AtomicU64 = AtomicU64::new(0);
    let now = std::time::Instant::now();

    c.bench_function("query", |b| {
        b.to_async(&runtime).iter(|| {
            acc.fetch_add(1, Ordering::Relaxed);
            let mut rpc_cli = rpc_cli.clone();
            async move {
                rpc_cli
                    .query(
                        QueryRequest {
                            xmin: -x,
                            xmax: x,
                            ymin: -y,
                            ymax: y,
                        }
                        .into_request(),
                    )
                    .await
                    .unwrap();
            }
        });
    });

    let elapsed = now.elapsed().as_millis();
    let acc = acc.load(Ordering::Relaxed);
    println!(
        "Query x:{x},y:{y} {acc} times in {elapsed}ms, RPS: {}",
        (acc * 1000) as u128 / elapsed
    );
}

fn small_query(c: &mut Criterion) {
    // query(c, 100.0, 100.0);
    big_query();
}

fn big_query() {
    let x = WORLD_X_MAX;
    let y = WORLD_Y_MAX;
    let runtime = Builder::new_multi_thread().enable_all().build().unwrap();
    let mut rpc_cli = runtime
        .block_on(GameServiceClient::connect(format!(
            "http://127.0.0.1:{DEFAULT_GAME_PORT}"
        )))
        .unwrap();
    let now = std::time::Instant::now();
    let res = runtime
        .block_on(
            rpc_cli.query(
                QueryRequest {
                    xmin: -x,
                    xmax: x,
                    ymin: -y,
                    ymax: y,
                }
                .into_request(),
            ),
        )
        .unwrap()
        .into_inner()
        .infos;
    let elapsed = now.elapsed().as_millis();
    println!("Query {} players in full map in {elapsed}ms", res.len());
}

criterion_group! {
    name = querys;
    // config = Criterion::default().measurement_time(std::time::Duration::from_secs(100));
    config = Criterion::default();
    targets = small_query
}
