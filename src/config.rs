use color_eyre::eyre::{Result, eyre};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs;
use toml::{Value, map::Map};
use tracing::{debug, info, warn};

const ALLOWED_KEYS: &[&str] = &["username", "token"];

#[derive(Serialize, Deserialize, Debug)]
pub struct FactorioConfig {
    pub username: String,
    pub token: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub factorio: FactorioConfig,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            factorio: FactorioConfig {
                username: "XXX".into(),
                token: "XXX".into(),
            },
        }
    }
}

/// Load or initialize `mod-manager.toml` at `path`.
/// If missing, writes default. If present, sanitizes and returns.
pub async fn load_or_init(path: &Path) -> Result<Config> {
    info!("Loading config from {:?}", path);
    if !file_exists(path).await? {
        return create_default(path).await;
    }
    sanitize_existing(path).await
}

async fn file_exists(path: &Path) -> Result<bool> {
    Ok(fs::metadata(path).await.is_ok())
}

async fn create_default(path: &Path) -> Result<Config> {
    info!("Config not found, writing default to {:?}", path);
    let cfg = Config::default();
    let toml_str = toml::to_string_pretty(&cfg)?;
    fs::write(path, toml_str).await?;
    Ok(cfg)
}

async fn sanitize_existing(path: &Path) -> Result<Config> {
    info!("Found config at {:?}, reading…", path);
    let contents = fs::read_to_string(path).await?;
    let mut doc: Value =
        toml::from_str(&contents).map_err(|e| eyre!("Invalid TOML in {:?}: {}", path, e))?;

    let table = doc
        .as_table_mut()
        .ok_or_else(|| eyre!("Expected root table in {:?}, overwriting", path))?;

    sanitize_factorio_section(table);

    let new_toml = toml::to_string_pretty(&doc)?;
    fs::write(path, new_toml.clone()).await?;
    info!("Sanitized config written back to {:?}", path);

    let cfg: Config = toml::from_str(&new_toml)?;
    info!("Loaded configuration: {:?}", cfg);
    Ok(cfg)
}

fn sanitize_factorio_section(root: &mut Map<String, Value>) {
    // Prepare a default FactorioConfig as TOML Value
    let default_section: Value = toml::from_str(
        &toml::to_string(&Config::default().factorio).expect("default factorio always serializes"),
    )
    .expect("default factorio always deserializes");

    // Ensure we have a table under "factorio"
    let entry = root
        .entry("factorio")
        .or_insert_with(|| default_section.clone());

    if let Value::Table(map) = entry {
        // Remove any keys not in our allow-list
        for key in map.keys().cloned().collect::<Vec<_>>() {
            if !ALLOWED_KEYS.contains(&key.as_str()) {
                debug!("Removing disallowed key '{}' from config", key);
                map.remove(&key);
            }
        }
        // Ensure username & token exist
        for &key in ALLOWED_KEYS {
            if !map.contains_key(key) {
                warn!("Missing '{}' in config, inserting default", key);
                if let Some(default_val) = default_section.get(key) {
                    map.insert(key.into(), default_val.clone());
                }
            }
        }
    } else {
        warn!(
            "'factorio' was not a table (got {:?}), resetting to default",
            entry
        );
        *entry = default_section;
    }
}
