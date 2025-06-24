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

    ensure_credentials(&cfg)?;

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

    // Data directory must be present
    ensure_path_is_dir("data", || {
        "data directory missing — please run the Factorio server at least once".to_string()
    })
    .await?;

    cleanup_temp_dir("temp").await;

    // Check for mod updates
    updater::check_update::check_mod_updates(Path::new("data"), &cfg).await?;

    info!("Shutting down");
    Ok(())
}

fn ensure_credentials(cfg: &config::Config) -> Result<()> {
    if cfg.factorio.username == "XXX" || cfg.factorio.token == "XXX" {
        bail!("Please edit mod-manager.toml to set your Factorio username and token.");
    }
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
