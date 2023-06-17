use game_server::dispatcher::Dispatcher;
use game_server::util::Config;

use common::proto::game_service::{
    game_service_server::GameService, AoeRequest, PlayerInfo, QueryRequest,
};

use tokio::time::sleep;
use tonic::IntoRequest;

use std::time::Duration;

#[tokio::test]
async fn test_game_aoe() {
    crate::init_log();

    let dispatcher = Dispatcher::new(Config {
        max_players: 10, // 第10个触发expand
        min_players: 3,
        max_zone_depth: 10,
        scaling_interval: 200,
    })
    .await
    .unwrap();
    tokio::spawn(dispatcher.clone().scaling_moniter());

    for i in 0..9 {
        dispatcher
            .login(
                PlayerInfo {
                    player_id: i,
                    x: 1.0,
                    y: 2.0,
                    money: 99,
                }
                .into_request(),
            )
            .await
            .unwrap();
    }
    // 第10个触发expand
    dispatcher
        .login(
            PlayerInfo {
                player_id: 9,
                x: -1.0,
                y: 2.0,
                money: 99,
            }
            .into_request(),
        )
        .await
        .unwrap();

    sleep(Duration::from_millis(1000)).await;

    // aoe跨越2个服务器
    dispatcher
        .aoe(
            AoeRequest {
                player_id: 0,
                radius: 5.0,
                coord: None,
            }
            .into_request(),
        )
        .await
        .unwrap();

    let mut players = dispatcher
        .query(
            QueryRequest {
                xmin: -1.0,
                xmax: 1.0,
                ymin: 0.0,
                ymax: 2.0,
            }
            .into_request(),
        )
        .await
        .unwrap()
        .into_inner()
        .infos;
    players.sort_by_key(|p| p.player_id);

    // aoe 排除自己
    assert_eq!(players[0].money, 99);
    for i in 1..10 {
        assert_eq!(players[i].money, 100);
    }

    dispatcher.shutdown_all_map_server().await;
}
