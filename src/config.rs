use color_eyre::eyre::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use toml::Value;
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
                username: "XXX".to_string(),
                token: "XXX".to_string(),
            },
        }
    }
}

/// Load or initialize `mod-manager.toml` at `path`.
/// If missing, writes default.  If present, strips any extra keys.
pub async fn load_or_init(path: &Path) -> Result<Config> {
    info!("Loading config from {:?}", path);

    // If the file doesn't exist, write the default and return it
    if !path.exists() {
        info!("Config file not found, creating default at {:?}", path);
        let cfg = Config::default();
        let toml_str = toml::to_string_pretty(&cfg)?;
        tokio::fs::write(path, &toml_str).await?;
        info!("Default config written to {:?}", path);
        return Ok(cfg);
    }

    // Otherwise, read and sanitize existing config
    info!("Config file found at {:?}, reading contents", path);
    let contents = tokio::fs::read_to_string(path).await?;
    let mut doc: Value = toml::from_str(&contents)?;

    // Ensure root is table
    if !doc.is_table() {
        warn!(
            "Unexpected root in {:?}: expected table, got {:?}; overwriting with default",
            path, doc
        );
        let cfg = Config::default();
        let toml_str = toml::to_string_pretty(&cfg)?;
        tokio::fs::write(path, &toml_str).await?;
        info!("Rewrote invalid config with default at {:?}", path);
        return Ok(cfg);
    }

    // Strip disallowed keys and ensure required ones
    {
        let table = doc.as_table_mut().unwrap();
        let factorio = table
            .entry("factorio")
            .or_insert_with(|| Value::Table(toml::map::Map::new()));
        if let Value::Table(map) = factorio {
            // remove any key not in our allowed list
            let to_remove: Vec<_> = map
                .keys()
                .filter(|k| !ALLOWED_KEYS.contains(&k.as_str()))
                .cloned()
                .collect();
            for k in to_remove {
                debug!("Removing disallowed key '{}' from config", k);
                map.remove(&k);
            }
            // ensure username/token exist
            if !map.contains_key("username") {
                warn!("Missing 'username' in config, inserting default");
                map.insert("username".into(), Value::String("XXX".into()));
            }
            if !map.contains_key("token") {
                warn!("Missing 'token' in config, inserting default");
                map.insert("token".into(), Value::String("XXX".into()));
            }
        }
    }

    // Write sanitized config back to disk
    let new_toml = toml::to_string_pretty(&doc)?;
    tokio::fs::write(path, &new_toml).await?;
    info!("Sanitized config written back to {:?}", path);

    // Deserialize into our Config struct and return
    let cfg: Config = toml::from_str(&new_toml)?;
    info!("Loaded configuration: {:?}", cfg);
    Ok(cfg)
}
