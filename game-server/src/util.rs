use crate::data::*;

use common::proto::game_service::game_service_client::GameServiceClient;
use common::proto::map_service::map_service_client::MapServiceClient;
use common::{ServerId, ZoneId, DEFAULT_MAP_PORT, MAP_PORT_ENV_NAME};

use anyhow::Result;
use common::{WORLD_X_MAX, WORLD_X_MIN, WORLD_Y_MAX, WORLD_Y_MIN};
use econf::LoadEnv;
use tonic::Status;
use tracing::*;

use std::sync::atomic::{AtomicU32, Ordering};

#[derive(Debug, LoadEnv)]
pub struct Config {
    pub max_players: u32,      // 扩容阈值
    pub min_players: u32,      // 缩容阈值
    pub max_zone_depth: u32,   // 四叉树最大高度
    pub scaling_interval: u64, // 扩缩容扫描间隔(ms)
}

pub fn check_xy_range(x: f32, y: f32) -> Result<(), Status> {
    if x >= WORLD_X_MAX || y >= WORLD_Y_MAX || x <= WORLD_X_MIN || y < WORLD_Y_MIN {
        Err(Status::out_of_range(format!("x:{x} y:{y}")))
    } else {
        Ok(())
    }
}

#[inline]
pub fn get_child_zone_ids(id: ZoneId) -> [ZoneId; 4] {
    [id * 10 + 1, id * 10 + 2, id * 10 + 3, id * 10 + 4]
}

pub fn gen_server_id() -> ServerId {
    static SERVER_ID: AtomicU32 = AtomicU32::new(0);
    SERVER_ID.fetch_add(1, Ordering::Relaxed)
}

// 从MAP_PORT_ENV_NAME开始，每次获取逐个+1
pub fn gen_port_no() -> u32 {
    use once_cell::sync::Lazy;
    use std::env;

    static PORT: Lazy<AtomicU32> = Lazy::new(|| {
        let port = env::var(MAP_PORT_ENV_NAME)
            .map(|s| s.parse().unwrap())
            .unwrap_or(DEFAULT_MAP_PORT);
        AtomicU32::new(port)
    });
    PORT.fetch_add(1, Ordering::Relaxed)
}

// 启动独立的bin
#[cfg(not(feature = "map_server_inside"))]
#[instrument]
pub async fn start_map_server(zones: Vec<ZoneId>) -> Result<ServerInfo> {
    use anyhow::Context;
    use std::env;
    use std::process::Command;
    use tokio::time::{sleep, Duration};

    let port = gen_port_no();
    let addr = format!("http://127.0.0.1:{port}");

    let map_bin_path = env::var("MAP_SERVER_BIN_PATH").expect("Please set env MAP_SERVER_BIN_PATH");
    Command::new(&map_bin_path)
        .env(MAP_PORT_ENV_NAME, port.to_string())
        .spawn()
        .with_context(|| format!("Failed to start {map_bin_path}"))?;
    sleep(Duration::from_millis(500)).await;

    let map_cli = MapServiceClient::connect(addr.clone()).await?;
    let game_cli = GameServiceClient::connect(addr.clone()).await?;
    let server_id = gen_server_id();

    info!(?server_id, ?addr);
    Ok(ServerInfo {
        inner: ServerInfoInner {
            server_id,
            zones,
            map_cli,
            game_cli,
            addr,
        }
        .into(),
    })
}

// 以对象形式加载。测试用
#[cfg(feature = "map_server_inside")]
#[instrument]
pub async fn start_map_server(zones: Vec<ZoneId>) -> Result<ServerInfo> {
    use common::proto::game_service::game_service_server::GameServiceServer;
    use common::proto::map_service::map_service_server::MapServiceServer;
    use tokio::time::{sleep, Duration};
    use tonic::transport::Server;

    let port = gen_port_no();
    let addr = format!("127.0.0.1:{port}");
    let socket = addr.parse().unwrap();

    let server_id = gen_server_id();
    let map_server = map_server::server::MapServer::new(server_id, addr.clone());
    tokio::spawn(
        Server::builder()
            .add_service(MapServiceServer::new(map_server.clone()))
            .add_service(GameServiceServer::new(map_server))
            .serve(socket),
    );
    sleep(Duration::from_millis(100)).await;

    let addr = format!("http://{}", addr);
    let map_cli = MapServiceClient::connect(addr.clone()).await?;
    let game_cli = GameServiceClient::connect(addr.clone()).await?;

    info!(?server_id, ?addr);
    Ok(ServerInfo {
        inner: ServerInfoInner {
            server_id,
            zones,
            map_cli,
            game_cli,
            addr,
        }
        .into(),
    })
}

pub async fn shutdown_map_server(server: &ServerInfo) -> Result<()> {
    info!(?server.server_id,?server.addr, "Shutdowning");
    server.map_cli.clone().shutdown(()).await?;
    info!(?server.server_id, "Shutdown done");
    Ok(())
}
