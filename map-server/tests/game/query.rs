use map_server::server::MapServer;

use common::proto::game_service::game_service_server::GameService;
use common::proto::game_service::PlayerInfo;
use common::proto::game_service::*;

use tonic::IntoRequest;

#[tokio::test]
async fn test_query() {
    crate::init_log();
    let server = MapServer::new(1, "127.0.0.1:5001".to_string());
    // (0,0) (1,1) (2,2) (3,3)
    let players = (0..4)
        .map(|i| PlayerInfo {
            player_id: i,
            x: i as f32,
            y: i as f32,
            money: 0,
        })
        .collect::<Vec<_>>();
    for player in &players {
        server.login(player.clone().into_request()).await.unwrap();
    }
    let res = server
        .query(
            QueryRequest {
                xmin: 0.9,
                ymin: 1.0,
                xmax: 2.0,
                ymax: 2.0,
            }
            .into_request(),
        )
        .await
        .unwrap()
        .into_inner()
        .infos;
    assert_eq!(res, &players[1..=2]);
}
