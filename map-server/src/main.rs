mod api;
mod server;

use api::map_service::SHUTDOWN_TX;

use common::proto::game_service::game_service_server::GameServiceServer;
use common::proto::map_service::map_service_server::MapServiceServer;
use common::MAP_PORT_ENV_NAME;

use tonic::transport::Server;
use tracing::info;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let port =
        std::env::var(MAP_PORT_ENV_NAME).unwrap_or_else(|_| panic!("Need env {MAP_PORT_ENV_NAME}"));
    let addr = format!("127.0.0.1:{}", port);
    let socket = addr.parse().unwrap();
    info!("starting at {addr}");

    let server_id = std::env::var(MAP_PORT_ENV_NAME)
        .map(|s| s.parse().unwrap())
        .unwrap_or(1);

    let map_server = server::MapServer::new(server_id, addr);
    let (otx, orx) = tokio::sync::oneshot::channel();
    // Safety: 用一次就退出
    unsafe { SHUTDOWN_TX.get_or_init(|| otx) };
    Server::builder()
        .add_service(MapServiceServer::new(map_server.clone()))
        .add_service(GameServiceServer::new(map_server))
        .serve_with_shutdown(socket, async move { orx.await.unwrap() })
        .await
        .unwrap();

    info!("exit");
}
