use crate::dispatcher::Dispatcher;
use crate::util::*;
use crate::{PlayerId, ServerInfo, ZoneId, ZoneServers};

use common::proto::map_service::{
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
    async fn expand_overload_server(&self, server: &ServerInfo) -> Result<()>;
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
}

/// API级别不能保证并发原子，应避免多个线程同时调用，以下API仅在monitor单线程中使用
#[async_trait]
impl ServerScaling for Dispatcher {
    async fn expand_overload_server(&self, server: &ServerInfo) -> Result<()> {
        let hd = tokio::spawn(Self::start_server());
        let ZonePlayersReply {
            zone_id: new_zone_id,
            player_ids,
        } = server
            .map_cli
            .clone()
            .get_heaviest_zone_players(())
            .await?
            .into_inner();
        let new_server = hd.await??;
        self.zone_server_map.insert(
            new_zone_id,
            ZoneServers {
                server: new_server.clone(),
                exporting_server: Some(server.clone()),
            },
        );
        self.transfer_players(&server, &new_server, &player_ids)
            .await?;
        // 完成后取消exporting_server设置
        self.zone_server_map.insert(
            new_zone_id,
            ZoneServers {
                server: new_server.clone(),
                exporting_server: None,
            },
        );
        Ok(())
    }

    async fn reduce_idle_server(&self, _server: &ServerInfo) -> Result<()> {
        todo!()
    }
    async fn get_overload_server(&self) -> Result<Option<ServerInfo>> {
        todo!()
    }
    async fn get_idle_server(&self) -> Result<Option<ServerInfo>> {
        todo!()
    }

    // 把player_id取来，逐个让map-server导出。这里要用ert将用户串行，避免与game api数据竞争
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
            .for_each_concurrent(None, |&player_id| async move {
                let mut cli = source_server.map_cli.clone();
                let target = target_server.clone();
                let self = self.clone();
                let _ = async move {
                    cli.export_player(ExportRequest {
                        player_id,
                        addr: target.addr.clone(),
                        coord: None,
                    })
                    .await
                    .map_err(anyhow::Error::msg)?;
                    let (_, x, y) = self.get_player_from_cache(&player_id)?;
                    self.player_map.insert(player_id, (target, x, y));
                    Result::<(), anyhow::Error>::Ok(())
                }
                .via_g(player_id)
                .await
                .map_err(|e| error!(?e));
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
}
