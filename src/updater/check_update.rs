use crate::config::Config;
use crate::updater::mod_updater;
use color_eyre::eyre::Result;
use reqwest::Client;
use semver::Version;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, info, warn};

#[derive(Deserialize)]
struct LocalInfo {
    name: String,
    version: String,
}

#[derive(Deserialize)]
struct ApiResponse {
    releases: Vec<Release>,
}

#[derive(Deserialize)]
struct Release {
    version: String,
    download_url: String,
    file_name: String,
    sha1: String,
}

pub async fn check_mod_updates(data_dir: &Path, cfg: &Config) -> Result<()> {
    info!("Starting mod update check in {:?}", data_dir);

    let client = Client::new();
    let mut dir = fs::read_dir(data_dir).await?;

    while let Some(entry) = dir.next_entry().await? {
        let path = entry.path();
        if !should_process_mod(&path).await {
            continue;
        }
        if let Err(e) = process_mod_dir(path, &client, cfg).await {
            warn!("Failed to process mod at {:?}: {:?}", entry.path(), e);
        }
    }

    info!("Mod update check complete");
    Ok(())
}

/// Decide whether this entry is a mod directory we care about.
async fn should_process_mod(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    let is_dir = fs::metadata(path)
        .await
        .map(|m| m.is_dir())
        .unwrap_or(false);

    let skip = !is_dir || name == "base" || name == "core";
    if skip {
        debug!("Skipping {:?}", path);
    }
    !skip
}

/// Process one mod folder: read local info, fetch remote, and update if needed.
async fn process_mod_dir(path: PathBuf, client: &Client, cfg: &Config) -> Result<()> {
    let local = read_local_info(&path).await?;
    info!("Local mod '{}' version: {}", local.name, local.version);

    let releases = fetch_remote_releases(&local.name, client).await?;
    debug!("Found {} releases for '{}'", releases.len(), local.name);

    if let Some((latest_ver, rel)) = pick_latest(&releases) {
        compare_and_update(&local, latest_ver, rel, cfg).await?;
    } else {
        warn!("No valid releases found for '{}'", local.name);
    }
    Ok(())
}

/// Read and parse `info.json` in the mod folder.
async fn read_local_info(path: &Path) -> Result<LocalInfo> {
    let info_path = path.join("info.json");
    debug!("Reading local info from {:?}", info_path);
    let data = fs::read_to_string(&info_path).await?;
    let info: LocalInfo = serde_json::from_str(&data)?;
    Ok(info)
}

/// Fetch the full list of releases from the Factorio mods API.
async fn fetch_remote_releases(name: &str, client: &Client) -> Result<Vec<Release>> {
    let url = format!("https://mods.factorio.com/api/mods/{}/full", name);
    info!("Fetching remote metadata from {}", url);
    let resp: ApiResponse = client.get(&url).send().await?.json().await?;
    Ok(resp.releases)
}

/// Pick the highest valid semver release.
fn pick_latest(releases: &[Release]) -> Option<(Version, &Release)> {
    releases
        .iter()
        .filter_map(|r| Version::parse(&r.version).ok().map(|v| (v, r)))
        .max_by(|(v1, _), (v2, _)| v1.cmp(v2))
}

/// Compare local vs remote and run the updater if the remote is newer.
async fn compare_and_update(
    local: &LocalInfo,
    remote_ver: Version,
    rel: &Release,
    cfg: &Config,
) -> Result<()> {
    info!(
        "Latest remote version for '{}' is {}",
        local.name, remote_ver
    );
    let local_ver = Version::parse(&local.version)?;
    if remote_ver <= local_ver {
        debug!(
            "No update needed for '{}': local {} >= remote {}",
            local.name, local.version, remote_ver
        );
    } else {
        info!(
            "Updating '{}' from {} → {}",
            local.name, local.version, rel.version
        );
        mod_updater::update_mod(
            &rel.file_name,
            &rel.download_url,
            &rel.sha1,
            &cfg.factorio.username,
            &cfg.factorio.token,
        )
        .await?;
        info!("Successfully updated '{}'", local.name);
    }
    Ok(())
}
