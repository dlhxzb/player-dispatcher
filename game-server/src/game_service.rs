use crate::data::*;
use crate::dispatcher::Dispatcher;
use crate::util::*;

use common::proto::game_service::game_service_server::GameService;
use common::proto::game_service::*;
use common::proto::map_service::ExportRequest;
use common::{ErrHandle, RPCResult, AABB};

use ert::prelude::RunVia;
use rayon::prelude::*;
use tonic::{async_trait, Request, Response, Status};
use tracing::*;

use std::collections::HashMap;

#[async_trait]
impl GameService for Dispatcher {
    #[instrument(skip(self))]
    async fn login(&self, request: Request<PlayerInfo>) -> RPCResult<()> {
        async fn inner_login(dsp: Dispatcher, player: PlayerInfo) -> RPCResult<()> {
            check_xy_range(player.x, player.y)?;
            if dsp.player_map.contains_key(&player.player_id) {
                return Err(Status::already_exists(format!(
                    "player_id:{} was already login",
                    player.player_id
                )));
            }
            let server = dsp.get_server_of_coord(player.x, player.y).1.server;

            server.game_cli.clone().login(player.clone()).await?;
            dsp.player_map
                .insert(player.player_id, (server, player.x, player.y));
            Ok(Response::new(()))
        }

        debug!("IN");
        let player = request.into_inner();
        let player_id = player.player_id;
        let res = inner_login(self.clone(), player).via_g(player_id).await;
        debug!(?res, "OUT");
        res
    }

    /// 根据正方形四个顶点，查找出对应的最多4个servers，给每个都发送aoe请求
    #[instrument(skip(self))]
    async fn aoe(&self, request: Request<AoeRequest>) -> RPCResult<()> {
        debug!("IN");
        let AoeRequest {
            player_id, radius, ..
        } = request.into_inner();
        let (_, x, y) = self.get_server_of_player(&player_id).map_err_unknown()?;
        check_xy_range(x, y)?;

        let xmin = x - radius;
        let xmax = x + radius;
        let ymax = y + radius;
        let ymin = y - radius;
        let tasks = [(xmin, ymin), (xmin, ymax), (xmax, ymin), (xmax, ymax)]
            .into_iter()
            .flat_map(|(x, y)| self.get_server_of_coord(x, y).1.into_vec())
            .map(|server| (server.server_id, server))
            .collect::<HashMap<_, _>>()
            .into_values()
            .map(|server| async move {
                let _ = server
                    .game_cli
                    .clone()
                    .aoe(AoeRequest {
                        player_id,
                        coord: Some(Coord { x, y }),
                        radius,
                    })
                    .await
                    .log_err();
            });
        futures::future::join_all(tasks).await;

        debug!("OUT");
        Ok(Response::new(()))
    }

    // 移动目标在当前服务器之外的要导出用户到目标服务器
    #[instrument(skip(self))]
    async fn moving(&self, request: Request<MovingRequest>) -> RPCResult<Coord> {
        async fn inner_moving(dsp: Dispatcher, request: MovingRequest) -> RPCResult<Coord> {
            let MovingRequest { player_id, dx, dy } = request.clone();
            let (current_server, x, y) = dsp.get_server_of_player(&player_id).map_err_unknown()?;

            let target_x = x + dx;
            let target_y = y + dy;
            check_xy_range(target_x, target_y)?;

            let (
                zone_id,
                ZoneServers {
                    server: target_server,
                    ..
                },
            ) = dsp.get_server_of_coord(target_x, target_y);
            let mut coord = Coord {
                x: target_x,
                y: target_y,
            };
            if !current_server.contains_zone(zone_id) {}
            if target_server != current_server {
                // 移动终点在另外一台服务器时，导出用户
                current_server
                    .map_cli
                    .clone()
                    .export_player(ExportRequest {
                        player_id,
                        addr: target_server.addr.clone(),
                        coord: Some(coord.clone()),
                    })
                    .await?;
            } else {
                // 在map服务器内移动
                coord = target_server
                    .game_cli
                    .clone()
                    .moving(request)
                    .await?
                    .into_inner();
            }
            dsp.player_map
                .insert(player_id, (target_server, coord.x, coord.y));
            Ok(Response::new(coord))
        }

        debug!("IN");
        let request = request.into_inner();
        let player_id = request.player_id;
        let res = inner_moving(self.clone(), request).via_g(player_id).await;
        debug!(?res, "OUT");
        res
    }

    #[instrument(skip(self))]
    async fn query(&self, request: Request<QueryRequest>) -> RPCResult<QueryReply> {
        debug!("IN");
        let QueryRequest {
            xmin,
            xmax,
            ymin,
            ymax,
        } = request.into_inner();
        let query_aabb = AABB {
            xmin,
            xmax,
            ymin,
            ymax,
        };
        let tasks = self
            .get_all_servers()
            .into_par_iter()
            .filter_map(|server| {
                let zone_id = if server.zones.len() == 1 {
                    server.zones[0]
                } else {
                    // server有多个zone时，返回父节点zone
                    server.zones[0] / 10
                };
                let zone_aabb = AABB::from_zone_id(zone_id);
                // 取交集
                zone_aabb
                    .get_intersection(&query_aabb)
                    .map(|aabb| async move {
                        server
                            .game_cli
                            .clone()
                            .query(QueryRequest {
                                xmin: aabb.xmin,
                                xmax: aabb.xmax,
                                ymin: aabb.ymin,
                                ymax: aabb.ymax,
                            })
                            .await
                            .map(|res| {
                                let infos = res.into_inner().infos;
                                debug!("server_id:{} infos:{}", server.server_id, infos.len());
                                infos
                            })
                    })
            })
            .collect::<Vec<_>>();
        let infos: Vec<_> = futures::future::join_all(tasks)
            .await
            .into_iter()
            .filter_map(|res| res.log_err().ok())
            .flatten()
            .collect();
        debug!("OUT: {}", infos.len());
        Ok(Response::new(QueryReply { infos }))
    }

    #[instrument(skip(self))]
    async fn logout(&self, request: Request<PlayerIdRequest>) -> RPCResult<()> {
        debug!("IN");
        let request = request.into_inner();
        let player_id = request.player_id;
        let self = self.clone();

        // ert: serialized by player_id
        async move {
            let (server, ..) = self.get_server_of_player(&player_id).map_err_unknown()?;
            server.game_cli.clone().logout(request).await
        }
        .via_g(player_id)
        .await
    }
}
