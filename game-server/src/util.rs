use common::proto::map_service::map_service_client::MapServiceClient;
use common::{ServerId, ZoneId};

use anyhow::Result;
use common::{WORLD_X_MAX, WORLD_X_MIN, WORLD_Y_MAX, WORLD_Y_MIN};
use tonic::transport::Channel;
use tonic::Status;

use std::ops::Deref;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ServerStatus {
    Working,
    Closing,
}
#[derive(Clone)]
pub struct ServerInfo {
    pub inner: Arc<ServerInfoInner>,
}

impl Deref for ServerInfo {
    type Target = ServerInfoInner;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

pub struct ServerInfoInner {
    pub server_id: ServerId,
    pub zones: Vec<ZoneId>,
    pub map_cli: MapServiceClient<Channel>,
    pub status: ServerStatus,
    pub addr: String,
}

impl PartialEq for ServerInfo {
    fn eq(&self, target: &Self) -> bool {
        self.server_id == target.server_id
    }
}

impl ServerInfo {
    pub fn contains_zone(&self, zone_id: ZoneId) -> bool {
        self.zones.iter().any(|id| &zone_id == id)
    }
}

// 一个叶子节点除了有自身服务器，可能还有一台正在给起导入用户的服务器。未指定用户的请求要两个都发送，e.g. aoe/query
#[derive(Clone)]
pub struct ZoneServers {
    pub server: ServerInfo,
    pub exporting_server: Option<ServerInfo>,
}

impl ZoneServers {
    pub fn into_vec(self) -> Vec<ServerInfo> {
        if let Some(export) = self.exporting_server {
            vec![self.server, export]
        } else {
            vec![self.server]
        }
    }
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

pub async fn start_server() -> String {
    // TODO: 启动server
    "http://[::1]:50051".to_string()
}
