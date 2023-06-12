use common::proto::game_service::{game_service_server::GameService, PlayerInfo};
use game_server::dispatcher::Dispatcher;
use tonic::IntoRequest;

pub fn init_log() {
    use once_cell::sync::OnceCell;

    static CELL: OnceCell<()> = OnceCell::new();
    CELL.get_or_init(|| tracing_subscriber::fmt::init());
}

#[tokio::test]
async fn dispatcher_works() {
    let dispatcher = Dispatcher::new().await.unwrap();
    dispatcher
        .login(
            PlayerInfo {
                player_id: 1,
                x: 100.0,
                y: 200.0,
                money: 99,
            }
            .into_request(),
        )
        .await
        .unwrap();
}
