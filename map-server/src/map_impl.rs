use crate::server::Server;

use common::proto::game_service::PlayerInfo;
use common::proto::game_service::*;
use common::proto::map_service::map_service_server::MapService;
use common::proto::map_service::*;
use common::{get_aabb_grids, get_xy_grid, MapErrUnknown, RPCResult};

use rayon::prelude::*;
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
    async fn internal_login(&self, request: Request<PlayerInfo>) -> RPCResult<()> {
        info!("entry");
        let player = request.into_inner();
        let player_id = player.id;
        if self.player_map.contains_key(&player.id) {
            return Err(Status::already_exists(player.id.to_string()));
        }

        let grid = get_xy_grid(player.x, player.y);
        self.grid_player_map
            .get_or_insert_with(grid, Default::default)
            .value()
            .insert(player_id);
        self.player_map.insert(player_id, player);
        Ok(Response::new(()))
    }

    #[instrument(skip(self))]
    async fn internal_moving(&self, request: Request<MovingRequest>) -> RPCResult<Coord> {
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

        let origin_grid = get_xy_grid(x0, y0);
        let target_grid = get_xy_grid(player.x, player.y);
        // 跨越grid，先删后插
        if target_grid != origin_grid {
            let set = self
                .grid_player_map
                .get(&origin_grid)
                .map(|entry| entry.value().clone())
                .ok_or("Not in grid_player_map")
                .map_err_unknown()?;
            if set.len() <= 1 {
                // set剩1个直接删set
                self.grid_player_map.remove(&origin_grid);
            } else {
                set.remove(&player_id);
            }
            self.grid_player_map
                .get_or_insert_with(target_grid, Default::default)
                .value()
                .insert(player_id);
        }

        let coord = Coord {
            x: player.x,
            y: player.y,
        };
        debug!(?player);
        // 更新player_map
        self.player_map.insert(player_id, player);
        Ok(Response::new(coord))
    }

    // 先找经过的grid，再逐点过滤
    #[instrument(skip(self))]
    async fn internal_query(&self, request: Request<QueryRequest>) -> RPCResult<QueryReply> {
        debug!("entry");
        let QueryRequest {
            id: _player_id,
            xmin,
            xmax,
            ymin,
            ymax,
        } = request.into_inner();

        let infos = get_aabb_grids(xmin, xmax, ymin, ymax)
            .into_par_iter()
            .filter_map(|grid| {
                self.grid_player_map.get(&grid).map(|entry| {
                    entry
                        .value()
                        .clone()
                        .iter()
                        .map(|id| *id)
                        .collect::<Vec<_>>()
                })
            })
            .flatten()
            .filter_map(|id| self.player_map.get(&id).map(|entry| entry.value().clone()))
            .filter(|p| p.x >= xmin && p.x <= xmax && p.y >= ymin && p.y <= ymax) // 逐点过滤
            .collect();
        Ok(Response::new(QueryReply { infos }))
    }

    #[instrument(skip(self))]
    async fn internal_logout(&self, request: Request<PlayerIdRequest>) -> RPCResult<()> {
        info!("entry");
        todo!()
    }

    #[instrument(skip(self))]
    async fn internal_aoe(&self, request: Request<InternalAoeRequest>) -> RPCResult<()> {
        const AOE_MONEY: u64 = 1_u64;

        debug!("entry");
        let InternalAoeRequest {
            player_id,
            x,
            y,
            radius,
        } = request.into_inner();
        // let player_map = self.player_map.clone();
        // let kdtree = self.kdtree.clone();
        // // 此处不在意时效性，读锁也没有并发一致性问题，可spawn出去
        // tokio::spawn(async move {
        //     kdtree
        //         .read()
        //         .await
        //         .within(&[x, y], radius * radius, &squared_euclidean)
        //         .map_err(|e| error!(?e))?
        //         .into_iter()
        //         .filter(|(_dis, id)| **id != player_id)
        //         .for_each(|(_dis, id)| {
        //             if let Some(entry) = player_map.get(id) {
        //                 let mut player = entry.value().clone();
        //                 player.money += AOE_MONEY;
        //                 player_map.insert(*id, player);
        //             } else {
        //                 error!("{id} in kdtree but not in player_map");
        //             }
        //         });
        //     Ok::<(), ()>(())
        // });

        Ok(Response::new(()))
    }
}
