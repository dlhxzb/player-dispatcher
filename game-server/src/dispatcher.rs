use crate::util::*;
use crate::{PlayerId, ServerInfo, ServerInfoInner, ServerStatus, ZoneId, ZoneServers};

use proto::game_service::game_service_client::GameServiceClient;
use proto::map_service::map_service_client::MapServiceClient;

use anyhow::Result;
use crossbeam_skiplist::SkipMap;

use std::ops::Deref;
use std::sync::Arc;

// 世界地图尺寸
pub const WORLD_X_MAX: u64 = 1 << 16;
pub const WORLD_Y_MAX: u64 = 1 << 16;
// Zone最大深度为10，可划分512*512个zone，最小层zone(0,0) id = 1,111,111,111
pub const MAX_ZONE_DEPTH: u32 = 10;

/// # 地图分割方法
/// 将地图分割为4象限，每个象限递归向下划分4象限。可以得到一个类似四叉树的结构。
/// ZoneId表示四叉树节点编号，从高位到低位表示根节点到叶子结点。
/// 例如：`123`:`1`代表根节点世界地图，划分四象限中第`2`象限中再划分四象限中的`3`象限
/// 树高最多MAX_ZONE_DEPTH层，ZoneId位数与节点所处高度相等。

/// # 并发读写保证：
/// ## zone_server_map
/// * 只有一个线程在增删server；
/// * 删除前通过ServerStatus::Closing来拒绝新增用户；
/// * 删除时用户已清零，没有并发访问了
/// ## player_map
/// * API以用户为单位串行
pub struct DispatcherInner {
    pub zone_server_map: SkipMap<ZoneId, ZoneServers>, // 通过Zone定位server
    pub player_map: SkipMap<PlayerId, ServerInfo>,     // 定位Player所属server
}

#[derive(Clone)]
pub struct Dispatcher {
    inner: Arc<DispatcherInner>,
}

impl Deref for Dispatcher {
    type Target = DispatcherInner;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
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
            ZoneServers::Parent(ServerInfo {
                inner: ServerInfoInner {
                    server_id,
                    zones: vec![ROOT_ZONE_ID],
                    map_cli,
                    game_cli,
                    status: ServerStatus::Working,
                    overhead: 0,
                    addr,
                }
                .into(),
            }),
        );

        Ok(Self {
            inner: DispatcherInner {
                zone_server_map: SkipMap::new(),
                player_map: SkipMap::new().into(),
            }
            .into(),
        })
    }

    // 从下层向上找Working的server。对于bottom zone，如果有多个选overhead最低的
    pub fn get_best_server_of_zone(&self, mut zone_id: ZoneId) -> ServerInfo {
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
    pub fn get_servers_of_zone(&self, mut zone_id: ZoneId) -> Vec<ServerInfo> {
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
