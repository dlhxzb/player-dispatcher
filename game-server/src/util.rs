use common::{ServerId, ZoneId};

use anyhow::Result;
use common::{WORLD_X_MAX, WORLD_X_MIN, WORLD_Y_MAX, WORLD_Y_MIN};
use tonic::Status;

pub const ROOT_ZONE_ID: ZoneId = 1;

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

pub fn check_xy(x: f32, y: f32) -> Result<(), Status> {
    if x >= WORLD_X_MAX || y >= WORLD_Y_MAX || x <= WORLD_X_MIN || y < WORLD_Y_MIN {
        Err(Status::out_of_range(format!("x:{x} y:{y}")))
    } else {
        Ok(())
    }
}

// 判断一节点是否在另一节点的父路径上
pub fn is_in_parent_path(zone: ZoneId, parent: ZoneId) -> bool {
    let len1 = zone.ilog10();
    let len2 = parent.ilog10();
    len1 >= len2 && zone / 10_u64.pow(len1 - len2) == parent
}

#[inline]
pub fn zone_depth(id: ZoneId) -> u32 {
    id.ilog10() + 1
}

pub fn gen_server_id() -> ServerId {
    use std::sync::atomic::{AtomicU32, Ordering};

    static SERVER_ID: AtomicU32 = AtomicU32::new(0);
    SERVER_ID.fetch_add(1, Ordering::Relaxed)
}

pub async fn start_server() -> String {
    // TODO: 启动server
    "http://[::1]:50051".to_string()
}
