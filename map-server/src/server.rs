use common::proto::game_service::PlayerInfo;

use anyhow::{Context, Result};
use async_trait::async_trait;
use crossbeam_skiplist::SkipMap;
use kdtree::kdtree::KdTree;
use tokio::sync::RwLock;

use std::sync::Arc;

pub type PlayerId = u64;
pub type ZoneId = u64;
pub type ServerId = u32;

/// 精确位置以player_map为准，kdtree作为加速结构缓存允许略微滞后，将在API中spawn出来修改，不必等待结束
pub struct Server {
    pub id: ServerId,
    pub player_map: Arc<SkipMap<PlayerId, PlayerInfo>>,
    pub kdtree: Arc<RwLock<KdTree<f32, PlayerId, [f32; 2]>>>,
}

impl Server {
    pub fn new(id: ServerId) -> Self {
        Self {
            id,
            player_map: SkipMap::new().into(),
            kdtree: RwLock::new(KdTree::with_capacity(2, 5)).into(),
        }
    }

    pub fn get_player_from_cache(&self, player_id: &PlayerId) -> Result<PlayerInfo> {
        self.player_map
            .get(player_id)
            .map(|entry| entry.value().clone())
            .context("player no in cache")
    }
}
