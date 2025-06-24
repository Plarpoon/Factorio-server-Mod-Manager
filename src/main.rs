use color_eyre::eyre::Result;
use osc8::Hyperlink;
use std::path::Path;
use tracing::info;

mod config;
mod logging;
mod updater;

#[tokio::main]
async fn main() -> Result<()> {
    logging::init("info");
    info!("Starting factorio mod manager");

    color_eyre::install()?;

    // Load or initialize the config file mod-manager.toml
    let cfg = config::load_or_init(Path::new("mod-manager.toml")).await?;

    if cfg.factorio.username == "XXX" || cfg.factorio.token == "XXX" {
        eprintln!("Please edit mod-manager.toml to set your Factorio username and token.");
        std::process::exit(1);
    }

    // Link to Factorio headless server docs
    let url = "https://wiki.factorio.com/Multiplayer#Dedicated/Headless_server";
    let link = Hyperlink::new(url);

    // Ensure we’re in the Factorio root directory
    let bin = Path::new("bin/x64/factorio");
    if !bin.exists() {
        eprintln!(
            "factorio not found — run me from the Factorio root directory. \
             If you do not have Factorio headless server downloaded learn how to do so \
             and configure it properly here {}{}{}.",
            link, url, link
        );
        std::process::exit(1);
    }

    // Ensure the data directory exists
    let data_dir = Path::new("data");
    if !data_dir.is_dir() {
        eprintln!("data directory missing — please run the Factorio server at least once");
        std::process::exit(1);
    }

    // Delete leftover temp directory if it exists
    let temp_dir = Path::new("temp");
    if temp_dir.exists() {
        if let Err(e) = tokio::fs::remove_dir_all(temp_dir).await {
            eprintln!("Failed to clean up temp directory: {}", e);
        }
    }

    // Dispatch the async updater
    updater::check_update::check_mod_updates(data_dir, &cfg).await?;

    info!("Shutting down");
    Ok(())
}
