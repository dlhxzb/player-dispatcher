use crate::server::Server;

use common::proto::game_service::PlayerInfo;
use common::proto::map_service::map_service_server::MapService;
use common::proto::map_service::*;
use common::{MapErrUnknown, RPCResult};

use tonic::{async_trait, Request, Response, Status};
use tracing::*;

#[async_trait]
impl MapService for Server {
    async fn export_player(&self, request: Request<ExportRequest>) -> RPCResult<()> {
        todo!()
    }
    async fn import_player(&self, request: Request<PlayerInfo>) -> RPCResult<()> {
        todo!()
    }
    async fn get_heaviest_zone_players(&self, request: Request<()>) -> RPCResult<ZonePlayersReply> {
        todo!()
    }
    async fn get_n_players(
        &self,
        request: Request<GetPlayersRequest>,
    ) -> RPCResult<GetPlayersReply> {
        todo!()
    }
    async fn connect_server(&self, request: Request<ConnectRequest>) -> RPCResult<()> {
        todo!()
    }
    async fn disconnect_server(&self, request: Request<ConnectRequest>) -> RPCResult<()> {
        todo!()
    }
    async fn get_overhead(&self, request: Request<()>) -> RPCResult<OverheadReply> {
        todo!()
    }

    #[instrument(skip(self))]
    async fn internal_aoe(&self, request: Request<InternalAoeRequest>) -> RPCResult<()> {
        use kdtree::distance::squared_euclidean;

        const AOE_MONEY: u64 = 1_u64;
        debug!("entry");
        let InternalAoeRequest { x, y, radius } = request.into_inner();
        // let player_map = self.player_map.clone();
        // let kdtree = self.kdtree.clone();
        // tokio::spawn(async move{

        // });

        let player_ids = self
            .kdtree
            .read()
            .await
            .within(&[x, y], radius * radius, &squared_euclidean)
            .map_err_unknown()?
            .into_iter()
            .map(|(_dis, player_id)| *player_id);

        Ok(Response::new(()))
    }
}
