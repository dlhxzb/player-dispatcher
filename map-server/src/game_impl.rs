use crate::server::Server;

use common::proto::game_service::game_service_server::GameService;
use common::proto::game_service::*;
use common::proto::map_service::{ExportRequest, InternalAoeRequest};
use common::{MapErrUnknown, RPCResult};

use anyhow::Context;
use tonic::{async_trait, Request, Response, Status};
use tracing::*;

use std::collections::HashMap;

#[async_trait]
impl GameService for Server {
    #[instrument(skip(self))]
    async fn login(&self, request: Request<PlayerInfo>) -> RPCResult<()> {
        info!("entry");
        let player = request.into_inner();
        let player_id = player.id;
        if !self.player_map.contains_key(&player.id) {
            return Err(Status::already_exists(player.id.to_string()));
        }

        let kdtree = self.kdtree.clone();
        tokio::spawn(async move {
            let _ = kdtree
                .write()
                .await
                .add([player.x, player.y], player_id)
                .map_err(|e| error!(?e));
        });
        self.player_map.insert(player_id, player);
        Ok(Response::new(()))
    }

    // Replace with `MapService::internal_aoe`
    async fn aoe(&self, request: Request<AoeRequest>) -> RPCResult<()> {
        unimplemented!()
    }

    #[instrument(skip(self))]
    async fn moving(&self, request: Request<MovingRequest>) -> RPCResult<Coord> {
        debug!("entry");
        let MovingRequest {
            id: player_id,
            dx,
            dy,
        } = request.into_inner();
        let mut player = self.get_player_from_cache(&player_id).map_err_unknown()?;
        let x0 = player.x;
        let y0 = player.y;
        player.x += dx;
        player.y += dy;
        let kdtree = self.kdtree.clone();
        tokio::spawn(async move {
            let mut guard = kdtree.write().await;
            let Ok(size) = guard.remove(&[x0, y0], &player_id).map_err(|e| error!(?e)) else {
                return;
             };
            if size > 0 {
                let _ = guard
                    .add([player.x, player.y], player_id)
                    .map_err(|e| error!(?e));
            } else {
                error!("Not found {x0} {y0} in kdtree");
            }
        });
        let coord = Coord {
            x: player.x,
            y: player.y,
        };
        self.player_map.insert(player_id, player);
        Ok(Response::new(coord))
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
