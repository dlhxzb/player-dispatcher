use map_server::server::MapServer;

use common::proto::game_service::game_service_server::GameService;
use common::proto::game_service::PlayerInfo;
use common::proto::game_service::*;
use tonic::IntoRequest;

#[tokio::test]
async fn test_moving() {
    crate::init_log();
    let server = MapServer::new(1, "127.0.0.1:5001".to_string());
    let player = PlayerInfo {
        player_id: 1,
        x: 0.0,
        y: 0.0,
        money: 0,
    };
    server.login(player.clone().into_request()).await.unwrap();
    server
        .moving(
            MovingRequest {
                player_id: 1,
                dx: 1.0,
                dy: -1.9,
            }
            .into_request(),
        )
        .await
        .unwrap();
    let player = server.player_map.get(&1).unwrap().value().clone();
    let expect = PlayerInfo {
        player_id: 1,
        x: 1.0,
        y: -1.9,
        money: 0,
    };
    assert_eq!(player, expect);
}
