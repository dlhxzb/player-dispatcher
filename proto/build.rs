fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .out_dir("src/")
        .compile(&["game_service.proto", "map_service.proto"], &[""])?;
    Ok(())
}
