use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use anyhow::Result;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ModelVersion {
    #[default]
    Quantized,
    Unquantized,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppConfig {
    pub trigger_mouse: bool,
    pub trigger_hold: bool,
    pub trigger_toggle: bool,
    pub language: String,
    pub model_dir: String,
    #[serde(default)]
    pub model_version: ModelVersion,
    #[serde(default)]
    pub input_device: String, // Empty string means default device
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            trigger_mouse: true,
            trigger_hold: true,
            trigger_toggle: true,
            language: "".to_string(), // Auto
            model_dir: "./models/sense-voice".to_string(),
            model_version: ModelVersion::default(),
            input_device: "".to_string(), // Default device
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HistoryItem {
    pub id: String,
    pub timestamp: String,
    pub text: String,
    pub duration_ms: u64,
}

pub struct StorageService {
    config_path: PathBuf,
    history_path: PathBuf,
}

impl StorageService {
    pub fn new(app_dir: PathBuf) -> Self {
        if !app_dir.exists() {
            fs::create_dir_all(&app_dir).ok();
        }
        Self {
            config_path: app_dir.join("config.json"),
            history_path: app_dir.join("history.json"),
        }
    }

    pub fn load_config(&self) -> AppConfig {
        if let Ok(content) = fs::read_to_string(&self.config_path) {
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            AppConfig::default()
        }
    }

    pub fn save_config(&self, config: &AppConfig) -> Result<()> {
        let content = serde_json::to_string_pretty(config)?;
        fs::write(&self.config_path, content)?;
        Ok(())
    }

    pub fn load_history(&self) -> Vec<HistoryItem> {
        if let Ok(content) = fs::read_to_string(&self.history_path) {
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            Vec::new()
        }
    }

    pub fn save_history(&self, history: &Vec<HistoryItem>) -> Result<()> {
        let content = serde_json::to_string_pretty(history)?;
        fs::write(&self.history_path, content)?;
        Ok(())
    }

    pub fn add_history_item(&self, item: HistoryItem) -> Result<()> {
        let mut history = self.load_history();
        history.insert(0, item); // Newest first
        self.save_history(&history)
    }
    
    pub fn clear_history(&self) -> Result<()> {
        self.save_history(&Vec::new())
    }
}
