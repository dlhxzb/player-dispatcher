mod game;

pub fn init_log() {
    use once_cell::sync::OnceCell;

    static CELL: OnceCell<()> = OnceCell::new();
    CELL.get_or_init(|| tracing_subscriber::fmt::init());
}
