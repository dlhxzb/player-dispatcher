use crate::dispatcher::Dispatcher;
use crate::util::*;

use common::proto::map_service::{ExportRequest, ZoneDepth, ZonePlayersReply};
use common::{zone_depth, PlayerId, MAX_ZONE_DEPTH};

use anyhow::{ensure, Result};
use ert::prelude::RunVia;
use futures::StreamExt;
use tonic::async_trait;
use tracing::error;

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
    // 管理多个同父叶子节点时，挑出最大的扩容
    // 只管理一个叶子节点时，深度+1，分出4个叶子结点，把最大的扩容
    async fn expand_overload_server(&self, server: &ServerInfo) -> Result<()> {
        let mut depth = zone_depth(server.zones[0]);
        let only_one_zone = server.zones.len() == 1;
        ensure!(
            !(only_one_zone && depth == MAX_ZONE_DEPTH),
            "Can not expand only one zone of MAX_ZONE_DEPTH"
        );
        let hd = tokio::spawn(Self::start_server());
        if only_one_zone {
            // 只管理一个叶子节点时，深度+1，分出4个叶子结点
            depth += 1;
        };
        let ZonePlayersReply {
            zone_id: new_zone_id,
            player_ids,
        } = server
            .map_cli
            .clone()
            .get_heaviest_zone_players(ZoneDepth { depth })
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

        let zones: Vec<_> = if only_one_zone {
            // 改成3个叶子节点，先删原节点
            self.zone_server_map.remove(&server.zones[0]);
            get_child_zone_ids(server.zones[0])
                .into_iter()
                .filter(|id| id != &new_zone_id)
                .collect()
        } else {
            // 从原节点中去除1个
            server
                .zones
                .iter()
                .filter(|&id| id != &new_zone_id)
                .copied()
                .collect()
        };
        let update_server = ZoneServers {
            server: ServerInfo {
                inner: ServerInfoInner {
                    server_id: server.server_id,
                    zones: zones.clone(),
                    map_cli: server.map_cli.clone(),
                    status: ServerStatus::Working,
                    addr: server.addr.clone(),
                }
                .into(),
            },
            exporting_server: None,
        };
        zones.into_iter().for_each(|zone_id| {
            self.zone_server_map.insert(zone_id, update_server.clone());
        });
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
                    let (_, x, y) = self.get_server_of_player(&player_id)?;
                    self.player_map.insert(player_id, (target, x, y));
                    Result::<(), anyhow::Error>::Ok(())
                }
                .via_g(player_id)
                .await
                .map_err(|e| error!(?e));
            })
            .await;
        Ok(())
    }
    async fn start_server() -> Result<ServerInfo> {
        todo!()
    }
    async fn stop_server(_addr: &str) -> Result<()> {
        todo!()
    }
}
