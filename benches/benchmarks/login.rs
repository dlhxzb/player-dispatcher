use crate::LOGINED_PLAYER_ID;

use common::proto::game_service::{game_service_client::GameServiceClient, PlayerInfo};
use common::{DEFAULT_GAME_PORT, WORLD_X_MAX, WORLD_X_MIN, WORLD_Y_MAX, WORLD_Y_MIN};

use criterion::{criterion_group, Criterion};
use rand::{thread_rng, Rng};
use tokio::runtime::Builder;
use tonic::IntoRequest;

use std::sync::atomic::Ordering;

fn from_elem(c: &mut Criterion) {
    let runtime = Builder::new_multi_thread().enable_all().build().unwrap();
    let rpc_cli = runtime
        .block_on(GameServiceClient::connect(format!(
            "http://127.0.0.1:{DEFAULT_GAME_PORT}"
        )))
        .unwrap();
    let mut rng = thread_rng();
    let now = std::time::Instant::now();

    c.bench_function("login", |b| {
        b.to_async(&runtime).iter(|| {
            let mut rpc_cli = rpc_cli.clone();
            let x = rng.gen_range(WORLD_X_MIN..WORLD_X_MAX);
            let y = rng.gen_range(WORLD_Y_MIN..WORLD_Y_MAX);
            async move {
                rpc_cli
                    .login(
                        PlayerInfo {
                            player_id: LOGINED_PLAYER_ID.fetch_add(1, Ordering::Relaxed),
                            x,
                            y,
                            money: 99,
                        }
                        .into_request(),
                    )
                    .await
                    .unwrap();
            }
        });
    });

    let elapsed = now.elapsed().as_millis();
    let players = LOGINED_PLAYER_ID.load(Ordering::Relaxed);
    println!(
        "Logined {players} players in {elapsed}ms, RPS: {}",
        (players * 1000) as u128 / elapsed
    );
}

criterion_group! {
    name = login;
    config = Criterion::default();
    targets = from_elem
}
