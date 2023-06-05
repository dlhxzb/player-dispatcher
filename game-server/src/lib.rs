pub mod dispatcher;
pub mod game_impl;
pub mod server_scaling;
pub mod util;

use proto::game_service::game_service_client::GameServiceClient;
use proto::map_service::map_service_client::MapServiceClient;

use crossbeam_skiplist::SkipMap;
use tonic::transport::Channel;

use std::ops::Deref;
use std::sync::Arc;

pub type PlayerId = u64;
pub type ZoneId = u64;
pub type ServerId = u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ServerStatus {
    Working,
    Closing,
    Closed,
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
    pub game_cli: GameServiceClient<Channel>,
    pub status: ServerStatus,
    pub overhead: usize,
    pub addr: String,
}

impl PartialEq for ServerInfo {
    fn eq(&self, target: &Self) -> bool {
        self.server_id == target.server_id
    }
}

// Bottom（MAX_ZONE_DEPTH）层可有多个server，其它层只有一个
pub enum ZoneServers {
    Bottom(SkipMap<ServerId, ServerInfo>),
    Parent(ServerInfo),
}
