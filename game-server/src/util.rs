use crate::data::*;

use common::proto::game_service::game_service_client::GameServiceClient;
use common::proto::map_service::map_service_client::MapServiceClient;
use common::{ServerId, ZoneId};

use anyhow::Result;
use common::{WORLD_X_MAX, WORLD_X_MIN, WORLD_Y_MAX, WORLD_Y_MIN};
use tonic::Status;
use tracing::*;

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
// 判断一节点是否在另一节点的父路径上
pub fn is_in_parent_path(zone: ZoneId, parent: ZoneId) -> bool {
    let len1 = zone.ilog10();
    let len2 = parent.ilog10();
    len1 >= len2 && zone / 10_u64.pow(len1 - len2) == parent
}

pub fn gen_server_id() -> ServerId {
    use std::sync::atomic::{AtomicU32, Ordering};

    static SERVER_ID: AtomicU32 = AtomicU32::new(0);
    SERVER_ID.fetch_add(1, Ordering::Relaxed)
}

#[instrument]
pub async fn start_map_server(zones: Vec<ZoneId>) -> Result<ServerInfo> {
    // TODO: 启动server
    let addr = "http://[::1]:50051".to_string();
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
            status: ServerStatus::Working,
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
