use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone)]
pub struct YrozConfig {
    #[serde(default)]
    pub disabled_backends: Vec<String>,
    #[serde(default)]
    pub priority: Vec<String>,
}

impl YrozConfig {
    pub fn default() -> Self {
        Self {
            disabled_backends: Vec::new(),
            priority: Vec::new(),
        }
    }

    pub fn load() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/yrozxm".to_string());
        let path = PathBuf::from(home).join(".config/yroz/config.toml");

        if !path.exists() {
            return Self::default();
        }

        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return Self::default(),
        };

        toml::from_str(&content).unwrap_or_else(|_| Self::default())
    }
}
