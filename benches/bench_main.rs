mod benchmarks;

use criterion::criterion_main;

use std::sync::atomic::AtomicU64;

static LOGINED_PLAYER_ID: AtomicU64 = AtomicU64::new(0);

criterion_main! {
    benchmarks::login::login,
    benchmarks::moving::moving,
    benchmarks::query::querys,
    benchmarks::aoe::aoe,
}
