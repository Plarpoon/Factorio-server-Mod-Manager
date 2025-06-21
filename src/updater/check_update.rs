use color_eyre::eyre::Result;
use reqwest::Client;
use semver::Version;
use serde::Deserialize;
use std::path::Path;
use tokio::fs::{self, ReadDir};

use crate::config::Config;
use crate::updater::mod_updater;

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
    let client = Client::new();

    let mut dir: ReadDir = fs::read_dir(data_dir).await?;
    while let Some(entry) = dir.next_entry().await? {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        // skip non-mod dirs
        if !path.is_dir() || name == "base" || name == "core" {
            continue;
        }

        // Read and parse local info.json
        let info_path = path.join("info.json");
        let data = fs::read_to_string(&info_path).await?;
        let local: LocalInfo = serde_json::from_str(&data)?;

        // Fetch remote metadata
        let url = format!("https://mods.factorio.com/api/mods/{}/full", local.name);
        let resp: ApiResponse = client.get(&url).send().await?.json().await?;

        // Pick the highest semver release
        if let Some((remote_ver, rel)) = resp
            .releases
            .iter()
            .filter_map(|r| Version::parse(&r.version).ok().map(|v| (v, r)))
            .max_by(|(v1, _), (v2, _)| v1.cmp(v2))
        {
            let local_ver = Version::parse(&local.version)?;
            if remote_ver > local_ver {
                println!(
                    "Updating {}: {} → {}",
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
            }
        }
    }

    Ok(())
}
