use crate::util::*;

use common::proto::map_service::map_service_client::MapServiceClient;
use common::{xy_to_zone_id, PlayerId, ZoneId, MAX_ZONE_DEPTH, ROOT_ZONE_ID};

use anyhow::{Context, Result};
use crossbeam_skiplist::SkipMap;

use std::collections::HashMap;
use std::ops::Deref;
use std::sync::Arc;

/// # 地图分割方法
/// 将地图分割为4象限，每个象限递归向下划分4象限。可以得到一个类似四叉树的结构。
/// ZoneId表示四叉树节点编号，从高位到低位表示根节点到叶子结点。
/// 例如：`123`:`1`代表根节点世界地图，划分四象限中第`2`象限中再划分四象限中的`3`象限
/// 树高1~MAX_ZONE_DEPTH层，ZoneId位数与节点所处高度相等。

/// # 并发读写保证：
/// ## zone_server_map
/// * 只有一个线程在增删server；
/// * 删除前通过ServerStatus::Closing来拒绝新增用户；
/// * 删除时用户已清零，没有并发访问了
/// ## player_map
/// * API以用户为单位串行
pub struct DispatcherInner {
    pub zone_server_map: SkipMap<ZoneId, ZoneServers>, // 通过Zone定位server
    pub player_map: SkipMap<PlayerId, (ServerInfo, f32, f32)>, // 定位Player所属server,x,y
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

        let zone_server_map = SkipMap::new();
        let server = ServerInfo {
            inner: ServerInfoInner {
                server_id,
                zones: vec![ROOT_ZONE_ID],
                map_cli,
                status: ServerStatus::Working,
                addr,
            }
            .into(),
        };
        zone_server_map.insert(
            ROOT_ZONE_ID,
            ZoneServers {
                server,
                exporting_server: None,
            },
        );

        Ok(Self {
            inner: DispatcherInner {
                zone_server_map: SkipMap::new(),
                player_map: SkipMap::new().into(),
            }
            .into(),
        })
    }

    // 逐层向下，找到为止
    pub fn get_server_of_coord(&self, x: f32, y: f32) -> (ZoneId, ZoneServers) {
        for depth in 1..=MAX_ZONE_DEPTH {
            let zone_id = xy_to_zone_id(x, y, depth);
            if let Some(server) = self
                .zone_server_map
                .get(&zone_id)
                .map(|entry| entry.value().clone())
            {
                return (zone_id, server);
            }
        }
        panic!("Root zone server not found");
    }

    pub fn get_server_of_player(&self, player_id: &PlayerId) -> Result<(ServerInfo, f32, f32)> {
        self.player_map
            .get(player_id)
            .map(|entry| entry.value().clone())
            .context("player no in cache")
    }

    pub fn get_all_servers(&self) -> Vec<ServerInfo> {
        self.zone_server_map
            .iter()
            .map(|entry| entry.value().clone().into_vec())
            .flatten()
            .map(|server| (server.server_id, server))
            .collect::<HashMap<_, _>>()
            .into_values()
            .collect()
    }
}
