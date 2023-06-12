use crate::data::*;

use common::proto::game_service::game_service_client::GameServiceClient;
use common::proto::map_service::map_service_client::MapServiceClient;
use common::{ServerId, ZoneId, DEFAULT_MAP_PORT, MAP_PORT_ENV_NAME};

use anyhow::{Context, Result};
use common::{WORLD_X_MAX, WORLD_X_MIN, WORLD_Y_MAX, WORLD_Y_MIN};
use tonic::Status;
use tracing::*;

use std::sync::atomic::{AtomicU32, Ordering};

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

#[instrument]
pub async fn start_map_server(zones: Vec<ZoneId>) -> Result<ServerInfo> {
    use std::env;
    use std::process::Command;
    use tokio::time::{sleep, Duration};

    static PORT: AtomicU32 = AtomicU32::new(DEFAULT_MAP_PORT);
    let port = PORT.fetch_add(1, Ordering::Relaxed);
    let addr = format!("http://[::1]:{port}");

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

pub async fn shutdown_map_server(server: &ServerInfo) -> Result<()> {
    info!(?server.server_id, "Shutdown");
    server.map_cli.clone().shutdown(()).await?;
    Ok(())
}
