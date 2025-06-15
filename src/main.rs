// src/main.rs
use color_eyre::eyre::Result;
mod check_update;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    // ensure we’re in the Factorio root
    let bin = std::path::Path::new("bin/x64/factorio");
    if !bin.exists() {
        color_eyre::eyre::bail!(
            "bin/x64/factorio not found — run me from the Factorio root directory"
        );
    }

    // ensure the data directory exists
    let data_dir = std::path::Path::new("data");
    if !data_dir.is_dir() {
        color_eyre::eyre::bail!(
            "data directory missing — please run the Factorio server at least once"
        );
    }

    // dispatch the async updater
    check_update::check_mod_updates(data_dir).await
}
