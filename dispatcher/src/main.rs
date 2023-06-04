use std::sync::Arc;

use tonic::transport::Server;
use tracing::info;

use crate::dispatcher::Dispatcher;
use crate::grpc::game_service::game_service_server::GameServiceServer;

mod dispatcher;
mod grpc;
mod server_scaling;
mod util;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    info!("starting");

    let addr = "[::1]:50051".parse().unwrap();
    let dispatcher = dispatcher::Dispatcher::new().await.unwrap();
    Server::builder()
        .add_service(GameServiceServer::new(Arc::new(dispatcher)))
        .serve(addr)
        .await
        .unwrap();

    info!("gRPC server start");
}
