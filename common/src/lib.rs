pub mod proto;

use tonic::{Response, Status};

pub type RPCResult<T> = Result<Response<T>, Status>;

pub type PlayerId = u64;
pub type ZoneId = u64;
pub type ServerId = u32;
pub type GridId = (usize, usize);

// 世界地图尺寸
pub const WORLD_X_MAX: f32 = 1_000_000.0;
pub const WORLD_Y_MAX: f32 = 1_000_000.0;
pub const WORLD_X_MIN: f32 = -WORLD_X_MAX;
pub const WORLD_Y_MIN: f32 = -WORLD_Y_MAX;
pub const MAX_PLAYER: u32 = 1000; // 服务器最大用户数，触发扩容
pub const MIN_PLAYER: u32 = MAX_PLAYER / 4; // 服务器最小用户数，触发缩容
pub const MAX_ZONE_DEPTH: u32 = 10; // 四叉树最大深度
pub const GRID_LENGTH: usize = 100; // Grid边长
pub const AOE_MONEY: u64 = 1; // 每次aoe给周边玩家增加的钱数
pub const ROOT_ZONE_ID: ZoneId = 1;

pub const GAME_PORT_ENV_NAME: &str = "GAME_SERVER_PORT";
pub const MAP_PORT_ENV_NAME: &str = "MAP_SERVER_PORT";
pub const DEFAULT_GAME_PORT: u32 = 4880;
pub const DEFAULT_MAP_PORT: u32 = 5000;

pub trait ErrHandle {
    type S;
    type R;
    fn map_err_unknown(self) -> std::result::Result<Self::S, Status>;
    fn log_err(self) -> std::result::Result<Self::S, Self::R>;
}

impl<T, E: std::fmt::Debug> ErrHandle for Result<T, E> {
    type S = T;
    type R = E;
    fn map_err_unknown(self) -> Result<Self::S, Status> {
        self.map_err(|e| {
            let s = format!("{e:?}");
            log::error!("{}", s);
            Status::unknown(s)
        })
    }

    fn log_err(self) -> std::result::Result<Self::S, Self::R> {
        self.map_err(|e| {
            let s = format!("{e:?}");
            log::error!("{}", s);
            e
        })
    }
}

// zone范围均为左闭右开，根节点depth=1
pub fn xy_to_zone_id(x: f32, y: f32, depth: u32) -> ZoneId {
    assert_ne!(depth, 0);
    let mut id = ROOT_ZONE_ID;
    let mut origin_x = 0.0;
    let mut origin_y = 0.0;
    let mut length = WORLD_X_MAX;
    let mut height = WORLD_Y_MAX;
    for _ in 1..depth {
        length /= 2.0;
        height /= 2.0;
        let quadrant = if y >= origin_y {
            origin_y += height;
            if x >= origin_x {
                origin_x += length;
                1 // 第1象限
            } else {
                origin_x -= length;
                2
            }
        } else {
            origin_y -= height;
            if x < origin_x {
                origin_x -= length;
                3
            } else {
                origin_x += length;
                4
            }
        };
        id = id * 10 + quadrant;
    }
    id
}

#[inline]
pub fn zone_depth(id: ZoneId) -> u32 {
    id.ilog10() + 1
}

#[inline]
pub fn xy_to_grid(x: f32, y: f32) -> GridId {
    (
        (x - WORLD_X_MIN) as usize / GRID_LENGTH,
        (y - WORLD_Y_MIN) as usize / GRID_LENGTH,
    )
}

#[derive(Debug, Clone, PartialEq)]
pub struct AABB {
    pub xmin: f32,
    pub xmax: f32,
    pub ymin: f32,
    pub ymax: f32,
}
impl AABB {
    pub fn from_zone_id(id: ZoneId) -> Self {
        // return (xmin,ymin,xmax,ymax)
        let mut xmin = WORLD_X_MIN;
        let mut ymin = WORLD_Y_MIN;
        let mut xmax = WORLD_X_MAX;
        let mut ymax = WORLD_Y_MAX;
        if id > 1 {
            let s = id
                .to_string()
                .chars()
                .map(|d| d.to_digit(10).unwrap())
                .collect::<Vec<_>>();
            for quadrant in &s[1..] {
                match quadrant {
                    1 => {
                        xmin = (xmin + xmax) / 2.0;
                        ymin = (ymin + ymax) / 2.0;
                    }
                    2 => {
                        xmax = (xmin + xmax) / 2.0;
                        ymin = (ymin + ymax) / 2.0;
                    }
                    3 => {
                        xmax = (xmin + xmax) / 2.0;
                        ymax = (ymin + ymax) / 2.0;
                    }
                    4 => {
                        xmin = (xmin + xmax) / 2.0;
                        ymax = (ymin + ymax) / 2.0;
                    }
                    _ => unreachable!(),
                }
            }
        }
        Self {
            xmin,
            ymin,
            xmax,
            ymax,
        }
    }

    // 获取AABB范围内所有grids
    pub fn get_grids_in_aabb(&self) -> Vec<GridId> {
        let grid_min = xy_to_grid(self.xmin, self.ymin);
        let grid_max = xy_to_grid(self.xmax, self.ymax);
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
        x >= self.xmin && x <= self.xmax && y >= self.ymin && y <= self.ymax
    }

    // 判断2个AABB是否有交集
    pub fn has_intersection(&self, other: &Self) -> bool {
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

    // 两个AABB的交集
    pub fn get_intersection(&self, other: &Self) -> Option<AABB> {
        if self.has_intersection(other) {
            Some(AABB {
                xmin: self.xmin.max(other.xmin),
                xmax: self.xmax.min(other.xmax),
                ymin: self.ymin.max(other.ymin),
                ymax: self.ymax.min(other.ymax),
            })
        } else {
            None
        }
    }
}
