use color_eyre::eyre::{Result, bail, eyre};
use osc8::Hyperlink;
use std::path::Path;
use tokio::fs;
use tracing::info;

mod config;
mod logging;
mod updater;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    logging::init("info");
    info!("Starting factorio mod manager");

    let cfg = config::load_or_init(Path::new("mod-manager.toml")).await?;

    let docs_url = "https://wiki.factorio.com/Multiplayer#Dedicated/Headless_server";
    let link = Hyperlink::new(docs_url);

    // Factorio binary must exist
    ensure_path_exists("bin/x64/factorio", || {
        format!(
            "factorio not found — run me from the Factorio root directory. \
             If you do not have the headless server, see {}{}{}.",
            link, docs_url, link
        )
    })
    .await?;

    // Data directory must be present and be a directory
    ensure_path_is_dir("data", || {
        "data directory missing or not a directory — please run the Factorio server at least once".to_string()
    })
    .await?;

    // If old temp directory exists, delete it
    cleanup_temp_dir("temp").await;

    // Check for mod updates
    updater::check_update::check_mod_updates(Path::new("data"), &cfg).await?;

    info!("Terminating factorio mod manager");
    Ok(())
}

async fn ensure_path_exists<F>(path: &str, err_msg: F) -> Result<()>
where
    F: FnOnce() -> String,
{
    fs::metadata(path).await.map_err(|_| eyre!(err_msg()))?;
    Ok(())
}

async fn ensure_path_is_dir<F>(path: &str, err_msg: F) -> Result<()>
where
    F: Fn() -> String,
{
    let md = fs::metadata(path).await.map_err(|_| eyre!(err_msg()))?;
    if !md.is_dir() {
        bail!(err_msg());
    }
    Ok(())
}

async fn cleanup_temp_dir(path: &str) {
    if fs::metadata(path).await.is_ok() {
        if let Err(e) = fs::remove_dir_all(path).await {
            tracing::warn!("Failed to clean up temp directory `{}`: {:?}", path, e);
        }
    }
}
