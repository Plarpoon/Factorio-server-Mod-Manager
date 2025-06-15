use color_eyre::eyre::{Result, WrapErr, eyre};
use futures::stream::{FuturesUnordered, StreamExt};
use reqwest::Client;
use semver::Version;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use tokio::fs;

#[derive(Deserialize)]
struct ModInfo {
    name: String,
    version: String,
}

#[derive(Deserialize)]
struct RemoteMod {
    releases: Vec<Release>,
}

#[derive(Deserialize)]
struct Release {
    version: String,
}

/// Main entry point to check for mod updates.
pub async fn check_mod_updates(data_dir: impl AsRef<Path>) -> Result<()> {
    let client = Client::new();
    let mod_dirs = find_mod_dirs(data_dir.as_ref()).await?;
    let mut tasks = FuturesUnordered::new();

    for dir in mod_dirs {
        tasks.push(process_mod(dir, client.clone()));
    }

    // Collect and return early on first error
    while let Some(res) = tasks.next().await {
        res?;
    }
    Ok(())
}

/// List subdirectories that contain an `info.json`, skipping built-ins.
async fn find_mod_dirs(data_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut dirs = Vec::new();
    let mut rd = fs::read_dir(data_dir)
        .await
        .wrap_err_with(|| format!("could not read {:?}", data_dir))?;

    while let Some(entry) = rd.next_entry().await? {
        let path = entry.path();
        if !fs::metadata(&path).await?.is_dir() {
            continue;
        }

        // skip both built-in mods
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name == "base" || name == "core" {
                continue;
            }
        }

        if fs::metadata(path.join("info.json")).await.is_ok() {
            dirs.push(path);
        }
    }
    Ok(dirs)
}

/// Handle one mod: load local info, fetch remote, compare.
async fn process_mod(path: PathBuf, client: Client) -> Result<()> {
    let mod_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("<unknown>");

    let info_path = path.join("info.json");
    let text = fs::read_to_string(&info_path)
        .await
        .wrap_err_with(|| format!("reading info.json for mod `{}`", mod_name))?;
    let info: ModInfo = serde_json::from_str(&text)
        .wrap_err_with(|| format!("parsing info.json for mod `{}`", mod_name))?;

    let local_ver = Version::parse(&info.version).wrap_err_with(|| {
        format!(
            "invalid version `{}` in info.json for mod `{}`",
            info.version, mod_name
        )
    })?;

    let latest = fetch_latest_version(&client, &info.name)
        .await?
        .ok_or_else(|| eyre!("no releases found for `{}`", mod_name))?;

    if latest > local_ver {
        println!("Mod `{}` update: {} → {}", mod_name, local_ver, latest);
    }

    Ok(())
}

/// Call the Factorio mods API and return the highest‐version release.
async fn fetch_latest_version(client: &Client, name: &str) -> Result<Option<Version>> {
    let url = format!("https://mods.factorio.com/api/mods/{}", name);
    let resp = client
        .get(&url)
        .send()
        .await
        .wrap_err_with(|| format!("requesting `{}`", url))?;

    let remote: RemoteMod = resp
        .json()
        .await
        .wrap_err("failed to parse JSON from mods API")?;

    Ok(remote
        .releases
        .into_iter()
        .filter_map(|r| Version::parse(&r.version).ok())
        .max())
}
