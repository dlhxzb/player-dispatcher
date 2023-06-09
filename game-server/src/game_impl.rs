use crate::dispatcher::Dispatcher;
use crate::util::*;

use common::proto::game_service::game_service_server::GameService;
use common::proto::game_service::*;
use common::proto::map_service::{ExportRequest, InternalAoeRequest};
use common::{MapErrUnknown, RPCResult, AABB};

use ert::prelude::RunVia;
use rayon::prelude::*;
use tonic::{async_trait, Request, Response, Status};
use tracing::*;

use std::collections::HashMap;

#[async_trait]
impl GameService for Dispatcher {
    #[instrument(skip(self))]
    async fn login(&self, request: Request<PlayerInfo>) -> RPCResult<()> {
        info!("entry");
        let player = request.into_inner();
        let player_id = player.id;
        let self = self.clone();
        async move {
            check_xy_range(player.x, player.y)?;
            if !self.player_map.contains_key(&player.id) {
                return Err(Status::already_exists(player.id.to_string()));
            }
            let server = self.get_server_of_coord(player.x, player.y).1.server;

            server
                .map_cli
                .clone()
                .internal_login(player.clone())
                .await?;
            self.player_map
                .insert(player_id, (server, player.x, player.y));
            Ok(Response::new(()))
        }
        .via_g(player_id)
        .await
    }

    /// 根据正方形四个顶点，查找出对应的最多4个servers，给每个都发送aoe请求
    #[instrument(skip(self))]
    async fn aoe(&self, request: Request<AoeRequest>) -> RPCResult<()> {
        debug!("entry");
        let AoeRequest {
            id: player_id,
            radius,
        } = request.into_inner();
        let (_, x, y) = self.get_server_of_player(&player_id).map_err_unknown()?;
        check_xy_range(x, y)?;

        let xmin = x - radius;
        let xmax = x + radius;
        let ymax = y + radius;
        let ymin = y - radius;
        let tasks = [(xmin, ymin), (xmin, ymax), (xmax, ymin), (xmax, ymax)]
            .into_iter()
            .map(|(x, y)| self.get_server_of_coord(x, y).1.into_vec())
            .flatten()
            .map(|server| (server.server_id, server))
            .collect::<HashMap<_, _>>()
            .into_values()
            .map(|server| async move {
                if let Err(e) = server
                    .map_cli
                    .clone()
                    .internal_aoe(InternalAoeRequest {
                        player_id,
                        x,
                        y,
                        radius,
                    })
                    .await
                {
                    error!(?e);
                }
            });
        futures::future::join_all(tasks).await;

        Ok(Response::new(()))
    }

    // 移动目标在当前服务器之外的要导出用户到目标服务器
    #[instrument(skip(self))]
    async fn moving(&self, request: Request<MovingRequest>) -> RPCResult<Coord> {
        debug!("entry");
        let request = request.into_inner();
        let MovingRequest {
            id: player_id,
            dx,
            dy,
        } = request.clone();
        let self = self.clone();

        // ert: serialized by player_id
        async move {
            let (current_server, x, y) = self.get_server_of_player(&player_id).map_err_unknown()?;

            let target_x = x + dx;
            let target_y = y + dy;
            check_xy_range(target_x, target_y)?;

            let (
                zone_id,
                ZoneServers {
                    server: target_server,
                    ..
                },
            ) = self.get_server_of_coord(target_x, target_y);
            let coord = Coord {
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
            }
            let Coord { x, y } = target_server
                .map_cli
                .clone()
                .internal_moving(request)
                .await?
                .into_inner();
            self.player_map.insert(player_id, (target_server, x, y));
            Ok(Response::new(coord))
        }
        .via_g(player_id)
        .await
    }

    #[instrument(skip(self))]
    async fn query(&self, request: Request<QueryRequest>) -> RPCResult<QueryReply> {
        debug!("entry");
        let QueryRequest {
            id,
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
                            .map_cli
                            .clone()
                            .internal_query(QueryRequest {
                                id,
                                xmin: aabb.xmin,
                                xmax: aabb.xmax,
                                ymin: aabb.ymin,
                                ymax: aabb.ymax,
                            })
                            .await
                            .map(|res| res.into_inner().infos)
                    })
            })
            .collect::<Vec<_>>();
        let infos = futures::future::join_all(tasks)
            .await
            .into_iter()
            .filter_map(|res| match res {
                Ok(infos) => Some(infos),
                Err(e) => {
                    error!(?e);
                    None
                }
            })
            .flatten()
            .collect();

        Ok(Response::new(QueryReply { infos }))
    }

    #[instrument(skip(self))]
    async fn logout(&self, request: Request<PlayerIdRequest>) -> RPCResult<()> {
        info!("entry");
        let request = request.into_inner();
        let player_id = request.id;
        let self = self.clone();

        // ert: serialized by player_id
        async move {
            let (server, ..) = self.get_server_of_player(&player_id).map_err_unknown()?;
            server.map_cli.clone().internal_logout(request).await
        }
        .via_g(player_id)
        .await
    }
}
