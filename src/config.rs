use color_eyre::eyre::{Result, eyre};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs;
use toml::{Value, map::Map};
use tracing::{debug, info, warn};

// Allowed keys per top-level section
const ALLOWED_SECTION_KEYS: &[(&str, &[&str])] = &[
    ("factorio", &["username", "token"]),
    (
        "mod-manager",
        &[
            "autoupdate-mods",
            "autoupdate-server",
            "autostart-when-finished",
        ],
    ),
];

#[derive(Serialize, Deserialize, Debug)]
pub struct FactorioConfig {
    pub username: String,
    pub token: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ModManagerConfig {
    #[serde(rename = "autoupdate-mods")]
    pub autoupdate_mods: bool,
    #[serde(rename = "autoupdate-server")]
    pub autoupdate_server: bool,
    #[serde(rename = "autostart-when-finished")]
    pub autostart_when_finished: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub factorio: FactorioConfig,
    #[serde(rename = "mod-manager")]
    pub mod_manager: ModManagerConfig,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            factorio: FactorioConfig {
                username: "my-username".into(),
                token: "my-token".into(),
            },
            mod_manager: ModManagerConfig {
                autoupdate_mods: true,
                autoupdate_server: true,
                autostart_when_finished: true,
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
    fs::write(path, &toml_str).await?;
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

    let default_cfg = Config::default();
    // Sanitize each known section
    for &(section, allowed_keys) in ALLOWED_SECTION_KEYS {
        // Generate the default section as a TOML Value
        let default_section = match section {
            "factorio" => toml::Value::try_from(&default_cfg.factorio).unwrap(),
            "mod-manager" => toml::Value::try_from(&default_cfg.mod_manager).unwrap(),
            _ => unreachable!(),
        };
        sanitize_section(table, section, allowed_keys, default_section);
    }

    let new_toml = toml::to_string_pretty(&doc)?;
    fs::write(path, &new_toml).await?;
    info!("Sanitized config written back to {:?}", path);

    let cfg: Config = toml::from_str(&new_toml)?;
    info!("Loaded configuration: {:?}", cfg);
    Ok(cfg)
}

/// Ensure a section table only contains allowed keys and has all required keys.
fn sanitize_section(
    root: &mut Map<String, Value>,
    section: &str,
    allowed_keys: &[&str],
    default_section: Value,
) {
    let entry = root
        .entry(section)
        .or_insert_with(|| default_section.clone());
    match entry {
        Value::Table(map) => {
            // Remove disallowed keys
            let to_remove: Vec<_> = map
                .keys()
                .filter(|k| !allowed_keys.contains(&k.as_str()))
                .cloned()
                .collect();
            for key in to_remove {
                debug!("Removing disallowed key '{}' from '{}'", key, section);
                map.remove(&key);
            }
            // Ensure required keys exist
            for &key in allowed_keys {
                if !map.contains_key(key) {
                    warn!(
                        "Missing '{}' in '{}' config, inserting default",
                        key, section
                    );
                    if let Some(default_val) = default_section.get(key) {
                        map.insert(key.into(), default_val.clone());
                    }
                }
            }
        }
        _ => {
            warn!(
                "'{}' was not a table (got {:?}), resetting to default",
                section, entry
            );
            *entry = default_section;
        }
    }
}
