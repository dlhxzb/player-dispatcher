use common::proto::game_service::PlayerInfo;
use common::proto::map_service::map_service_client::MapServiceClient;
use common::{PlayerId, ServerId};

use anyhow::{Context, Result};
use crossbeam_skiplist::{SkipMap, SkipSet};
use tokio::sync::Mutex;
use tonic::transport::Channel;

use std::ops::Deref;
use std::sync::Arc;

/// 精确位置以player_map为准，grid作为加速结构按位置初筛
#[derive(Default)]
pub struct InnerServer {
    pub id: ServerId,
    pub player_map: SkipMap<PlayerId, PlayerInfo>,
    pub grid_player_map: SkipMap<(usize, usize), SkipSet<PlayerId>>, // (usize, usize): grid id
    pub export_cli: Mutex<Option<(String, MapServiceClient<Channel>)>>, // 导出用户时使用，导出完成清空。不会同时向两个服务器导出
}

#[derive(Clone)]
pub struct Server {
    inner: Arc<InnerServer>,
}

impl Deref for Server {
    type Target = InnerServer;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl Server {
    pub fn new(id: ServerId) -> Self {
        Self {
            inner: InnerServer {
                id,
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
}
