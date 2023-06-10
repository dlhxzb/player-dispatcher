mod data;
mod dispatcher;
mod game_service;
mod server_scaling;
mod util;

use common::proto::game_service::game_service_server::GameServiceServer;

use tonic::transport::Server;
use tracing::info;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().pretty().init();
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

    info!("exit");
}
