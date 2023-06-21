use crate::data::*;
use crate::dispatcher::Dispatcher;
use crate::util::*;

use common::proto::game_service::QueryRequest;
use common::proto::map_service::{ExportRequest, GetPlayersRequest, ZoneDepth, ZonePlayersReply};
use common::*;

use anyhow::Result;
use ert::prelude::RunVia;
use futures::StreamExt;
use tonic::async_trait;
use tracing::*;

use std::collections::HashMap;

#[async_trait]
pub trait ServerScaling {
    /// 扩容，叶子结点展开
    async fn expand_overload_server(&self, server: &ServerInfo) -> Result<bool>;
    /// 缩容，叶子结点结合
    async fn close_idle_server(&self, server: &ServerInfo, merge_to: &ServerInfo) -> Result<()>;
    /// 在给定overhead_map中找到一个合适的缩容导入服务器
    fn get_merge_target_server(
        &self,
        server: &ServerInfo,
        overhead: u32,
        overhead_map: &HashMap<ServerId, u32>,
    ) -> Result<Option<ServerInfo>>;
    async fn transfer_players(
        &self,
        source_server: &ServerInfo,
        target_server: &ServerInfo,
        players: &[PlayerId],
    ) -> Result<()>;
}

/// API级别不能保证并发原子，应避免多个线程同时调用，以下API仅在monitor单线程中使用
#[async_trait]
impl ServerScaling for Dispatcher {
    /// 管理多个同父叶子节点时，挑出最大的扩容
    /// 只管理一个叶子节点时，深度+1，分出4个叶子结点，把最大的分配到新服务器
    /// 只有一个节点且达最大深度则无法扩展，直接返回false
    /// 1. 新旧服务器同时注册到要导出的zone(server + exporting_server)
    /// 2. 用户导出(此时对于该zone的范围请求(aoe/query)会送到两台服务器)
    /// 3. 旧服务器取消导出zone的注册(exporting=None)
    #[instrument(skip_all, fields(server_id = %server.server_id, zones = ?server.zones))]
    async fn expand_overload_server(&self, server: &ServerInfo) -> Result<bool> {
        info!("IN");
        let mut depth = zone_depth(server.zones[0]);
        let only_one_zone = server.zones.len() == 1;
        if only_one_zone && depth == self.config.max_zone_depth {
            info!(
                "Can not expand server:{} for only one zone of max depth {}",
                server.server_id, self.config.max_zone_depth
            );
            return Ok(false);
        }
        if only_one_zone {
            // 只管理一个叶子节点时，深度+1，分出4个叶子结点
            depth += 1;
        };
        // 从server拆分出人数最多的zone。
        // 注意：在获取之后，zone_server_map.insert之前，该服务器还可能被login。下面要loop transfer_players直至该zone无人为止
        let ZonePlayersReply {
            zone_id: new_zone_id,
            mut player_ids,
        } = server
            .map_cli
            .clone()
            .get_heaviest_zone_players(ZoneDepth { depth })
            .await?
            .into_inner();
        // 启动一台新server
        let new_server = start_map_server(vec![new_zone_id]).await?;
        // 将导出server和导入server都注册到zone
        self.zone_server_map.insert(
            new_zone_id,
            ZoneServers {
                server: new_server.clone(),
                exporting_server: Some(server.clone()),
            },
        );
        // 获得原server拆分后剩下的zone
        let zones: Vec<_> = if only_one_zone {
            // 只有一个zone时，先注销原节点，拆分成4个叶子节点，去掉导出的
            self.zone_server_map.remove(&server.zones[0]);
            get_child_zone_ids(server.zones[0])
                .into_iter()
                .filter(|id| id != &new_zone_id)
                .collect()
        } else {
            // 原server有多个zone，去掉导出的
            server
                .zones
                .iter()
                .filter(|&id| id != &new_zone_id)
                .copied()
                .collect()
        };
        let mut inner = (*server.inner).clone();
        inner.zones = zones.clone();
        let update_server = ZoneServers {
            server: ServerInfo {
                inner: inner.into(),
            },
            exporting_server: None,
        };
        // 重新注册原server拆分剩下的zone
        zones.into_iter().for_each(|zone_id| {
            self.zone_server_map.insert(zone_id, update_server.clone());
        });

        let AABB {
            xmin,
            xmax,
            ymin,
            ymax,
        } = AABB::from_zone_id(new_zone_id);
        while !player_ids.is_empty() {
            // 用户导出。loop transfer_players直至该zone无人为止
            self.transfer_players(server, &new_server, &player_ids)
                .await?;
            player_ids = server
                .game_cli
                .clone()
                .query(QueryRequest {
                    xmin,
                    xmax,
                    ymin,
                    ymax,
                })
                .await?
                .into_inner()
                .infos
                .into_iter()
                .map(|info| info.player_id)
                .collect();
        }

        // 完成后取消exporting_server设置
        self.zone_server_map.insert(
            new_zone_id,
            ZoneServers {
                server: new_server.clone(),
                exporting_server: None,
            },
        );

        info!("OUT");
        Ok(true)
    }

    // 关闭负载小的服务器，将用户转移到其它同父叶子节点服务器。若因合并后人数超限无法合并则返回false
    #[instrument(skip_all, fields(server_id = %server.server_id, zones = ?server.zones))]
    fn get_merge_target_server(
        &self,
        server: &ServerInfo,
        overhead: u32,
        overhead_map: &HashMap<ServerId, u32>,
    ) -> Result<Option<ServerInfo>> {
        if server.zones[0] == ROOT_ZONE_ID {
            info!("Skip merge root zone");
            return Ok(None);
        }

        // 获取同父其它叶子节点的最闲服务器
        let Some((bro_overhead, bro_server)) = get_child_zone_ids(server.zones[0] / 10)
            .into_iter()
            .filter(|id| !server.zones.contains(id))
            .filter_map(|id| {
                self.zone_server_map.get(&id).map(|entry| {
                    let server = entry.value().server.clone();
                    (server.server_id, server)
                })
            })
            .collect::<HashMap<_, _>>()
            .into_values()
            .filter_map(|server| {
                overhead_map
                    .get(&server.server_id)
                    .map(|count| (*count, server))
            })
            .min_by_key(|(count, _)| *count) else
            {
                info!("No brother leaf found to merge");
                return Ok(None);
            };

        info!(
            "Idle server:{} with {overhead} players, min brother server{} with {bro_overhead}",
            server.server_id, bro_server.server_id
        );
        if overhead + bro_overhead >= self.config.max_players {
            Ok(None)
        } else {
            Ok(Some(bro_server))
        }
    }

    #[instrument(skip_all, fields(server_id = %server.server_id, export_to = %export_to.server_id))]
    async fn close_idle_server(&self, server: &ServerInfo, export_to: &ServerInfo) -> Result<()> {
        info!("IN");
        const FULL_LEAVES: usize = 4; // 满叶子结点为4个

        let mut export_inner = (*export_to.inner).clone();
        export_inner.zones.append(&mut server.zones.clone());
        let exported_server = ServerInfo {
            inner: export_inner.clone().into(),
        };
        // 将关闭server和接收server都注册到zone
        server.zones.iter().for_each(|zone_id| {
            self.zone_server_map.insert(
                *zone_id,
                ZoneServers {
                    server: exported_server.clone(),
                    exporting_server: Some(server.clone()),
                },
            );
        });
        // 更新接收server的原本zone的服务器信息
        export_to.zones.iter().for_each(|zone_id| {
            self.zone_server_map.insert(
                *zone_id,
                ZoneServers {
                    server: exported_server.clone(),
                    exporting_server: None,
                },
            );
        });
        // 用户导出
        loop {
            let players = server
                .map_cli
                .clone()
                .get_n_players(GetPlayersRequest {
                    n: self.config.max_players,
                })
                .await?
                .into_inner()
                .player_ids;
            if players.is_empty() {
                break;
            }
            self.transfer_players(server, export_to, &players).await?;
        }

        if exported_server.zones.len() == FULL_LEAVES {
            // 4个叶子在同一个server，合并成父节点。先插父节点，再删叶子
            let zones = exported_server.zones.clone();
            export_inner.zones = vec![zones[0] / 10];
            self.zone_server_map.insert(
                zones[0] / 10,
                ZoneServers {
                    server: ServerInfo {
                        inner: export_inner.into(),
                    },
                    exporting_server: None,
                },
            );
            zones.iter().for_each(|zone_id| {
                self.zone_server_map.remove(zone_id);
            });
        } else {
            // 完成后取消exporting_server设置
            server.zones.iter().for_each(|zone_id| {
                self.zone_server_map.insert(
                    *zone_id,
                    ZoneServers {
                        server: exported_server.clone(),
                        exporting_server: None,
                    },
                );
            });
        };

        shutdown_map_server(server).await?;

        info!("OUT");
        Ok(())
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
                .log_err();
            })
            .await;
        info!(
            "Transfered {} players from server:{} to {}",
            players.len(),
            source_server.server_id,
            target_server.server_id
        );
        Ok(())
    }
}
