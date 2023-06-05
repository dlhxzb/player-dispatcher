use crate::dispatcher::{Dispatcher, MAX_ZONE_DEPTH};
use crate::util::*;
use crate::{PlayerId, ServerInfo, ZoneId, ZoneServers};

use proto::map_service::{
    ConnectRequest, ExportRequest, GetPlayersReply, GetPlayersRequest, ZonePlayersReply,
};

use anyhow::Result;
use crossbeam_skiplist::SkipMap;
use ert::prelude::RunVia;
use futures::StreamExt;
use tonic::async_trait;
use tracing::error;

// 服务器最大用户数，触发扩容
pub const MAX_PLAYER: u64 = 1000;
// 服务器最小用户数，触发缩容
pub const MIN_PLAYER: u64 = MAX_PLAYER / 4;

#[async_trait]
pub trait ServerScaling {
    /// 扩容
    async fn expand_overload_server(&self, server: &ServerInfo) -> Result<()> {
        let hd = tokio::spawn(Self::start_server());
        let (players, zone_id) = self.divide_zone_from_server(&server).await?;
        let new_server = hd.await??;
        self.bind_server(&new_server, zone_id).await?;
        self.transfer_players(&server, &new_server, &players).await
    }
    /// 缩容
    async fn reduce_idle_server(&self, server: &ServerInfo) -> Result<()>;
    async fn get_overload_server(&self) -> Result<Option<ServerInfo>>;
    async fn get_idle_server(&self) -> Result<Option<ServerInfo>>;
    async fn transfer_players(
        &self,
        source_server: &ServerInfo,
        target_server: &ServerInfo,
        players: &[PlayerId],
    ) -> Result<()>;
    async fn start_server() -> Result<ServerInfo>;
    async fn stop_server(addr: &str) -> Result<()>;
    async fn divide_zone_from_server(&self, server: &ServerInfo)
        -> Result<(Vec<PlayerId>, ZoneId)>;
    async fn bind_server(&self, server: &ServerInfo, zone_id: ZoneId) -> Result<()>;
}

/// API级别不能保证并发原子，应避免多个线程同时调用，仅在monitor线程中使用
#[async_trait]
impl ServerScaling for Dispatcher {
    async fn reduce_idle_server(&self, _server: &ServerInfo) -> Result<()> {
        todo!()
    }
    async fn get_overload_server(&self) -> Result<Option<ServerInfo>> {
        todo!()
    }
    async fn get_idle_server(&self) -> Result<Option<ServerInfo>> {
        todo!()
    }
    async fn transfer_players(
        &self,
        source_server: &ServerInfo,
        target_server: &ServerInfo,
        players: &[PlayerId],
    ) -> Result<()> {
        source_server
            .map_cli
            .clone()
            .connect_server(ConnectRequest {
                addr: target_server.addr.clone(),
            })
            .await?;

        futures::stream::iter(players)
            .for_each_concurrent(None, |&player_id| {
                let mut cli = source_server.map_cli.clone();
                let target = target_server.clone();
                let dp = self.clone();
                async move {
                    if let Err(e) = cli
                        .export_player(ExportRequest {
                            player_id,
                            addr: target.addr.clone(),
                        })
                        .await
                    {
                        error!(?e, "transfer_player {player_id} failed");
                    } else {
                        dp.player_map.insert(player_id, target);
                    }
                }
                .via_g(player_id)
            })
            .await;

        source_server
            .map_cli
            .clone()
            .disconnect_server(ConnectRequest {
                addr: target_server.addr.clone(),
            })
            .await?;
        Ok(())
    }
    async fn start_server() -> Result<ServerInfo> {
        todo!()
    }
    async fn stop_server(_addr: &str) -> Result<()> {
        todo!()
    }

    // 分割出用户最多的zone，zone无法再分时直接分割用户
    async fn divide_zone_from_server(
        &self,
        server: &ServerInfo,
    ) -> Result<(Vec<PlayerId>, ZoneId)> {
        if server.zones.len() == 1 && zone_depth(server.zones[0]) == MAX_ZONE_DEPTH {
            // bottom zone拿出一半用户
            let GetPlayersReply { player_ids } = server
                .map_cli
                .clone()
                .get_n_players(GetPlayersRequest { n: MAX_PLAYER / 2 })
                .await?
                .into_inner();
            Ok((player_ids, server.zones[0]))
        } else {
            // 划分出有最多用户的zone或者子Zone（只有一个zone时）
            let ZonePlayersReply {
                zone_id: new_zone_id,
                player_ids,
            } = server
                .map_cli
                .clone()
                .get_heaviest_zone_players(())
                .await?
                .into_inner();
            Ok((player_ids, new_zone_id))
        }
    }

    async fn bind_server(&self, server: &ServerInfo, zone_id: ZoneId) -> Result<()> {
        if zone_depth(zone_id) == MAX_ZONE_DEPTH {
            // 绑定到bottom zone
            let entry = self
                .zone_server_map
                .get_or_insert_with(zone_id, || ZoneServers::Bottom(SkipMap::new()));
            let ZoneServers::Bottom(map) = entry.value() else{
                    anyhow::bail!("Not ZoneServers::Bottom {zone_id}");
                };
            map.insert(server.server_id, server.clone().into());
        } else {
            // 绑定到parent zone
            self.zone_server_map
                .insert(zone_id, ZoneServers::Parent(server.clone().into()));
        }
        Ok(())
    }
}
