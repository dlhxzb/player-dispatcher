use map_server::server::MapServer;

use common::proto::game_service::game_service_server::GameService;
use common::proto::game_service::PlayerInfo;
use common::proto::game_service::*;
use common::AOE_MONEY;

use tonic::IntoRequest;

#[tokio::test]
async fn test_query() {
    crate::init_log();
    let server = MapServer::new(1);
    // (-1,-1) (0,0) (1,1) (2,2)
    let mut players = (0..4)
        .map(|i| PlayerInfo {
            player_id: i,
            x: i as f32 - 1.0,
            y: i as f32 - 1.0,
            money: 0,
        })
        .collect::<Vec<_>>();
    for player in &players {
        server.login(player.clone().into_request()).await.unwrap();
    }
    server
        .aoe(
            AoeRequest {
                player_id: 1,
                coord: Some(Coord {
                    x: players[1].x,
                    y: players[1].y,
                }),
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
