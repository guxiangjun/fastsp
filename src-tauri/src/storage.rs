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

pub const DEFAULT_LLM_PROMPT: &str = r#"你是一个语音识别纠错助手。用户会提供语音识别的原始文本，其中可能包含：
- 同音字/近音字错误
- 语法不通顺
- 漏字或多字
- 标点符号问题

请修正这些错误，保持原意不变。

重要规则：
1. 只修正明显的错误，不要改变语义或风格
2. 如果原文已经正确，直接返回原文
3. 必须以 JSON 格式返回结果

输入文本：{text}

请以如下 JSON 格式返回（不要包含其他内容）：
{"corrected": "纠正后的文本"}"#;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LlmConfig {
    pub enabled: bool,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub custom_prompt: String, // Empty means use default
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            base_url: "https://api.openai.com/v1".to_string(),
            api_key: "".to_string(),
            model: "gpt-4o-mini".to_string(),
            custom_prompt: "".to_string(),
        }
    }
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
    #[serde(default)]
    pub llm_config: LlmConfig,
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
            llm_config: LlmConfig::default(),
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
