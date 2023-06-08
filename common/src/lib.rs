pub mod proto;

use tonic::{Response, Status};

pub type RPCResult<T> = Result<Response<T>, Status>;

// 世界地图尺寸
pub const WORLD_X_MAX: f32 = 1_000_000.0;
pub const WORLD_Y_MAX: f32 = 1_000_000.0;
pub const WORLD_X_MIN: f32 = -WORLD_X_MAX;
pub const WORLD_Y_MIN: f32 = -WORLD_Y_MAX;
pub const MAX_PLAYER: u64 = 1000; // 服务器最大用户数，触发扩容
pub const MIN_PLAYER: u64 = MAX_PLAYER / 4; // 服务器最小用户数，触发缩容
pub const MAX_ZONE_DEPTH: u32 = 10; // 四叉树最大深度
pub const GRID_LENGTH: usize = 100; // Grid边长
pub const AOE_MONEY: u64 = 1_u64; // 每次aoe给周边玩家增加的钱数

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
        (x - WORLD_X_MIN) as usize / GRID_LENGTH,
        (y - WORLD_Y_MIN) as usize / GRID_LENGTH,
    )
}

pub struct AABB {
    pub xmin: f32,
    pub xmax: f32,
    pub ymin: f32,
    pub ymax: f32,
}
impl AABB {
    // 获取AABB范围内所有grids
    pub fn get_grids_in_aabb(&self) -> Vec<(usize, usize)> {
        let grid_min = get_xy_grid(self.xmin, self.ymin);
        let grid_max = get_xy_grid(self.xmax, self.ymax);
        let mut set = Vec::new();
        for x in grid_min.0..=grid_max.0 {
            for y in grid_min.1..=grid_max.1 {
                set.push((x, y));
            }
        }
        set
    }
    // 判断点是否在AABB内
    #[inline]
    pub fn contains(&self, x: f32, y: f32) -> bool {
        self.x >= xmin && self.x <= xmax && self.y >= ymin && self.y <= ymax
    }
    // 判断2个AABB是否有重叠
    pub fn check_crossed(&self, other: &Self) -> bool {
        [
            (self.xmin, self.ymin),
            (self.xmin, self.ymax),
            (self.xmax, self.ymin),
            (self.xmax, self.ymax),
        ]
        .into_iter()
        .any(|(x, y)| other.contains(x, y))
            || [
                (other.xmin, other.ymin),
                (other.xmin, other.ymax),
                (other.xmax, other.ymin),
                (other.xmax, other.ymax),
            ]
            .into_iter()
            .any(|(x, y)| self.contains(x, y))
    }
}
