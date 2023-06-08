pub mod proto;

use tonic::{Response, Status};

pub type RPCResult<T> = Result<Response<T>, Status>;

// 世界地图尺寸
pub const WORLD_X_MAX: f32 = 1_000_000.0;
pub const WORLD_Y_MAX: f32 = 1_000_000.0;
pub const WORLD_X_MIN: f32 = -WORLD_X_MAX;
pub const WORLD_Y_MIN: f32 = -WORLD_Y_MAX;
pub const GRID_LENTH: usize = 100;

pub trait MapErrUnknown {
    type S;
    fn map_err_unknown(self) -> std::result::Result<Self::S, Status>;
}

impl<T, E: std::fmt::Debug> MapErrUnknown for Result<T, E> {
    type S = T;
    fn map_err_unknown(self) -> Result<Self::S, Status> {
        self.map_err(|e| {
            let s = format!("{e:?}");
            log::error!("{}", s);
            Status::unknown(s)
        })
    }
}

#[inline]
pub fn get_xy_grid(x: f32, y: f32) -> (usize, usize) {
    (
        (x - WORLD_X_MIN) as usize / GRID_LENTH,
        (y - WORLD_Y_MIN) as usize / GRID_LENTH,
    )
}

pub fn get_aabb_grids(xmin: f32, xmax: f32, ymin: f32, ymax: f32) -> Vec<(usize, usize)> {
    let grid_min = get_xy_grid(xmin, ymin);
    let grid_max = get_xy_grid(xmax, ymax);
    let mut set = Vec::new();
    for x in grid_min.0..=grid_max.0 {
        for y in grid_min.1..=grid_max.1 {
            set.push((x, y));
        }
    }
    set
}
