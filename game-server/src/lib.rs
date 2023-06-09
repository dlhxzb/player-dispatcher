pub mod dispatcher;
pub mod game_impl;
pub mod server_scaling;
pub mod util;

use common::proto::game_service::game_service_client::GameServiceClient;
use common::proto::map_service::map_service_client::MapServiceClient;
use common::{ServerId, ZoneId};

use tonic::transport::Channel;

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
    pub overhead: usize,
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
