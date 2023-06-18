use crate::server::MapServer;

use common::proto::game_service::game_service_server::GameService;
use common::proto::game_service::*;
use common::*;

use rayon::prelude::*;
use tonic::{async_trait, Request, Response, Status};
use tracing::*;

#[async_trait]
impl GameService for MapServer {
    #[instrument(skip(self),fields(addr = %self.addr))]
    async fn login(&self, request: Request<PlayerInfo>) -> RPCResult<()> {
        async fn inner_login(server: MapServer, player: PlayerInfo) -> RPCResult<()> {
            let player_id = player.player_id;
            if server.player_map.contains_key(&player.player_id) {
                return Err(Status::already_exists(format!(
                    "player_id:{} was already login",
                    player.player_id
                )));
            }

            let grid = xy_to_grid(player.x, player.y);
            server
                .grid_player_map
                .get_or_insert_with(grid, Default::default)
                .value()
                .insert(player_id);
            server.player_map.insert(player_id, player);
            Ok(Response::new(()))
        }

        debug!("IN");
        let res = tokio::spawn(inner_login(self.clone(), request.into_inner()))
            .await
            .map_err_unknown()?;
        debug!(?res, "OUT");
        res
    }

    #[instrument(skip(self),fields(addr = %self.addr,))]
    async fn logout(&self, request: Request<PlayerIdRequest>) -> RPCResult<()> {
        async fn inner_logout(server: MapServer, id: PlayerId) -> RPCResult<()> {
            if let Some(entry) = server.player_map.remove(&id) {
                let p = entry.value();
                let grid = xy_to_grid(p.x, p.y);
                let entry = server.grid_player_map.get(&grid).ok_or_else(|| {
                    Status::unknown(format!("player_id:{id} not in the grid_player_map"))
                })?;
                if entry.value().len() <= 1 {
                    server.grid_player_map.remove(&grid);
                } else {
                    entry.value().remove(&id);
                }
            };
            Ok(Response::new(()))
        }

        debug!("IN");
        let res = tokio::spawn(inner_logout(self.clone(), request.into_inner().player_id))
            .await
            .map_err_unknown()?;
        debug!(?res, "OUT");
        res
    }

    #[instrument(skip(self),fields(addr = %self.addr, player_id = %request.get_ref().player_id))]
    async fn moving(&self, request: Request<MovingRequest>) -> RPCResult<Coord> {
        async fn inner_moving(server: MapServer, request: MovingRequest) -> RPCResult<Coord> {
            let MovingRequest { player_id, dx, dy } = request;
            let mut player = server.get_player_info(&player_id).map_err_unknown()?;
            let x0 = player.x;
            let y0 = player.y;
            player.x += dx;
            player.y += dy;

            let origin_grid = xy_to_grid(x0, y0);
            let target_grid = xy_to_grid(player.x, player.y);
            // 跨越grid，先删后插
            if target_grid != origin_grid {
                let entry = server
                    .grid_player_map
                    .get(&origin_grid)
                    .ok_or("Not in grid_player_map")
                    .map_err_unknown()?;
                if entry.value().len() <= 1 {
                    // set剩1个直接删set
                    server.grid_player_map.remove(&origin_grid);
                } else {
                    entry.value().remove(&player_id);
                }
                server
                    .grid_player_map
                    .get_or_insert_with(target_grid, Default::default)
                    .value()
                    .insert(player_id);
            }

            let coord = Coord {
                x: player.x,
                y: player.y,
            };
            // 更新player_map
            server.player_map.insert(player_id, player);
            Ok(Response::new(coord))
        }

        debug!("IN");
        let res = tokio::spawn(inner_moving(self.clone(), request.into_inner()))
            .await
            .map_err_unknown()?;
        debug!(?res, "OUT");
        res
    }

    // 先找经过的grid，再逐点过滤
    #[instrument(skip_all,fields(addr = %self.addr, aabb = ?request.get_ref()))]
    async fn query(&self, request: Request<QueryRequest>) -> RPCResult<QueryReply> {
        async fn inner_query(server: MapServer, request: QueryRequest) -> Vec<PlayerInfo> {
            let QueryRequest {
                xmin,
                xmax,
                ymin,
                ymax,
            } = request;
            let aabb = AABB {
                xmin,
                xmax,
                ymin,
                ymax,
            };
            if (xmax - xmin) as usize / GRID_LENGTH * (ymax - ymin) as usize / GRID_LENGTH
                >= server.player_map.len()
            {
                // grid数量比用户还多，不用它过滤了，直接遍历所有用户
                server
                    .player_map
                    .iter()
                    .filter_map(|entry| {
                        let p = entry.value();
                        if aabb.contains(p.x, p.y) {
                            Some(p.clone())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
            } else {
                let grids = aabb.get_grids_in_aabb();
                grids
                    .par_iter()
                    .filter_map(|grid| {
                        server
                            .grid_player_map
                            .get(grid)
                            .map(|entry| entry.value().iter().map(|id| *id).collect::<Vec<_>>())
                    })
                    .flatten()
                    .filter_map(|id| server.player_map.get(&id))
                    .filter_map(|entry| {
                        let p = entry.value();
                        if aabb.contains(p.x, p.y) {
                            Some(p.clone())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
            }
        }

        debug!("IN");
        let res = tokio::spawn(inner_query(self.clone(), request.into_inner()))
            .await
            .map_err_unknown()
            .map(|infos| Response::new(QueryReply { infos }));
        debug!(?res, "OUT");
        res
    }

    #[instrument(skip(self),fields(addr = %self.addr))]
    async fn aoe(&self, request: Request<AoeRequest>) -> RPCResult<()> {
        async fn inner_aoe(server: MapServer, request: AoeRequest) -> RPCResult<()> {
            let AoeRequest {
                player_id,
                coord: Some(Coord { x, y }),
                radius,
            } = request else {
                return Err(Status::data_loss("Coord { x, y }"));
            };
            AABB {
                xmin: x - radius,
                xmax: x + radius,
                ymin: y - radius,
                ymax: y + radius,
            }
            .get_grids_in_aabb()
            .into_par_iter()
            .filter_map(|grid| {
                server
                    .grid_player_map
                    .get(&grid)
                    .map(|entry| entry.value().iter().map(|id| *id).collect::<Vec<_>>())
            })
            .flatten()
            .filter_map(|id| {
                // 过滤掉自己
                if id != player_id {
                    server
                        .player_map
                        .get(&id)
                        .map(|entry| entry.value().clone())
                } else {
                    None
                }
            })
            .for_each(|mut p| {
                if (p.x - x) * (p.x - x) + (p.y - y) * (p.y - y) <= radius * radius {
                    p.money += AOE_MONEY;
                    server.player_map.insert(p.player_id, p);
                }
            });

            Ok(Response::new(()))
        }

        debug!("IN");
        let res = tokio::spawn(inner_aoe(self.clone(), request.into_inner()))
            .await
            .map_err_unknown()?;
        debug!(?res, "OUT");
        res
    }
}
