use std::sync::Arc;

use anyhow::Error;
use ert::prelude::RunVia;
use tonic::transport::Channel;
use tonic::{async_trait, Request, Response, Status};
use tracing::error;

use game_service::game_service_server::GameService;
use game_service::{Coord, PlayerInfo, WalkingRequest};

pub mod map_service {
    tonic::include_proto!("map_service");
}
pub mod game_service {
    tonic::include_proto!("game_service");
}

use crate::dispatcher::{Dispatcher, WORLD_X_MAX, WORLD_Y_MAX};
use crate::util::*;

pub type RPCResult<T> = Result<Response<T>, Status>;

#[async_trait]
impl GameService for Arc<Dispatcher> {
    async fn login(&self, player: Request<PlayerInfo>) -> RPCResult<()> {
        let player = player.into_inner();
        let player_id = player.id;
        let dp = self.clone();
        async move {
            check_xy(player.x, player.y)?;
            if !dp.player_map.contains_key(&player.id) {
                return Err(Status::already_exists(player.id.to_string()));
            }
            let zone_id = xy_to_zone(player.x, player.y);
            let server = dp.get_best_server_of_zone(zone_id);

            server.game_cli.clone().login(player).await?;
            dp.player_map.insert(player_id, server.server_id);
            Ok(Response::new(()))
        }
        .via_g(player_id)
        .await
    }

    /// 根据正方形四个顶点，查找出对应的servers，给每个都发送aoe请求
    async fn aoe(&self, coord: Request<Coord>) -> RPCResult<()> {
        use std::collections::HashMap;

        const AOE_RADIUS: u64 = 10;

        let coord = coord.into_inner();
        check_xy(coord.x, coord.y)?;

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
            .map(|mut server| {
                let coord = coord.clone();
                async move {
                    if let Err(e) = server.game_cli.clone().aoe(coord).await {
                        error!(?e);
                    }
                }
            });
        futures::future::join_all(tasks).await;

        Ok(Response::new(()))
    }

    async fn walking(&self, step: Request<WalkingRequest>) -> RPCResult<Coord> {
        todo!()
    }
}
