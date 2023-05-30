use async_trait::async_trait;
use crossbeam_skiplist::SkipMap;

pub type PlayerId = usize;

pub enum ServerStatus {
    Working,
    Closing,
    Closed,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum ServerId {
    World(usize) = 1,        // (index)可有多台世界服务器
    Zone(ZoneId, usize) = 2, // (ZoneId, index)同一Zone可有多台服务器
}

#[derive(Debug, Clone, Copy)]
pub struct Player {
    pub id: PlayerId,
    pub x: usize,
    pub y: usize,
    pub money: usize,
}

/// 地图某一Zone的服务器，一个Zone可有多台服务器
pub struct Server {
    id: ServerId,
    player_map: SkipMap<PlayerId, Player>,
    status: ServerStatus,
}

// Zone以开始坐标为ID
#[derive(Hash, PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Copy)]
pub struct ZoneId {
    pub x: u64,
    pub y: u64,
}

#[async_trait]
pub trait ServerApi {
    async fn login(&self, player: Player);

    async fn get_overhead(&self) -> usize;
}

impl Server {
    pub fn new(id: ServerId) -> Self {
        Self {
            id,
            player_map: SkipMap::new(),
            status: ServerStatus::Working,
        }
    }
}

#[async_trait]
impl ServerApi for Server {
    async fn login(&self, player: Player) {}

    async fn get_overhead(&self) -> usize {
        self.player_map.len()
    }
}
