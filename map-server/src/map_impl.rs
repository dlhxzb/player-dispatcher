use crate::server::Server;

use common::proto::game_service::PlayerInfo;
use common::proto::game_service::*;
use common::proto::map_service::map_service_client::MapServiceClient;
use common::proto::map_service::map_service_server::MapService;
use common::proto::map_service::*;
use common::{get_xy_grid, MapErrUnknown, RPCResult, AABB, AOE_MONEY};

use rayon::prelude::*;
use tonic::codegen::http::request;
use tonic::{async_trait, IntoRequest, Request, Response, Status};
use tracing::*;

#[async_trait]
impl MapService for Server {
    async fn export_player(&self, request: Request<ExportRequest>) -> RPCResult<()> {
        info!("entry");
        let self = self.clone();
        tokio::spawn(async move {
            let ExportRequest {
                player_id,
                addr,
                coord,
            } = request.into_inner();
            let mut target_cli = {
                let mut guard = self.export_cli.lock().await;
                match &*guard {
                    Some((saved_addr, saved_cli)) if saved_addr == &addr => saved_cli.clone(),
                    _ => {
                        let map_cli = MapServiceClient::connect(addr.clone())
                            .await
                            .map_err_unknown()?;
                        *guard = Some((addr, map_cli.clone()));
                        map_cli
                    }
                }
            };
            let mut player = self.get_player_info(&player_id).map_err_unknown()?;
            if let Some(Coord { x, y }) = coord {
                player.x = x;
                player.y = y;
            }
            target_cli.internal_login(player.into_request()).await
        })
        .await
        .map_err_unknown()?
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
        let self = self.clone();
        tokio::spawn(async move {
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
        })
        .await
        .map_err_unknown()?
    }

    #[instrument(skip(self))]
    async fn internal_moving(&self, request: Request<MovingRequest>) -> RPCResult<Coord> {
        debug!("entry");
        let self = self.clone();
        tokio::spawn(async move {
            let MovingRequest {
                id: player_id,
                dx,
                dy,
            } = request.into_inner();
            let mut player = self.get_player_info(&player_id).map_err_unknown()?;
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
        })
        .await
        .map_err_unknown()?
    }

    // 先找经过的grid，再逐点过滤
    #[instrument(skip(self))]
    async fn internal_query(&self, request: Request<QueryRequest>) -> RPCResult<QueryReply> {
        debug!("entry");
        let self = self.clone();
        tokio::spawn(async move {
            let QueryRequest {
                id: _player_id,
                xmin,
                xmax,
                ymin,
                ymax,
            } = request.into_inner();
            let aabb = AABB {
                xmin,
                xmax,
                ymin,
                ymax,
            };
            let grids = aabb.get_grids_in_aabb();
            let infos = if grids.len() > self.player_map.len() {
                // grid数量比用户还多，不用它过滤了，直接遍历所有用户
                self.player_map
                    .iter()
                    .filter_map(|entry| {
                        let p = entry.value();
                        if aabb.contains(p.x, p.y) {
                            Some(p.clone())
                        } else {
                            None
                        }
                    })
                    .collect()
            } else {
                grids
                    .par_iter()
                    .filter_map(|grid| {
                        self.grid_player_map.get(grid).map(|entry| {
                            entry
                                .value()
                                // .clone()
                                .iter()
                                .map(|id| *id)
                                .collect::<Vec<_>>()
                        })
                    })
                    .flatten()
                    .filter_map(|id| self.player_map.get(&id))
                    .filter_map(|entry| {
                        let p = entry.value();
                        if aabb.contains(p.x, p.y) {
                            Some(p.clone())
                        } else {
                            None
                        }
                    })
                    .collect()
            };
            Ok(Response::new(QueryReply { infos }))
        })
        .await
        .map_err_unknown()?
    }

    #[instrument(skip(self))]
    async fn internal_logout(&self, request: Request<PlayerIdRequest>) -> RPCResult<()> {
        info!("entry");
        let self = self.clone();
        tokio::spawn(async move {
            let id = request.into_inner().id;
            self.player_map.remove(&id).map(|entry| {
                let p = entry.value();
                let grid = get_xy_grid(p.x, p.y);
                self.grid_player_map.remove(&grid);
            });
            Ok(Response::new(()))
        })
        .await
        .map_err_unknown()?
    }

    #[instrument(skip(self))]
    async fn internal_aoe(&self, request: Request<InternalAoeRequest>) -> RPCResult<()> {
        debug!("entry");
        let self = self.clone();
        tokio::spawn(async move {
            let InternalAoeRequest {
                player_id,
                x,
                y,
                radius,
            } = request.into_inner();
            AABB {
                xmin: x - radius,
                xmax: x + radius,
                ymin: y - radius,
                ymax: y + radius,
            }
            .get_grids_in_aabb()
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
            .filter_map(|id| {
                // 过滤掉自己
                if id != player_id {
                    self.player_map.get(&id).map(|entry| entry.value().clone())
                } else {
                    None
                }
            })
            .for_each(|mut p| {
                if (p.x - x) * (p.x - x) + (p.y - y) * (p.y - y) <= radius * radius {
                    p.money += AOE_MONEY;
                    self.player_map.insert(p.id, p);
                }
            });

            Ok(Response::new(()))
        })
        .await
        .map_err_unknown()?
    }
}
