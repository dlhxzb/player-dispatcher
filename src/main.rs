use std::collections::HashMap;

struct Player {
    id: u32,
    x: usize,
    y: usize,
}

struct Server;

// A zone = 1000x1000, 100x100 zones
struct Dispatcher {
    servers: Vec<Server>,
    server_zone_matrix: [[u32; 100]; 100],
    player_server_map: HashMap<u32, u32>,
}

impl Dispatcher {
    fn new() -> Self {
        Self {
            servers: vec![Server],
            server_zone_matrix: [[0; 100]; 100],
            player_server_map: HashMap::<u32, u32>::new(),
        }
    }

    fn login(player: Player) {
        todo!();
    }
}

#[inline]
fn pos_to_zone(x: usize, y: usize) -> (usize, usize) {
    (x / 100, y / 100)
}

fn main() {
    let mut dispatcher = Dispatcher::new();
}
