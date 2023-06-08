use map_server::server::Server;

use common::proto::game_service::PlayerInfo;
use common::proto::game_service::*;
use common::proto::map_service::map_service_server::MapService;

use tonic::IntoRequest;

#[tokio::test]
async fn test_query() {
    crate::init_log();
    let server = Server::new(1);
    // (0,0) (1,1) (2,2) (3,3)
    let players = (0..4)
        .map(|i| PlayerInfo {
            id: i,
            x: i as f32,
            y: i as f32,
            money: 0,
        })
        .collect::<Vec<_>>();
    for player in &players {
        server
            .internal_login(player.clone().into_request())
            .await
            .unwrap();
    }
    let res = server
        .internal_query(
            QueryRequest {
                id: 1,
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
