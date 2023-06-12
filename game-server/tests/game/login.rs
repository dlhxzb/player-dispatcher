use game_server::dispatcher::Dispatcher;
use game_server::util::Config;

use common::proto::game_service::{game_service_server::GameService, PlayerInfo};

use tonic::IntoRequest;

#[tokio::test]
async fn test_game_login() {
    // crate::init_log();

    let dispatcher = Dispatcher::new(Config {
        max_players: 10,
        min_players: 3,
        max_zone_depth: 10,
        scaling_interval: 0,
    })
    .await
    .unwrap();

    dispatcher
        .login(
            PlayerInfo {
                player_id: 1,
                x: -100.0,
                y: 200.0,
                money: 99,
            }
            .into_request(),
        )
        .await
        .unwrap();

    let (server, ..) = dispatcher.get_server_of_player(&1).unwrap();
    let count = server
        .map_cli
        .clone()
        .get_overhead(())
        .await
        .unwrap()
        .into_inner()
        .count;

    assert_eq!(count, 1);

    dispatcher.shutdown_all_map_server().await;
}
