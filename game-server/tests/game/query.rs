use game_server::dispatcher::Dispatcher;
use game_server::util::Config;

use common::{
    proto::game_service::{
        game_service_server::GameService, MovingRequest, PlayerInfo, QueryRequest,
    },
    WORLD_X_MAX, WORLD_X_MIN, WORLD_Y_MAX, WORLD_Y_MIN,
};

use tokio::time::{sleep, Duration};
use tonic::IntoRequest;

// 服务器扩容，moving引起用户在服务器转移，这些情况下query
#[tokio::test]
async fn test_query() {
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
                    x: 100.0,
                    y: 200.0,
                    money: 99,
                }
                .into_request(),
            )
            .await
            .unwrap();
    }

    let count = dispatcher
        .query(
            QueryRequest {
                xmin: 0.0,
                xmax: 100.0,
                ymin: 0.0,
                ymax: 200.0,
            }
            .into_request(),
        )
        .await
        .unwrap()
        .into_inner()
        .infos
        .len();
    assert_eq!(count, 9);

    // 第10个触发expand
    dispatcher
        .login(
            PlayerInfo {
                player_id: 9,
                x: -100.0,
                y: 200.0,
                money: 99,
            }
            .into_request(),
        )
        .await
        .unwrap();

    sleep(Duration::from_millis(1000)).await;

    let count = dispatcher
        .query(
            QueryRequest {
                xmin: 0.0,
                xmax: 100.0,
                ymin: 0.0,
                ymax: 200.0,
            }
            .into_request(),
        )
        .await
        .unwrap()
        .into_inner()
        .infos
        .len();
    assert_eq!(count, 9);
    let count = dispatcher
        .query(
            QueryRequest {
                xmin: -100.0,
                xmax: 100.0,
                ymin: 0.0,
                ymax: 200.0,
            }
            .into_request(),
        )
        .await
        .unwrap()
        .into_inner()
        .infos
        .len();
    assert_eq!(count, 10);

    // 从第一象限移动到第二象限，原本1,9人服务器，变成2,8
    dispatcher
        .moving(
            MovingRequest {
                player_id: 1,
                dx: -150.0,
                dy: 1.0,
            }
            .into_request(),
        )
        .await
        .unwrap();

    let players = dispatcher
        .query(
            QueryRequest {
                xmin: 0.0,
                xmax: 100.0,
                ymin: 0.0,
                ymax: 200.0,
            }
            .into_request(),
        )
        .await
        .unwrap()
        .into_inner()
        .infos;
    assert_eq!(8, players.len());

    let players = dispatcher
        .query(
            QueryRequest {
                xmin: -100.0,
                xmax: 0.0,
                ymin: 0.0,
                ymax: 200.0,
            }
            .into_request(),
        )
        .await
        .unwrap()
        .into_inner()
        .infos;
    assert_eq!(players.len(), 1);

    let players = dispatcher
        .query(
            QueryRequest {
                xmin: -100.0,
                xmax: 0.0,
                ymin: 0.0,
                ymax: 201.0,
            }
            .into_request(),
        )
        .await
        .unwrap()
        .into_inner()
        .infos;
    assert_eq!(players.len(), 2);

    let players = dispatcher
        .query(
            QueryRequest {
                xmin: -100.0,
                xmax: 100.0,
                ymin: 0.0,
                ymax: 201.0,
            }
            .into_request(),
        )
        .await
        .unwrap()
        .into_inner()
        .infos;
    assert_eq!(players.len(), 10);

    dispatcher.shutdown_all_map_server().await;
}

#[tokio::test]
async fn test_full_map_query() {
    crate::init_log();

    let dispatcher = Dispatcher::new(Config {
        max_players: 100, // 第10个触发expand
        min_players: 25,
        max_zone_depth: 10,
        scaling_interval: 1000,
    })
    .await
    .unwrap();
    tokio::spawn(dispatcher.clone().scaling_moniter());

    for i in 0..1000 {
        dispatcher
            .login(
                PlayerInfo {
                    player_id: i,
                    x: i as f32 * 10.0,
                    y: i as f32 * 10.0,
                    money: 99,
                }
                .into_request(),
            )
            .await
            .unwrap();
    }
    sleep(Duration::from_millis(5000)).await;

    let infos = dispatcher
        .query(
            QueryRequest {
                xmin: WORLD_X_MIN,
                xmax: WORLD_X_MAX,
                ymin: WORLD_Y_MIN,
                ymax: WORLD_Y_MAX,
            }
            .into_request(),
        )
        .await
        .unwrap()
        .into_inner()
        .infos;
    assert_eq!(infos.len(), 1000);

    dispatcher.shutdown_all_map_server().await;
}
