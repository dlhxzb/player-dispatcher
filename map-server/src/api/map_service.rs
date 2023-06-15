use crate::server::MapServer;

use common::proto::game_service::game_service_server::GameService;
use common::proto::game_service::*;
use common::proto::map_service::map_service_server::MapService;
use common::proto::map_service::*;
use common::*;

use itertools::Itertools;
use once_cell::sync::OnceCell;
use tokio::sync::oneshot;
use tonic::{async_trait, IntoRequest, Request, Response, Status};
use tracing::*;

pub static mut SHUTDOWN_TX: OnceCell<oneshot::Sender<()>> = OnceCell::new();

#[async_trait]
impl MapService for MapServer {
    #[instrument(skip(self))]
    async fn export_player(&self, request: Request<ExportRequest>) -> RPCResult<()> {
        debug!("IN");
        let self = self.clone();
        tokio::spawn(async move {
            let ExportRequest {
                player_id,
                addr,
                coord,
            } = request.into_inner();
            let mut target_cli = self.get_export_cli(addr).await.map_err_unknown()?;
            let mut player = self.get_player_info(&player_id).map_err_unknown()?;
            if let Some(Coord { x, y }) = coord {
                player.x = x;
                player.y = y;
            }
            target_cli.import_player(player).await?;
            self.logout(PlayerIdRequest { player_id }.into_request())
                .await
        })
        .await
        .map_err_unknown()?
    }

    #[instrument(skip(self))]
    async fn import_player(&self, request: Request<PlayerInfo>) -> RPCResult<()> {
        debug!("IN");
        self.login(request).await
    }

    // 找到人数最多的zone，只有一个zone时从4个子zone中找
    #[instrument(skip_all)]
    async fn get_heaviest_zone_players(
        &self,
        request: Request<ZoneDepth>,
    ) -> RPCResult<ZonePlayersReply> {
        let depth = request.into_inner().depth;
        info!(?depth, "IN");
        let self = self.clone();
        tokio::spawn(async move {
            self.player_map
                .iter()
                .map(|entry| (*entry.key(), (entry.value().x, entry.value().y)))
                .into_group_map_by(|(_id, (x, y))| xy_to_zone_id(*x, *y, depth))
                .into_iter()
                .map(|(zone_id, value)| {
                    let (player_ids, _): (Vec<_>, Vec<_>) = value.into_iter().unzip();
                    (zone_id, player_ids)
                })
                .max_by_key(|(_zone_id, ids)| ids.len())
                .ok_or(Status::unknown("No Zone found"))
        })
        .await
        .map_err_unknown()?
        .map(|(zone_id, player_ids)| {
            info!("OUT: zone_id:{}, players:{}", zone_id, player_ids.len());
            Response::new(ZonePlayersReply {
                zone_id,
                player_ids,
            })
        })
    }

    #[instrument(skip(self))]
    async fn get_n_players(
        &self,
        request: Request<GetPlayersRequest>,
    ) -> RPCResult<GetPlayersReply> {
        info!("IN");
        let n = request.into_inner().n as usize;
        let self = self.clone();
        tokio::spawn(async move {
            let player_ids = self
                .player_map
                .iter()
                .take(n)
                .map(|entry| *entry.key())
                .collect();
            Ok(Response::new(GetPlayersReply { player_ids }))
        })
        .await
        .map_err_unknown()?
    }

    #[instrument(skip_all)]
    async fn get_overhead(&self, _request: Request<()>) -> RPCResult<OverheadReply> {
        let count = self.player_map.len() as u32;
        debug!(?count);
        Ok(Response::new(OverheadReply { count }))
    }

    #[instrument(skip_all)]
    async fn shutdown(&self, _request: Request<()>) -> RPCResult<()> {
        use tokio::time::{sleep, Duration};

        info!("IN");
        tokio::spawn(async {
            sleep(Duration::from_millis(100)).await;
            unsafe {
                SHUTDOWN_TX.take().unwrap().send(()).unwrap();
            }
        });
        Ok(Response::new(()))
    }
}
