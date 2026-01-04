use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use crate::storage::{LlmConfig, DEFAULT_LLM_PROMPT};

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessageResponse,
}

#[derive(Deserialize)]
struct ChatMessageResponse {
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct CorrectionResult {
    corrected: String,
}

/// Extract JSON from LLM response, handling potential markdown code blocks
fn extract_json(text: &str) -> &str {
    let text = text.trim();

    // Handle ```json ... ``` blocks
    if text.starts_with("```") {
        if let Some(start) = text.find('\n') {
            if let Some(end) = text.rfind("```") {
                if end > start {
                    return text[start + 1..end].trim();
                }
            }
        }
    }

    // Try to find JSON object directly
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            if end >= start {
                return &text[start..=end];
            }
        }
    }

    text
}

/// Correct text using LLM
pub async fn correct_text(text: &str, config: &LlmConfig) -> Result<String> {
    if !config.enabled || config.api_key.is_empty() {
        return Ok(text.to_string());
    }

    let prompt = if config.custom_prompt.is_empty() {
        DEFAULT_LLM_PROMPT.to_string()
    } else {
        config.custom_prompt.clone()
    };

    let prompt = prompt.replace("{text}", text);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let url = format!("{}/chat/completions", config.base_url.trim_end_matches('/'));

    let request = ChatRequest {
        model: config.model.clone(),
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: prompt,
        }],
        temperature: 0.3,
    };

    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", config.api_key))
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(anyhow!("LLM API error ({}): {}", status, error_text));
    }

    let chat_response: ChatResponse = response.json().await?;

    let content = chat_response
        .choices
        .first()
        .map(|c| c.message.content.clone())
        .ok_or_else(|| anyhow!("Empty response from LLM"))?;

    // Parse JSON response
    let json_str = extract_json(&content);

    match serde_json::from_str::<CorrectionResult>(json_str) {
        Ok(result) => Ok(result.corrected),
        Err(_) => {
            // If JSON parsing fails, try to use the content directly
            // This handles cases where LLM returns plain text
            eprintln!("LLM returned non-JSON response, using original text: {}", content);
            Ok(text.to_string())
        }
    }
}

/// Test LLM connection with a simple request
pub async fn test_connection(config: &LlmConfig) -> Result<String> {
    if config.api_key.is_empty() {
        return Err(anyhow!("API Key is empty"));
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    let url = format!("{}/chat/completions", config.base_url.trim_end_matches('/'));

    let request = ChatRequest {
        model: config.model.clone(),
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: "Say 'OK' to confirm connection.".to_string(),
        }],
        temperature: 0.0,
    };

    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", config.api_key))
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(anyhow!("API error ({}): {}", status, error_text));
    }

    let chat_response: ChatResponse = response.json().await?;

    let content = chat_response
        .choices
        .first()
        .map(|c| c.message.content.clone())
        .ok_or_else(|| anyhow!("Empty response"))?;

    Ok(format!("Connection successful! Model response: {}", content.chars().take(100).collect::<String>()))
}
