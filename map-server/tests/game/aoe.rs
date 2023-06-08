use map_server::server::Server;

use common::proto::game_service::PlayerInfo;
use common::proto::game_service::*;
use common::proto::map_service::map_service_server::MapService;
use common::proto::map_service::InternalAoeRequest;
use common::AOE_MONEY;

use tonic::IntoRequest;

#[tokio::test]
async fn test_query() {
    crate::init_log();
    let server = Server::new(1);
    // (-1,-1) (0,0) (1,1) (2,2)
    let mut players = (0..4)
        .map(|i| PlayerInfo {
            id: i,
            x: i as f32 - 1.0,
            y: i as f32 - 1.0,
            money: 0,
        })
        .collect::<Vec<_>>();
    for player in &players {
        server
            .internal_login(player.clone().into_request())
            .await
            .unwrap();
    }
    server
        .internal_aoe(
            InternalAoeRequest {
                player_id: 1,
                x: players[1].x,
                y: players[1].y,
                radius: 1.9,
            }
            .into_request(),
        )
        .await
        .unwrap();
    let res = server
        .player_map
        .iter()
        .map(|entry| entry.value().clone())
        .collect::<Vec<_>>();
    players[0].money += AOE_MONEY;
    players[2].money += AOE_MONEY;
    assert_eq!(res, players);
}
