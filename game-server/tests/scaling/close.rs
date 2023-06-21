use game_server::dispatcher::Dispatcher;
use game_server::util::Config;

use common::proto::game_service::{game_service_server::GameService, PlayerIdRequest, PlayerInfo};

use tokio::time::{sleep, Duration};
use tonic::IntoRequest;

// 扩容测试
#[tokio::test]
async fn close_idle_server() {
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

    let (server, ..) = dispatcher.get_server_of_player(&0).unwrap();
    let count = server
        .map_cli
        .clone()
        .get_overhead(())
        .await
        .unwrap()
        .into_inner()
        .count;

    assert_eq!(count, 9);

    let (server, ..) = dispatcher.get_server_of_player(&9).unwrap();
    let count = server
        .map_cli
        .clone()
        .get_overhead(())
        .await
        .unwrap()
        .into_inner()
        .count;

    assert_eq!(count, 1);

    // logout 1个触发缩容
    dispatcher
        .logout(PlayerIdRequest { player_id: 1 }.into_request())
        .await
        .unwrap();
    sleep(Duration::from_millis(1000)).await;
    let (server1, ..) = dispatcher.get_server_of_player(&0).unwrap();
    let (server2, ..) = dispatcher.get_server_of_player(&9).unwrap();
    assert_eq!(server1.server_id, server2.server_id);

    dispatcher.shutdown_all_map_server().await;
}
