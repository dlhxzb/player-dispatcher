use common::proto::game_service::PlayerInfo;

use anyhow::{Context, Result};
use async_trait::async_trait;
use crossbeam_skiplist::{SkipMap, SkipSet};
use tokio::sync::RwLock;

use std::ops::Deref;
use std::sync::Arc;

pub type PlayerId = u64;
pub type ZoneId = u64;
pub type ServerId = u32;

/// 精确位置以player_map为准，kdtree作为加速结构缓存允许略微滞后，将在API中spawn出来修改，不必等待结束
pub struct InnerServer {
    pub id: ServerId,
    pub player_map: SkipMap<PlayerId, PlayerInfo>,
    pub grid_player_map: SkipMap<(usize, usize), Arc<SkipSet<PlayerId>>>, // (usize, usize): grid id
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
                player_map: SkipMap::new(),
                grid_player_map: SkipMap::new(),
            }
            .into(),
        }
    }

    pub fn get_player_from_cache(&self, player_id: &PlayerId) -> Result<PlayerInfo> {
        self.player_map
            .get(player_id)
            .map(|entry| entry.value().clone())
            .context("player no in cache")
    }
}
