use color_eyre::eyre::{Result, eyre};
use reqwest::Url;
use sha1::{Digest, Sha1};
use std::io::Cursor;
use std::path::Path;
use tokio::fs;
use walkdir::WalkDir;
use zip::ZipArchive;

pub async fn update_mod(
    name: &str,
    download_url: &str,
    expected_sha: &str,
    username: &str,
    token: &str,
) -> Result<()> {
    // Ensure the data/ directory exists
    let data_dir = Path::new("data");
    fs::create_dir_all(data_dir).await?;

    // URL + authentication
    let base = if download_url.starts_with("http") {
        download_url.to_string()
    } else {
        format!("https://mods.factorio.com/{}", download_url)
    };
    let mut url = Url::parse(&base)
        .map_err(|e| eyre!("Invalid download URL for {}: {}: {}", name, download_url, e))?;
    {
        let mut qp = url.query_pairs_mut();
        qp.append_pair("username", username);
        qp.append_pair("token", token);
    }
    eprintln!("→ downloading {} from {}", name, url);

    // Download ZIP into memory
    let resp = reqwest::get(url).await?;
    if !resp.status().is_success() {
        return Err(eyre!(
            "Failed to download {}: HTTP {} at {}",
            name,
            resp.status(),
            resp.url()
        ));
    }
    let bytes = resp.bytes().await?;

    // SHA-1 check
    let mut hasher = Sha1::new();
    hasher.update(&bytes);
    let got_sha = format!("{:x}", hasher.finalize());
    if got_sha != expected_sha {
        return Err(eyre!(
            "SHA1 mismatch for {}: expected {}, got {}",
            name,
            expected_sha,
            got_sha
        ));
    }

    // In-memory unzip into temp/
    let temp_dir = Path::new("temp");
    if temp_dir.exists() {
        fs::remove_dir_all(temp_dir).await?;
    }
    fs::create_dir_all(temp_dir).await?;

    let reader = Cursor::new(bytes);
    let mut archive =
        ZipArchive::new(reader).map_err(|e| eyre!("Failed to read ZIP for {}: {}", name, e))?;
    archive
        .extract(temp_dir)
        .map_err(|e| eyre!("Failed to extract {}: {}", name, e))?;

    // Find the folder that contains info.json
    let extracted_root = WalkDir::new(temp_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .find(|e| e.file_name() == "info.json")
        .and_then(|e| e.path().parent().map(std::path::PathBuf::from))
        .ok_or_else(|| eyre!("Could not find info.json in ZIP for {}", name))?;

    // Strip any trailing “.zip” from the folder’s name
    let raw = extracted_root
        .file_name()
        .and_then(|os| os.to_str())
        .ok_or_else(|| eyre!("Bad folder name in ZIP for {}", name))?;
    let neat = raw.trim_end_matches(".zip");

    // Remove any old version directories for this mod (so only the newest remains)
    let slug = neat.split_once('_').map_or(neat, |(first, _)| first);
    let mut entries = fs::read_dir(data_dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let ty = entry.file_type().await?;
        if ty.is_dir() {
            let fname = entry.file_name().to_string_lossy().to_string();
            // remove any folder that starts with "slug_" but isn't the one we're about to install
            if fname.starts_with(&format!("{}_", slug)) && fname != neat {
                fs::remove_dir_all(entry.path()).await?;
            }
        }
    }

    // Move the extracted folder into data/{neat}
    let dest = data_dir.join(neat);
    fs::rename(&extracted_root, &dest).await?;

    // Cleanup temp files
    fs::remove_dir_all(temp_dir).await?;

    println!("{} updated successfully → {}", name, neat);
    Ok(())
}
