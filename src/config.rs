use color_eyre::eyre::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use toml::Value;

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
/// If missing, writes default.  If present, strips any extra keys
pub async fn load_or_init(path: &Path) -> Result<Config> {
    if !path.exists() {
        let cfg = Config::default();
        let toml_str = toml::to_string_pretty(&cfg)?;
        tokio::fs::write(path, toml_str).await?;
        return Ok(cfg);
    }

    let contents = tokio::fs::read_to_string(path).await?;
    let mut doc: Value = toml::from_str(&contents)?;

    {
        let table = doc
            .as_table_mut()
            .expect("mod-manager.toml root must be a table");
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
                map.remove(&k);
            }
            // ensure username/token exist
            if !map.contains_key("username") {
                map.insert("username".into(), Value::String("XXX".into()));
            }
            if !map.contains_key("token") {
                map.insert("token".into(), Value::String("XXX".into()));
            }
        }
    }

    let new_toml = toml::to_string_pretty(&doc)?;
    tokio::fs::write(path, &new_toml).await?;

    let cfg: Config = toml::from_str(&new_toml)?;
    Ok(cfg)
}
