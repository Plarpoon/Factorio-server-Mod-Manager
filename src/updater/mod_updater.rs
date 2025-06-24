use color_eyre::eyre::{Result, eyre};
use reqwest::{Client, Url};
use sha1::{Digest, Sha1};
use std::{
    io::Cursor,
    path::{Path, PathBuf},
};
use tokio::{fs, task};
use tracing::{debug, error, info};
use walkdir::WalkDir;
use zip::ZipArchive;

/// Downloads, verifies, and installs a Factorio mod.
pub async fn update_mod(
    name: &str,
    download_url: &str,
    expected_sha: &str,
    username: &str,
    token: &str,
) -> Result<()> {
    info!("Starting update for mod '{}'", name);

    ensure_data_dir().await?;
    let url = build_download_url(download_url, username, token)?;

    info!("Downloading '{}' from {}", name, url);
    let bytes = download_zip(&url).await?;

    verify_sha(&bytes[..], expected_sha, name)?;

    let temp_dir = prepare_temp_dir().await?;
    extract_zip(bytes, &temp_dir)?;

    let extracted_root = find_extracted_root(&temp_dir, name)?;
    let neat = derive_neat_name(&extracted_root, name)?;

    clean_old_versions(&neat).await?;
    install_new_mod(&extracted_root, &neat).await?;
    cleanup_temp(&temp_dir).await?;

    info!("Mod '{}' installed as '{}'", name, neat);
    Ok(())
}

async fn ensure_data_dir() -> Result<()> {
    let data_dir = Path::new("data");
    fs::create_dir_all(data_dir).await?;
    Ok(())
}

fn build_download_url(raw_url: &str, user: &str, token: &str) -> Result<Url> {
    let base = if raw_url.starts_with("http") {
        raw_url.to_string()
    } else {
        format!("https://mods.factorio.com/{}", raw_url)
    };
    let mut url =
        Url::parse(&base).map_err(|e| eyre!("Invalid download URL '{}': {}", raw_url, e))?;
    {
        let mut qp = url.query_pairs_mut();
        qp.append_pair("username", user);
        qp.append_pair("token", token);
    }
    Ok(url)
}

async fn download_zip(url: &Url) -> Result<bytes::Bytes> {
    let resp = Client::new().get(url.clone()).send().await?;
    if !resp.status().is_success() {
        error!("HTTP {} at {}", resp.status(), resp.url());
        return Err(eyre!("Download failed: HTTP {}", resp.status()));
    }
    let bytes = resp.bytes().await?;
    debug!("Downloaded {} bytes", bytes.len());
    Ok(bytes)
}

fn verify_sha(bytes: &[u8], expected: &str, name: &str) -> Result<()> {
    let mut hasher = Sha1::new();
    hasher.update(bytes);
    let got = format!("{:x}", hasher.finalize());
    debug!("SHA1 for '{}': {}", name, got);
    if got != expected {
        error!(
            "SHA mismatch for '{}': expected {}, got {}",
            name, expected, got
        );
        return Err(eyre!("SHA mismatch for {}", name));
    }
    Ok(())
}

async fn prepare_temp_dir() -> Result<PathBuf> {
    let temp = PathBuf::from("temp");
    if fs::metadata(&temp).await.is_ok() {
        fs::remove_dir_all(&temp).await?;
    }
    fs::create_dir_all(&temp).await?;
    Ok(temp)
}

fn extract_zip(bytes: bytes::Bytes, temp: &Path) -> Result<()> {
    // Extraction can block, so run in a blocking task.
    task::block_in_place(|| {
        let reader = Cursor::new(bytes);
        let mut archive =
            ZipArchive::new(reader).map_err(|e| eyre!("Failed to read zip: {}", e))?;
        archive
            .extract(temp)
            .map_err(|e| eyre!("Failed to extract zip: {}", e))
    })?;
    info!("Extracted archive to '{:?}'", temp);
    Ok(())
}

fn find_extracted_root(temp: &Path, name: &str) -> Result<PathBuf> {
    WalkDir::new(temp)
        .into_iter()
        .filter_map(|e| e.ok())
        .find(|e| e.file_name() == "info.json")
        .and_then(|e| e.path().parent().map(PathBuf::from))
        .ok_or_else(|| eyre!("Missing info.json in archive for {}", name))
}

fn derive_neat_name(extracted: &Path, name: &str) -> Result<String> {
    let raw = extracted
        .file_name()
        .and_then(|os| os.to_str())
        .ok_or_else(|| eyre!("Bad folder name for {}", name))?;
    let neat = raw.trim_end_matches(".zip");
    Ok(neat.to_string())
}

async fn clean_old_versions(neat: &str) -> Result<()> {
    let slug = neat.split_once('_').map_or(neat, |(s, _)| s);
    let mut entries = fs::read_dir("data").await?;
    while let Some(ent) = entries.next_entry().await? {
        let ty = ent.file_type().await?;
        if ty.is_dir() {
            let fname = ent.file_name().to_string_lossy().to_string();
            if fname.starts_with(slug) && fname != neat {
                let old = ent.path();
                info!("Removing old version '{:?}'", old);
                fs::remove_dir_all(old).await?;
            }
        }
    }
    Ok(())
}

async fn install_new_mod(src: &Path, neat: &str) -> Result<()> {
    let dest = Path::new("data").join(neat);
    fs::rename(src, &dest).await?;
    Ok(())
}

async fn cleanup_temp(temp: &Path) -> Result<()> {
    fs::remove_dir_all(temp).await?;
    Ok(())
}
