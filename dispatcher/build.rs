fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::compile_protos(r"../proto/game_interface.proto")?;
    tonic_build::compile_protos(r"../proto/map_service.proto")?;
    Ok(())
}
