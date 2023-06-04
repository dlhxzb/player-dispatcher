use tonic::{Response, Status};

use crate::dispatcher::{ZoneId, MAX_ZONE_DEPTH, WORLD_X_MAX, WORLD_Y_MAX};
use crate::grpc::RPCResult;

pub const ROOT_ZONE_ID: u64 = 1;
// zone范围均为左闭右开
pub fn xy_to_zone(mut x: u64, mut y: u64) -> u64 {
    let mut id = ROOT_ZONE_ID;
    let mut length = WORLD_X_MAX;
    let mut height = WORLD_Y_MAX;
    for _ in 0..MAX_ZONE_DEPTH {
        length /= 2;
        height /= 2;
        //      2 4
        // 原点 1 3
        let pos = if x < length {
            if y < height {
                1
            } else {
                y -= height;
                2
            }
        } else {
            x -= length;
            if y < height {
                3
            } else {
                y -= height;
                4
            }
        };
        id = id * 10 + pos;
    }
    id
}

#[inline]
pub fn zone_depth(id: ZoneId) -> u32 {
    id.ilog10() + 1
}

pub fn check_xy(x: u64, y: u64) -> RPCResult<()> {
    if x >= WORLD_X_MAX || y >= WORLD_Y_MAX {
        Err(Status::out_of_range(format!("x:{x} y:{y}")))
    } else {
        Ok(Response::new(()))
    }
}

pub fn gen_server_id() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};

    static SERVER_ID: AtomicU64 = AtomicU64::new(0);
    SERVER_ID.fetch_add(1, Ordering::Relaxed)
}
pub async fn start_server() -> String {
    // TODO: 启动server
    "http://[::1]:50051".to_string()
}
