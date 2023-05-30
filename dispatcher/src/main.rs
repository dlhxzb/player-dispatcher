mod dispatcher;

#[tokio::main]
async fn main() {
    let mut dispatcher = dispatcher::Dispatcher::new().await;
}
