use color_eyre::eyre::{Result, eyre};
use reqwest::Url;
use sha1::{Digest, Sha1};
use std::io::Cursor;
use std::path::Path;
use tokio::fs;
use tracing::{debug, error, info};
use walkdir::WalkDir;
use zip::ZipArchive;

pub async fn update_mod(
    name: &str,
    download_url: &str,
    expected_sha: &str,
    username: &str,
    token: &str,
) -> Result<()> {
    info!("Beginning update for mod '{}'", name);

    // Ensure the data/ directory exists
    let data_dir = Path::new("data");
    info!("Ensuring data directory exists at {:?}", data_dir);
    fs::create_dir_all(data_dir).await?;

    // Build URL with authentication
    let base = if download_url.starts_with("http") {
        download_url.to_string()
    } else {
        format!("https://mods.factorio.com/{}", download_url)
    };
    debug!("Base download URL: {}", base);

    let mut url = Url::parse(&base)
        .map_err(|e| eyre!("Invalid download URL for {}: {}: {}", name, download_url, e))?;
    {
        let mut qp = url.query_pairs_mut();
        qp.append_pair("username", username);
        qp.append_pair("token", token);
    }
    info!("Downloading '{}' from {}", name, url);

    // Download ZIP into memory
    let resp = reqwest::get(url.clone()).await?;
    if !resp.status().is_success() {
        error!(
            "Failed to download '{}': HTTP {} at {}",
            name,
            resp.status(),
            resp.url()
        );
        return Err(eyre!(
            "Failed to download {}: HTTP {} at {}",
            name,
            resp.status(),
            resp.url()
        ));
    }
    let bytes = resp.bytes().await?;
    debug!("Downloaded {} bytes for '{}'", bytes.len(), name);

    // SHA-1 check
    let mut hasher = Sha1::new();
    hasher.update(&bytes);
    let got_sha = format!("{:x}", hasher.finalize());
    debug!("Computed SHA1 for '{}': {}", name, got_sha);
    if got_sha != expected_sha {
        error!(
            "SHA1 mismatch for '{}': expected {}, got {}",
            name, expected_sha, got_sha
        );
        return Err(eyre!(
            "SHA1 mismatch for {}: expected {}, got {}",
            name,
            expected_sha,
            got_sha
        ));
    }
    info!("SHA1 verified for '{}'", name);

    // In-memory unzip into temp/
    let temp_dir = Path::new("temp");
    if temp_dir.exists() {
        info!("Cleaning up existing temp directory at {:?}", temp_dir);
        fs::remove_dir_all(temp_dir).await?;
    }
    fs::create_dir_all(temp_dir).await?;
    info!("Extracting archive into {:?}", temp_dir);

    let reader = Cursor::new(bytes);
    let mut archive =
        ZipArchive::new(reader).map_err(|e| eyre!("Failed to read ZIP for {}: {}", name, e))?;
    archive
        .extract(temp_dir)
        .map_err(|e| eyre!("Failed to extract {}: {}", name, e))?;
    info!("Extraction complete for '{}'", name);

    // Find the folder that contains info.json
    let extracted_root = WalkDir::new(temp_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .find(|e| e.file_name() == "info.json")
        .and_then(|e| e.path().parent().map(std::path::PathBuf::from))
        .ok_or_else(|| {
            error!("Could not find info.json in ZIP for '{}'", name);
            eyre!("Could not find info.json in ZIP for {}", name)
        })?;
    debug!("Located extracted root directory: {:?}", extracted_root);

    // Derive the neat folder name
    let raw = extracted_root
        .file_name()
        .and_then(|os| os.to_str())
        .ok_or_else(|| {
            error!("Bad folder name in ZIP for '{}'", name);
            eyre!("Bad folder name in ZIP for {}", name)
        })?;
    let neat = raw.trim_end_matches(".zip");
    info!("Preparing install folder name: '{}'", neat);

    // Remove any old versions for this mod
    let slug = neat.split_once('_').map_or(neat, |(first, _)| first);
    info!("Cleaning up old versions for slug '{}'", slug);
    let mut entries = fs::read_dir(data_dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let ty = entry.file_type().await?;
        if ty.is_dir() {
            let fname = entry.file_name().to_string_lossy().to_string();
            if fname.starts_with(&format!("{}_", slug)) && fname != neat {
                let old_path = entry.path();
                info!("Removing old version directory {:?}", old_path);
                fs::remove_dir_all(old_path).await?;
            }
        }
    }

    // Move the extracted folder into data/{neat}
    let dest = data_dir.join(neat);
    info!("Moving '{}' to {:?}", name, dest);
    fs::rename(&extracted_root, &dest).await?;

    // Cleanup temp files
    info!("Removing temp directory at {:?}", temp_dir);
    fs::remove_dir_all(temp_dir).await?;

    info!("Mod '{}' updated successfully → {}", name, neat);
    Ok(())
}
