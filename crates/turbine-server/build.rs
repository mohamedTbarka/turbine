fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Proto compilation is optional - the generated code is included in the repo
    // To regenerate, install protoc and run: cargo build --features proto-gen

    #[cfg(feature = "proto-gen")]
    {
        // Create output directory if it doesn't exist
        std::fs::create_dir_all("src/generated")?;

        tonic_build::configure()
            .build_server(true)
            .build_client(false)
            .out_dir("src/generated")
            .compile(&["../../proto/turbine.proto"], &["../../proto"])?;
    }

    Ok(())
}
