mod api;
mod server;

use common::proto::game_service::game_service_server::GameServiceServer;
use common::proto::map_service::map_service_server::MapServiceServer;

use tonic::transport::Server;
use tracing::info;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().pretty().init();
    info!("starting");

    let addr = "[::1]:50052".parse().unwrap();
    let map_server = server::MapServer::new(1);
    Server::builder()
        .add_service(MapServiceServer::new(map_server.clone()))
        .add_service(GameServiceServer::new(map_server))
        .serve(addr)
        .await
        .unwrap();

    info!("exit");
}
