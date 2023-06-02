use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use anyhow::{Context, Result};
use crossbeam_skiplist::{SkipMap, SkipSet};
use ert::prelude::*;
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
type ZoneId = u64;
type ZoneSubId = u64;
type RPCResult<T> = Result<Response<T>, Status>;
// 世界地图尺寸
const WORLD_X_MAX: u64 = 1 << 16;
const WORLD_Y_MAX: u64 = 1 << 16;
// Zone最大分割深度为10，可划分1024*1024个zone，最小层zone(0,0) id = 1,111,111,111
const ZONE_DEPTH: u32 = 10;
// 服务器最大用户数，触发扩容
const MAX_PLAYER: u64 = 1000;
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

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ServerId {
    zone_id: ZoneId,
    zone_sub_id: ZoneSubId, // ZONE_DEPTH层的Zone可有多台服务器，其它层该值为0
}

#[derive(Clone)]
struct ServerInfo {
    server_id: ServerId,
    cli: ServerCli,
    status: ServerStatus,
    overhead: usize,
}

const WORLD_ZONE_ID: ZoneId = 0;

pub struct Dispatcher {
    zone_server_map: SkipMap<ZoneId, SkipMap<ZoneSubId, ServerInfo>>, // 快速定位Zone的server
    player_map: SkipMap<PlayerId, ServerId>,                          // 快速定位Player所属server
}

impl Dispatcher {
    pub async fn new() -> Result<Self> {
        let server_id = ServerId {
            zone_id: WORLD_ZONE_ID,
            zone_sub_id: 0,
        };
        let addr = start_server().await;
        let world_server_cli = MapServiceClient::connect(addr).await?;

        let zone_server_map = SkipMap::new();
        zone_server_map.insert(
            WORLD_ZONE_ID,
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
        anyhow::ensure!(
            !self.player_map.contains_key(&player.id),
            format!("{} had logged in", player.id)
        );

        let zone_id = xy_to_zone(player.x, player.y);
        let mut server_info = self.get_zone_server(zone_id);
        let player_id = player.id;
        server_info.cli.login(player).await?;
        self.player_map.insert(player_id, server_info.server_id);

        Ok(())
    }

    // 如果有多个，返回Working的，overhead最低的
    fn get_zone_server(&self, mut id: ZoneId) -> ServerInfo {
        loop {
            let info = self.zone_server_map.get(&id).map(|zone_entry| {
                zone_entry
                    .value()
                    .iter()
                    .filter(|entry| entry.value().status == ServerStatus::Working)
                    .min_by_key(|entry| entry.value().overhead)
                    .expect(&format!("Empty value for {id} in zone_server_map"))
                    .value()
                    .clone()
            });
            if let Some(info) = info {
                return info;
            }
            assert_eq!(id, 0, "No root map server found");
            id /= 10;
        }
    }

    // server中用户超过上限，触发扩容。最底层zone无法分割，划分一半用户到新server。
    // 上层zone可分割，找出用户最多的zone，划分到新server
    async fn devide_server(&self, server: ServerInfo) -> Result<()> {
        Ok(())
    }

    // 分割zone扩容。将server中的某一个zone用户移动到新server
    async fn devide_server_zone(&self, from: ServerInfo, zone_id: ZoneId) -> Result<()> {
        // self.player_map.iter().
        Ok(())
    }

    async fn add_server_for_zone(&self, zone_id: ZoneId) -> Result<ServerInfo> {
        let addr = start_server().await;
        let cli = MapServiceClient::connect(addr).await?;
        if zone_depth(zone_id) == ZONE_DEPTH {
            // 最底层zone无法分割，获取该zone的所有server
            let servers = self
                .zone_server_map
                .get(&zone_id)
                .map(|entry| {
                    entry
                        .value()
                        .iter()
                        .map(|entry| entry.value().clone())
                        .collect::<Vec<_>>()
                })
                .unwrap();
            let mut zone_sub_id = 0;
            // 跳表会按从小到大排序，找到空缺的作为新server的zone_sub_id
            for server in servers {
                if zone_sub_id != server.server_id.zone_sub_id {
                    break;
                }
                zone_sub_id += 1;
            }
            let server_id = ServerId {
                zone_id,
                zone_sub_id,
            };
            let info = ServerInfo {
                server_id,
                cli,
                status: ServerStatus::Working,
                overhead: 0,
            };
            self.zone_server_map.get(&zone_id).map(|entry| {
                let map = entry.value();
                map.insert(zone_sub_id, info);
            });
            // TODO: 随机分割用户
        } else {
        }
        todo!()
    }
}

#[async_trait]
impl GameInterface for Arc<Dispatcher> {
    async fn login(&self, player: Request<PlayerInfo>) -> RPCResult<()> {
        let player = player.into_inner();
        let player_id = player.id;
        let server = self.clone();
        async move { server.login_inner(player).await }
            .via_g(player_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(()))
    }
}

#[inline]
fn xy_to_zone(mut x: u64, mut y: u64) -> u64 {
    let mut id = 0;
    let mut length = WORLD_X_MAX;
    let mut height = WORLD_Y_MAX;
    for _ in 0..ZONE_DEPTH {
        length /= 2;
        height /= 2;
        //      2 4
        // 原点 1 3
        let pos = if x < length {
            if y < height {
                1
            } else {
                y -= height;
                2
            }
        } else {
            x -= length;
            if y < height {
                3
            } else {
                y -= height;
                4
            }
        };
        id = id * 10 + pos;
    }
    id
}

#[inline]
fn zone_depth(id: ZoneId) -> u32 {
    if id > 0 {
        id.ilog10() + 1
    } else {
        0
    }
}

async fn start_server() -> String {
    // TODO: 启动server
    "http://[::1]:50051".to_string()
}
