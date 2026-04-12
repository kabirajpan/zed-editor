use tokio::sync::mpsc;
use serde_json::json;
use futures_util::StreamExt;

/// Represents available AI providers
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ProviderType {
    Anthropic,
    Ollama,
    Grok,
    Groq,
}

/// The common interface for all AI Backends.
/// Handles the complexity of different API schemas and streaming protocols.
pub trait ModelProvider: Send + Sync {
    /// Starts a streaming completion. Tokens are sent back via the provided channel.
    fn stream_completion(
        &self,
        system_prompt: String,
        user_prompt: String,
        api_key: String,
        sender: mpsc::UnboundedSender<String>,
    );

    fn stream_chat(
        &self,
        messages: Vec<crate::ai::chat::ChatMessage>,
        api_key: String,
        sender: mpsc::UnboundedSender<String>,
    );
}

pub struct AnthropicProvider;

impl ModelProvider for AnthropicProvider {
    fn stream_completion(
        &self,
        system_prompt: String,
        user_prompt: String,
        api_key: String,
        sender: mpsc::UnboundedSender<String>,
    ) {
        let client = reqwest::Client::new();
        let api_key = api_key.clone();

        tokio::spawn(async move {
            let res = client
                .post("https://api.anthropic.com/v1/messages")
                .header("x-api-key", api_key)
                .header("anthropic-version", "2023-06-01")
                .header("content-type", "application/json")
                .json(&json!({
                    "model": "claude-3-5-sonnet-20240620",
                    "max_tokens": 1024,
                    "system": system_prompt,
                    "messages": [{"role": "user", "content": user_prompt}],
                    "stream": true,
                }))
                .send()
                .await;

            match res {
                Ok(response) => {
                    let mut stream = response.bytes_stream();
                    while let Some(chunk) = stream.next().await {
                        if let Ok(bytes) = chunk {
                            let text = String::from_utf8_lossy(&bytes);
                            for line in text.lines() {
                                if line.starts_with("data: ") {
                                    let json_str = &line[6..];
                                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str) {
                                        if let Some(delta) = val.get("delta") {
                                            if let Some(t) = delta.get("text") {
                                                if let Some(content) = t.as_str() {
                                                    let _ = sender.send(content.to_string());
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    let _ = sender.send(format!("\n❌ Error: {}", e));
                }
            }
        });
    }

    fn stream_chat(
        &self,
        messages: Vec<crate::ai::chat::ChatMessage>,
        api_key: String,
        sender: mpsc::UnboundedSender<String>,
    ) {
        let client = reqwest::Client::new();
        let api_key = api_key.clone();

        tokio::spawn(async move {
            let anthropic_messages: Vec<serde_json::Value> = messages
                .iter()
                .filter(|m| m.role != crate::ai::chat::MessageRole::System)
                .map(|m| {
                    let role = match m.role {
                        crate::ai::chat::MessageRole::Assistant => "assistant",
                        _ => "user",
                    };
                    json!({"role": role, "content": m.content})
                })
                .collect();

            let system_msg = messages
                .iter()
                .find(|m| m.role == crate::ai::chat::MessageRole::System)
                .map(|m| m.content.clone())
                .unwrap_or_default();

            let res = client
                .post("https://api.anthropic.com/v1/messages")
                .header("x-api-key", api_key)
                .header("anthropic-version", "2023-06-01")
                .header("content-type", "application/json")
                .json(&json!({
                    "model": "claude-3-5-sonnet-20240620",
                    "max_tokens": 1024,
                    "system": system_msg,
                    "messages": anthropic_messages,
                    "stream": true,
                }))
                .send()
                .await;

            match res {
                Ok(response) => {
                    let mut stream = response.bytes_stream();
                    while let Some(chunk) = stream.next().await {
                        if let Ok(bytes) = chunk {
                            let text = String::from_utf8_lossy(&bytes);
                            for line in text.lines() {
                                if line.starts_with("data: ") {
                                    let json_str = &line[6..];
                                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str) {
                                        if let Some(delta) = val.get("delta") {
                                            if let Some(t) = delta.get("text") {
                                                if let Some(content) = t.as_str() {
                                                    let _ = sender.send(content.to_string());
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    let _ = sender.send(format!("\n❌ Error: {}", e));
                }
            }
        });
    }
}

pub struct OllamaProvider;

impl ModelProvider for OllamaProvider {
    fn stream_completion(
        &self,
        system_prompt: String,
        user_prompt: String,
        _api_key: String,
        sender: mpsc::UnboundedSender<String>,
    ) {
        let client = reqwest::Client::new();

        tokio::spawn(async move {
            let res = client
                .post("http://localhost:11434/api/generate")
                .json(&json!({
                    "model": "deepseek-coder",
                    "system": system_prompt,
                    "prompt": user_prompt,
                    "stream": true,
                }))
                .send()
                .await;

            match res {
                Ok(response) => {
                    let mut stream = response.bytes_stream();
                    while let Some(chunk) = stream.next().await {
                        if let Ok(bytes) = chunk {
                            if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                                if let Some(response_text) = val.get("response") {
                                    if let Some(content) = response_text.as_str() {
                                        let _ = sender.send(content.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    let _ = sender.send(format!("\n❌ Error: {}", e));
                }
            }
        });
    }

    fn stream_chat(
        &self,
        _messages: Vec<crate::ai::chat::ChatMessage>,
        _api_key: String,
        _sender: mpsc::UnboundedSender<String>,
    ) {
        // Placeholder for Ollama chat
    }
}

pub struct GrokProvider;

impl ModelProvider for GrokProvider {
    fn stream_completion(
        &self,
        system_prompt: String,
        user_prompt: String,
        api_key: String,
        sender: mpsc::UnboundedSender<String>,
    ) {
        let client = reqwest::Client::new();
        let api_key = api_key.clone();

        tokio::spawn(async move {
            let res = client
                .post("https://api.x.ai/v1/chat/completions")
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json")
                .json(&json!({
                    "model": "grok-beta",
                    "messages": [
                        {"role": "system", "content": system_prompt},
                        {"role": "user", "content": user_prompt}
                    ],
                    "stream": true,
                }))
                .send()
                .await;

            match res {
                Ok(response) => {
                    let mut stream = response.bytes_stream();
                    while let Some(chunk) = stream.next().await {
                        if let Ok(bytes) = chunk {
                            let text = String::from_utf8_lossy(&bytes);
                            for line in text.lines() {
                                if line.starts_with("data: ") {
                                    let json_str = &line[6..];
                                    if json_str.trim() == "[DONE]" { break; }
                                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str) {
                                        if let Some(content) = val["choices"][0]["delta"]["content"].as_str() {
                                            let _ = sender.send(content.to_string());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    let _ = sender.send(format!("\n❌ Error: {}", e));
                }
            }
        });
    }

    fn stream_chat(
        &self,
        messages: Vec<crate::ai::chat::ChatMessage>,
        api_key: String,
        sender: mpsc::UnboundedSender<String>,
    ) {
        let client = reqwest::Client::new();
        let api_key = api_key.clone();

        tokio::spawn(async move {
            let openai_messages: Vec<serde_json::Value> = messages
                .iter()
                .map(|m| {
                    let role = match m.role {
                        crate::ai::chat::MessageRole::System => "system",
                        crate::ai::chat::MessageRole::Assistant => "assistant",
                        _ => "user",
                    };
                    json!({"role": role, "content": m.content})
                })
                .collect();

            let res = client
                .post("https://api.x.ai/v1/chat/completions")
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json")
                .json(&json!({
                    "model": "grok-beta",
                    "messages": openai_messages,
                    "stream": true,
                }))
                .send()
                .await;

            match res {
                Ok(response) => {
                    let mut stream = response.bytes_stream();
                    while let Some(chunk) = stream.next().await {
                        if let Ok(bytes) = chunk {
                            let text = String::from_utf8_lossy(&bytes);
                            for line in text.lines() {
                                if line.starts_with("data: ") {
                                    let json_str = &line[6..];
                                    if json_str.trim() == "[DONE]" { break; }
                                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str) {
                                        if let Some(content) = val["choices"][0]["delta"]["content"].as_str() {
                                            let _ = sender.send(content.to_string());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    let _ = sender.send(format!("\n❌ Error: {}", e));
                }
            }
        });
    }
}

pub struct GroqProvider;

impl ModelProvider for GroqProvider {
    fn stream_completion(
        &self,
        system_prompt: String,
        user_prompt: String,
        api_key: String,
        sender: mpsc::UnboundedSender<String>,
    ) {
        let client = reqwest::Client::new();
        let api_key = api_key.clone();

        tokio::spawn(async move {
            let res = client
                .post("https://api.groq.com/openai/v1/chat/completions")
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json")
                .json(&json!({
                    "model": "llama-3.1-8b-instant",
                    "messages": [
                        {"role": "system", "content": system_prompt},
                        {"role": "user", "content": user_prompt}
                    ],
                    "stream": true,
                }))
                .send()
                .await;

            match res {
                Ok(response) => {
                    let mut stream = response.bytes_stream();
                    while let Some(chunk) = stream.next().await {
                        if let Ok(bytes) = chunk {
                            let text = String::from_utf8_lossy(&bytes);
                            for line in text.lines() {
                                if line.starts_with("data: ") {
                                    let json_str = &line[6..];
                                    if json_str.trim() == "[DONE]" { break; }
                                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str) {
                                        if let Some(content) = val["choices"][0]["delta"]["content"].as_str() {
                                            let _ = sender.send(content.to_string());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    let _ = sender.send(format!("\n❌ Error: {}", e));
                }
            }
        });
    }

    fn stream_chat(
        &self,
        messages: Vec<crate::ai::chat::ChatMessage>,
        api_key: String,
        sender: mpsc::UnboundedSender<String>,
    ) {
        let client = reqwest::Client::new();
        let api_key = api_key.clone();

        tokio::spawn(async move {
            let openai_messages: Vec<serde_json::Value> = messages
                .iter()
                .map(|m| {
                    let role = match m.role {
                        crate::ai::chat::MessageRole::System => "system",
                        crate::ai::chat::MessageRole::Assistant => "assistant",
                        _ => "user",
                    };
                    json!({"role": role, "content": m.content})
                })
                .collect();

            let res = client
                .post("https://api.groq.com/openai/v1/chat/completions")
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json")
                .json(&json!({
                    "model": "llama-3.1-8b-instant",
                    "messages": openai_messages,
                    "stream": true,
                }))
                .send()
                .await;

            match res {
                Ok(response) => {
                    let mut stream = response.bytes_stream();
                    while let Some(chunk) = stream.next().await {
                        if let Ok(bytes) = chunk {
                            let text = String::from_utf8_lossy(&bytes);
                            for line in text.lines() {
                                if line.starts_with("data: ") {
                                    let json_str = &line[6..];
                                    if json_str.trim() == "[DONE]" { break; }
                                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str) {
                                        if let Some(content) = val["choices"][0]["delta"]["content"].as_str() {
                                            let _ = sender.send(content.to_string());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    let _ = sender.send(format!("\n❌ Error: {}", e));
                }
            }
        });
    }
}
