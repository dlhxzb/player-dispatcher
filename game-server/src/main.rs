mod data;
mod dispatcher;
mod game_service;
mod server_scaling;
mod util;

use common::proto::game_service::game_service_server::GameServiceServer;
use common::{DEFAULT_GAME_PORT, GAME_PORT_ENV_NAME};
use tonic::transport::Server;
use tracing::*;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().pretty().init();

    let port = std::env::var(GAME_PORT_ENV_NAME).unwrap_or_else(|_| DEFAULT_GAME_PORT.to_string());
    let addr = format!("[::1]:{}", port).parse().unwrap();
    info!("starting at {addr}");
    // Set ert worker count.
    ert::prelude::Router::new(10_000).set_as_global();

    let dispatcher = dispatcher::Dispatcher::new().await.unwrap();
    tokio::spawn(dispatcher.clone().scaling_moniter());
    Server::builder()
        .add_service(GameServiceServer::new(dispatcher.clone()))
        .serve(addr)
        .await
        .unwrap();

    dispatcher.shutdown_all_map_server().await;
    info!("exit");
}
