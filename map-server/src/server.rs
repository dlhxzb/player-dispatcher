use common::proto::game_service::PlayerInfo;
use common::proto::map_service::map_service_client::MapServiceClient;
use common::{GridId, PlayerId, ServerId};

use anyhow::{Context, Result};
use crossbeam_skiplist::{SkipMap, SkipSet};
use tokio::sync::Mutex;
use tonic::transport::Channel;

use std::ops::Deref;
use std::sync::Arc;

/// 精确位置以player_map为准，grid作为加速结构按位置初筛
#[derive(Default)]
pub struct InnerServer {
    pub server_id: ServerId,
    pub player_map: SkipMap<PlayerId, PlayerInfo>,
    pub grid_player_map: SkipMap<GridId, SkipSet<PlayerId>>, // (usize, usize): grid id
    pub export_addr_cli_cache: Mutex<Option<(String, MapServiceClient<Channel>)>>, // 导出用户时使用，导出完成清空。不会同时向两个服务器导出
}

#[derive(Clone)]
pub struct MapServer {
    inner: Arc<InnerServer>,
}

impl Deref for MapServer {
    type Target = InnerServer;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl MapServer {
    pub fn new(server_id: ServerId) -> Self {
        Self {
            inner: InnerServer {
                server_id,
                ..Default::default()
            }
            .into(),
        }
    }

    pub fn get_player_info(&self, player_id: &PlayerId) -> Result<PlayerInfo> {
        self.player_map
            .get(player_id)
            .map(|entry| entry.value().clone())
            .context("player no in cache")
    }

    pub async fn get_export_cli(&self, addr: String) -> Result<MapServiceClient<Channel>> {
        let mut guard = self.export_addr_cli_cache.lock().await;
        match &*guard {
            Some((cached_addr, cached_cli)) if cached_addr == &addr => Ok(cached_cli.clone()),
            _ => {
                let map_cli = MapServiceClient::connect(addr.clone()).await?;
                *guard = Some((addr, map_cli.clone()));
                Ok(map_cli)
            }
        }
    }
}
