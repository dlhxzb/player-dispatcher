use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use anyhow::{Context, Result};
use crossbeam_skiplist::SkipMap;
use ert::prelude::*;
use futures::stream::{self, StreamExt};
use tonic::transport::{Channel, Server};
use tonic::{async_trait, Request, Response, Status};
use tracing::error;

use crate::grpc::game_service::game_service_client::GameServiceClient;
use crate::grpc::map_service::map_service_client::MapServiceClient;
use crate::grpc::map_service::{
    ConnectRequest, ExportRequest, GetPlayersReply, GetPlayersRequest, ZonePlayersReply,
};
use crate::util::*;

/// # 地图分割方法
/// 将地图分割为4象限，每个象限递归向下划分4象限。可以得到一个类似四叉树的结构。
/// ZoneId表示四叉树节点编号，从高位到低位表示根节点到叶子结点。
/// 例如：`12`代表世界地图4象限中第`1`象限中再划分四象限中的`2`象限
/// 最多划分MAX_ZONE_DEPTH层，ZoneId位数与节点所处深度相等。

pub type PlayerId = u64;
pub type ZoneId = u64;
pub type ServerId = u64;
type ZoneSubId = u64;

// 世界地图尺寸
pub const WORLD_X_MAX: u64 = 1 << 16;
pub const WORLD_Y_MAX: u64 = 1 << 16;
// Zone最大深度为10，可划分512*512个zone，最小层zone(0,0) id = 1,111,111,111
pub const MAX_ZONE_DEPTH: u32 = 10;
// 服务器最大用户数，触发扩容
pub const MAX_PLAYER: u64 = 1000;
// 服务器最小用户数，触发缩容
const MIN_PLAYER: u64 = MAX_PLAYER / 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ServerStatus {
    Working,
    Closing,
    Closed,
}

#[derive(Debug, Clone, Copy)]
pub struct Player {
    pub id: PlayerId,
    pub x: usize,
    pub y: usize,
    pub money: usize,
}

#[derive(Clone)]
pub struct ServerInfo {
    pub server_id: ServerId,
    pub zones: Vec<ZoneId>,
    pub map_cli: MapServiceClient<Channel>,
    pub game_cli: GameServiceClient<Channel>,
    pub status: ServerStatus,
    pub overhead: usize,
    pub addr: String,
}

// Bottom（MAX_ZONE_DEPTH）层可有多个server，其它层只有一个
pub enum ZoneServers {
    Bottom(SkipMap<ServerId, Arc<ServerInfo>>),
    Parent(Arc<ServerInfo>),
}

/// # 并发读写保证：
/// ## zone_server_map
/// * 只有一个线程在增删server；
/// * 删除前通过ServerStatus::Closing来拒绝新增用户；
/// * 删除时用户已清零，没有并发访问了
/// ## player_map
/// * API以用户为单位串行
pub struct Dispatcher {
    pub zone_server_map: SkipMap<ZoneId, ZoneServers>, // 通过Zone定位server
    pub player_map: Arc<SkipMap<PlayerId, ServerId>>,  // 定位Player所属server
}

impl Dispatcher {
    pub async fn new() -> Result<Self> {
        let server_id = gen_server_id();
        let addr = start_server().await;
        // TODO: 将zone等配置传递给server
        let map_cli = MapServiceClient::connect(addr.clone()).await?;
        let game_cli = GameServiceClient::connect(addr.clone()).await?;

        let zone_server_map = SkipMap::new();
        zone_server_map.insert(
            ROOT_ZONE_ID,
            ZoneServers::Parent(
                ServerInfo {
                    server_id,
                    zones: vec![ROOT_ZONE_ID],
                    map_cli,
                    game_cli,
                    status: ServerStatus::Working,
                    overhead: 0,
                    addr,
                }
                .into(),
            ),
        );

        Ok(Self {
            zone_server_map: SkipMap::new(),
            player_map: SkipMap::new().into(),
        })
    }

    // 从下层向上找Working的server。对于bottom zone，如果有多个选overhead最低的
    pub fn get_best_server_of_zone(&self, mut zone_id: ZoneId) -> Arc<ServerInfo> {
        loop {
            let server = self
                .zone_server_map
                .get(&zone_id)
                .map(|zone_entry| match zone_entry.value() {
                    ZoneServers::Parent(server) => {
                        if server.status == ServerStatus::Working {
                            Some(server.clone())
                        } else {
                            None
                        }
                    }
                    ZoneServers::Bottom(map) => map
                        .iter()
                        .filter(|entry| entry.value().status == ServerStatus::Working)
                        .min_by_key(|entry| entry.value().overhead)
                        .map(|entry| entry.value().clone()),
                })
                .flatten();
            if let Some(server) = server {
                return server;
            }
            assert_eq!(zone_id, 0, "No root map server found");
            zone_id /= 10;
        }
    }

    // 从下层向上找包含指定Zone的 !Closed server
    pub fn get_servers_of_zone(&self, mut zone_id: ZoneId) -> Vec<Arc<ServerInfo>> {
        loop {
            let servers = self
                .zone_server_map
                .get(&zone_id)
                .map(|zone_entry| match zone_entry.value() {
                    ZoneServers::Parent(server) => {
                        if server.status == ServerStatus::Closed {
                            None
                        } else {
                            Some(vec![server.clone()])
                        }
                    }
                    ZoneServers::Bottom(map) => {
                        let servers: Vec<_> = map
                            .iter()
                            .filter(|entry| entry.value().status != ServerStatus::Closed)
                            .map(|entry| entry.value().clone())
                            .collect();
                        if servers.is_empty() {
                            None
                        } else {
                            Some(servers)
                        }
                    }
                })
                .flatten();
            if let Some(servers) = servers {
                return servers;
            }
            assert_eq!(zone_id, 0, "No root map server found");
            zone_id /= 10;
        }
    }
}
