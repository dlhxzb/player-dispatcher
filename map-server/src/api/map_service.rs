use crate::server::MapServer;

use common::proto::game_service::game_service_server::GameService;
use common::proto::game_service::*;
use common::proto::map_service::map_service_server::MapService;
use common::proto::map_service::*;
use common::*;

use itertools::Itertools;
use tonic::{async_trait, IntoRequest, Request, Response};
use tracing::*;

#[async_trait]
impl MapService for MapServer {
    async fn export_player(&self, request: Request<ExportRequest>) -> RPCResult<()> {
        info!("IN");
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

    async fn import_player(&self, request: Request<PlayerInfo>) -> RPCResult<()> {
        self.login(request).await
    }

    // 找到人数最多的zone，只有一个zone时从4个子zone中找
    async fn get_heaviest_zone_players(
        &self,
        request: Request<ZoneDepth>,
    ) -> RPCResult<ZonePlayersReply> {
        info!("IN");
        let depth = request.into_inner().depth;
        let self = self.clone();
        tokio::spawn(async move {
            let (zone_id, player_ids) = self
                .player_map
                .iter()
                .map(|entry| (*entry.key(), (entry.value().x, entry.value().y)))
                .into_group_map_by(|(_id, (x, y))| xy_to_zone_id(*x, *y, depth))
                .into_iter()
                .map(|(zone_id, value)| {
                    let (player_ids, _): (Vec<_>, Vec<_>) = value.into_iter().unzip();
                    (zone_id, player_ids)
                })
                .max_by_key(|(_zone_id, ids)| ids.len())
                .unwrap_or_default();
            info!("zone_id:{}, players:{}", zone_id, player_ids.len());
            Ok(Response::new(ZonePlayersReply {
                zone_id,
                player_ids,
            }))
        })
        .await
        .map_err_unknown()?
    }

    async fn get_n_players(
        &self,
        request: Request<GetPlayersRequest>,
    ) -> RPCResult<GetPlayersReply> {
        info!("IN");
        let n = request.into_inner().n;
        let self = self.clone();
        tokio::spawn(async move {
            let mut count = 0;
            let mut player_ids = Vec::with_capacity(n as usize);
            let mut iter = self.player_map.iter();
            while let Some(entry) = iter.next() {
                if count == n {
                    break;
                }
                player_ids.push(*entry.key());
                count += 1;
            }
            Ok(Response::new(GetPlayersReply { player_ids }))
        })
        .await
        .map_err_unknown()?
    }
    async fn get_overhead(&self, _request: Request<()>) -> RPCResult<OverheadReply> {
        info!("IN");
        let count = self.player_map.len() as u32;
        Ok(Response::new(OverheadReply { count }))
    }

    async fn shutdown(&self, _request: Request<()>) -> RPCResult<()> {
        todo!()
    }
}
