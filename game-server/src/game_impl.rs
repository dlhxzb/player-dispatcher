use crate::dispatcher::Dispatcher;
use crate::util::*;
use crate::ZoneServers;

use common::proto::game_service::game_service_server::GameService;
use common::proto::game_service::*;
use common::proto::map_service::{ExportRequest, InternalAoeRequest};
use common::{MapErrUnknown, RPCResult};

use ert::prelude::RunVia;
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
            check_xy(player.x, player.y)?;
            if !self.player_map.contains_key(&player.id) {
                return Err(Status::already_exists(player.id.to_string()));
            }
            let (_, ZoneServers { server, .. }) = self.get_server_of_coord(player.x, player.y);

            server.game_cli.clone().login(player.clone()).await?;
            self.player_map
                .insert(player_id, (server, player.x, player.y));
            Ok(Response::new(()))
        }
        .via_g(player_id)
        .await
    }

    /// 根据正方形四个顶点，查找出对应的servers，给每个都发送aoe请求
    #[instrument(skip(self))]
    async fn aoe(&self, request: Request<AoeRequest>) -> RPCResult<()> {
        debug!("entry");
        let AoeRequest {
            id: player_id,
            radius,
        } = request.into_inner();
        let (_, x, y) = self.get_player_from_cache(&player_id).map_err_unknown()?;
        check_xy(x, y)?;

        let xmin = x - radius;
        let xmax = x + radius;
        let ymax = y + radius;
        let ymin = y - radius;
        let tasks = [(xmin, ymin), (xmin, ymax), (xmax, ymin), (xmax, ymax)]
            .into_iter()
            .map(|(x, y)| {
                let (
                    _,
                    ZoneServers {
                        server,
                        exporting_server,
                    },
                ) = self.get_server_of_coord(x, y);
                if let Some(export) = exporting_server {
                    vec![(server.server_id, server), (export.server_id, export)]
                } else {
                    vec![(server.server_id, server)]
                }
            })
            .flatten()
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
        async move {
            let (current_server, x, y) =
                self.get_player_from_cache(&player_id).map_err_unknown()?;

            let target_x = x + dx;
            let target_y = y + dy;
            check_xy(target_x, target_y)?;

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
                // 移动到当前服务器之外的区域
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
                .game_cli
                .clone()
                .moving(request)
                .await?
                .into_inner();
            self.player_map.insert(player_id, (target_server, x, y));
            Ok(Response::new(coord))
        }
        .via_g(player_id)
        .await
    }

    #[instrument(skip(self))]
    async fn query(&self, _player_id: Request<QueryRequest>) -> RPCResult<QueryReply> {
        debug!("entry");
        todo!()
    }

    #[instrument(skip(self))]
    async fn logout(&self, _player_id: Request<PlayerIdRequest>) -> RPCResult<()> {
        info!("entry");
        todo!()
    }
}
