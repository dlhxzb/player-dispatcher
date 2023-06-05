mod dispatcher;
mod game_impl;
mod server_scaling;
mod util;

pub use game_server::*;

use common::proto::game_service::game_service_server::GameServiceServer;

use tonic::transport::Server;
use tracing::info;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    info!("starting");
    // Set ert worker count.
    ert::prelude::Router::new(10_000).set_as_global();

    let addr = "[::1]:50051".parse().unwrap();
    let dispatcher = dispatcher::Dispatcher::new().await.unwrap();
    Server::builder()
        .add_service(GameServiceServer::new(dispatcher))
        .serve(addr)
        .await
        .unwrap();

    info!("gRPC server start");
}
