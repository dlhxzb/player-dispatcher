use crate::server::MapServer;

use common::proto::game_service::game_service_server::GameService;
use common::proto::game_service::*;
use common::*;

use rayon::prelude::*;
use tonic::{async_trait, Request, Response, Status};
use tracing::*;

#[async_trait]
impl GameService for MapServer {
    #[instrument(skip(self))]
    async fn login(&self, request: Request<PlayerInfo>) -> RPCResult<()> {
        debug!("IN");
        let self = self.clone();
        tokio::spawn(async move {
            let player = request.into_inner();
            let player_id = player.player_id;
            if self.player_map.contains_key(&player.player_id) {
                return Err(Status::already_exists(format!(
                    "player_id:{} was already login",
                    player.player_id
                )));
            }

            let grid = xy_to_grid(player.x, player.y);
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
    async fn logout(&self, request: Request<PlayerIdRequest>) -> RPCResult<()> {
        debug!("IN");
        let self = self.clone();
        tokio::spawn(async move {
            let id = request.into_inner().player_id;
            if let Some(entry) = self.player_map.remove(&id) {
                let p = entry.value();
                let grid = xy_to_grid(p.x, p.y);
                self.grid_player_map.remove(&grid);
            };
            Ok(Response::new(()))
        })
        .await
        .map_err_unknown()?
    }

    #[instrument(skip(self))]
    async fn moving(&self, request: Request<MovingRequest>) -> RPCResult<Coord> {
        debug!("IN");
        let self = self.clone();
        tokio::spawn(async move {
            let MovingRequest { player_id, dx, dy } = request.into_inner();
            let mut player = self.get_player_info(&player_id).map_err_unknown()?;
            let x0 = player.x;
            let y0 = player.y;
            player.x += dx;
            player.y += dy;

            let origin_grid = xy_to_grid(x0, y0);
            let target_grid = xy_to_grid(player.x, player.y);
            // 跨越grid，先删后插
            if target_grid != origin_grid {
                let entry = self
                    .grid_player_map
                    .get(&origin_grid)
                    .ok_or("Not in grid_player_map")
                    .map_err_unknown()?;
                if entry.value().len() <= 1 {
                    // set剩1个直接删set
                    self.grid_player_map.remove(&origin_grid);
                } else {
                    entry.value().remove(&player_id);
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
    async fn query(&self, request: Request<QueryRequest>) -> RPCResult<QueryReply> {
        debug!("IN");
        let self = self.clone();
        tokio::spawn(async move {
            let QueryRequest {
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
                        self.grid_player_map
                            .get(grid)
                            .map(|entry| entry.value().iter().map(|id| *id).collect::<Vec<_>>())
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
    async fn aoe(&self, request: Request<AoeRequest>) -> RPCResult<()> {
        debug!("IN");
        let self = self.clone();
        tokio::spawn(async move {
            let AoeRequest {
                player_id,
                coord: Some(Coord { x, y }),
                radius,
            } = request.into_inner() else {
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
                self.grid_player_map
                    .get(&grid)
                    .map(|entry| entry.value().iter().map(|id| *id).collect::<Vec<_>>())
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
                    self.player_map.insert(p.player_id, p);
                }
            });

            Ok(Response::new(()))
        })
        .await
        .map_err_unknown()?
    }
}
