use crate::dispatcher::Dispatcher;
use crate::util::*;

use proto::game_service::game_service_server::GameService;
use proto::game_service::{Coord, CoordRequest, PlayerIdRequest, PlayerInfo};
use proto::map_service::ExportRequest;

use ert::prelude::RunVia;
use tonic::{async_trait, Request, Response, Status};
use tracing::error;

pub type RPCResult<T> = Result<Response<T>, Status>;

#[async_trait]
impl GameService for Dispatcher {
    async fn login(&self, player: Request<PlayerInfo>) -> RPCResult<()> {
        let player = player.into_inner();
        let player_id = player.id;
        let self = self.clone();
        async move {
            self.check_xy(player.x, player.y)?;
            if !self.player_map.contains_key(&player.id) {
                return Err(Status::already_exists(player.id.to_string()));
            }
            let zone_id = xy_to_zone(player.x, player.y);
            let server = self.get_best_server_of_zone(zone_id);

            server.game_cli.clone().login(player).await?;
            self.player_map.insert(player_id, server);
            Ok(Response::new(()))
        }
        .via_g(player_id)
        .await
    }

    /// 根据正方形四个顶点，查找出对应的servers，给每个都发送aoe请求
    async fn aoe(&self, request: Request<CoordRequest>) -> RPCResult<()> {
        use std::collections::HashMap;

        const AOE_RADIUS: u64 = 10;
        let inner = request.into_inner();
        let CoordRequest{id,target:Some(coord)} = inner.clone() else {
            return Err(Status::invalid_argument("target is None"));
        };
        self.check_player_exist(id)?;
        self.check_xy(coord.x, coord.y)?;

        let left = coord.x.saturating_sub(AOE_RADIUS);
        let right = coord.x + AOE_RADIUS;
        let top = coord.y + AOE_RADIUS;
        let bottom = coord.y.saturating_sub(AOE_RADIUS);
        let tasks = [(left, bottom), (left, top), (right, bottom), (right, top)]
            .into_iter()
            .map(|(x, y)| {
                let zone_id = xy_to_zone(x, y);
                self.get_servers_of_zone(zone_id)
                    .into_iter()
                    .map(|s| (s.server_id, s))
            })
            .flatten()
            .collect::<HashMap<_, _>>()
            .into_values()
            .map(|server| {
                let request = inner.clone();
                async move {
                    if let Err(e) = server.game_cli.clone().aoe(request).await {
                        error!(?e);
                    }
                }
            });
        futures::future::join_all(tasks).await;

        Ok(Response::new(()))
    }

    async fn moving(&self, request: Request<CoordRequest>) -> RPCResult<Coord> {
        let CoordRequest{id:player_id,target:Some(target)} = request.into_inner() else {
            return Err(Status::invalid_argument("target is None"));
        };

        let Some(entry) = self.player_map.get(&player_id) else {
            return Err(Status::permission_denied("Please login first"))
        };
        self.check_xy(target.x, target.y)?;
        let current_server = entry.value().clone();
        let zone_id = xy_to_zone(target.x, target.y);
        if !server_contains_zone(&current_server, zone_id) {
            // 移动到当前服务器之外的区域
            let target_server = self.get_best_server_of_zone(zone_id);
            current_server
                .map_cli
                .clone()
                .export_player(ExportRequest {
                    player_id,
                    addr: target_server.addr.clone(),
                    coord: Some(target),
                })
                .await?;
        }
        todo!()
    }

    async fn query(&self, _player_id: Request<PlayerIdRequest>) -> RPCResult<PlayerInfo> {
        todo!()
    }

    async fn logout(&self, _player_id: Request<PlayerIdRequest>) -> RPCResult<()> {
        todo!()
    }
}
