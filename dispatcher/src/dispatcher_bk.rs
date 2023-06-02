use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use anyhow::{Context, Result};
use crossbeam_skiplist::{SkipMap, SkipSet};
use futures::future::join_all;
use futures::stream::{self, StreamExt};
use hashbrown::HashMap;
use tokio::sync::RwLock;
use tonic::transport::{Channel, Server};
use tonic::{async_trait, Request, Response, Status};

// gRPC
use game_interface::game_interface_server::GameInterface;
use game_interface::PlayerInfo;
use map_service::map_service_client::MapServiceClient;

pub mod map_service {
    tonic::include_proto!("map_service");
}

pub mod game_interface {
    tonic::include_proto!("game_interface");
}

type ServerCli = MapServiceClient<tonic::transport::Channel>;
type PlayerId = u64;
type RPCResult<T> = Result<Response<T>, Status>;
// 世界地图尺寸
const WORLD_X_MAX: u64 = 1 << 16;
const WORLD_Y_MAX: u64 = 1 << 16;
// 单台服务器地图尺寸，合计16*16个zone
const ZONE_X_MAX: u64 = 1 << 12;
const ZONE_Y_MAX: u64 = 1 << 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ServerStatus {
    Working,
    Closing,
    Closed,
}

// Zone以开始坐标为ID，全地图表示为(true,0,0)
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ZoneId {
    pub b_world: bool,
    pub x: u64,
    pub y: u64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ServerId {
    zone_id: ZoneId,
    idx: u64, // 同一Zone可有多台服务器
}

#[derive(Debug, Clone, Copy)]
pub struct Player {
    pub id: PlayerId,
    pub x: usize,
    pub y: usize,
    pub money: usize,
}

struct ServerInfo {
    server_id: ServerId,
    cli: ServerCli,
    status: ServerStatus,
    overhead: usize,
}

const WORLD_ZONE: ZoneId = ZoneId {
    b_world: true,
    x: 0,
    y: 0,
};

pub struct Dispatcher {
    zone_server_map: SkipMap<ZoneId, SkipMap<usize, ServerInfo>>, // 快速定位Zone的server
    player_map: SkipMap<PlayerId, ServerId>,                      // 快速定位Player所属server
}

impl Dispatcher {
    pub async fn new() -> Result<Self> {
        let server_id = ServerId {
            zone_id: WORLD_ZONE,
            idx: 0,
        };
        // TODO: start world_server
        let mut world_server_cli: MapServiceClient<Channel> =
            MapServiceClient::connect("http://[::1]:50051").await?;

        let zone_server_map = SkipMap::new();
        zone_server_map.insert(
            WORLD_ZONE,
            ServerInfo {
                server_id,
                cli: world_server_cli.into(),
                status: ServerStatus::Working,
                overhead: 0,
            },
        );

        Ok(Self {
            zone_server_map: SkipMap::new(),
            player_map: SkipMap::new(),
        })
    }

    async fn login_inner(&self, player: PlayerInfo) -> Result<()> {
        anyhow::ensure!(
            player.x < WORLD_X_MAX && player.y < WORLD_Y_MAX,
            format!("{:?} out of map range", player)
        );

        let zone_id = xy_to_zone(player.x, player.y);

        // 找到对应的Zone server，没有的话用world server
        let (server_id, mut server) = self
            .zone_server_map
            .get(&zone_id)
            .map(|zone_entry| {
                zone_entry
                    .value()
                    .iter()
                    .filter(|entry| entry.value().status == ServerStatus::Working)
                    .min_by_key(|entry| entry.value().overhead)
                    .map(|entry| {
                        let info = entry.value();
                        (info.server_id, info.cli.clone())
                    })
            })
            .flatten()
            .unwrap_or_else(|| {
                // overhead最小的world server
                self.zone_server_map
                    .get(&WORLD_ZONE)
                    .expect("No world server found")
                    .value()
                    .iter()
                    .filter(|entry| entry.value().status == ServerStatus::Working)
                    .min_by_key(|entry| entry.value().overhead)
                    .map(|entry| {
                        let info = entry.value();
                        (info.server_id, info.cli.clone())
                    })
                    .expect("No world server found")
            });
        let player_id = player.id;
        server.login(player).await?;
        self.player_map.insert(player_id, server_id);
        Ok(())
    }

    async fn logout(&self, player: Player) -> Result<()> {
        todo!();
    }
}

#[async_trait]
impl GameInterface for Dispatcher {
    async fn login(&self, player: Request<PlayerInfo>) -> RPCResult<()> {
        let player = player.into_inner();
        self.login_inner(player)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(()))
    }
}

#[inline]
fn xy_to_zone(x: u64, y: u64) -> ZoneId {
    ZoneId {
        b_world: false,
        x: x - x % ZONE_X_MAX,
        y: y - y % ZONE_Y_MAX,
    }
}
