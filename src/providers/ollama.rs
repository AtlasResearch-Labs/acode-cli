//! Atlas Daedalus — Local Ollama Provider Implementation

use reqwest::Client;
use std::env;

use crate::providers::openai;
use crate::providers::types::{LLMMessage, LLMResponse, ProviderConfig};
use crate::tools::registry::ToolDefinition;

pub async fn complete(
    client: &Client,
    config: &ProviderConfig,
    messages: &[LLMMessage],
    tools: &[ToolDefinition],
) -> Result<LLMResponse, String> {
    let mut ollama_config = config.clone();
    if ollama_config.base_url.is_none() {
        let base =
            env::var("OLLAMA_BASE_URL").unwrap_or_else(|_| "http://localhost:11434/v1".to_string());
        ollama_config.base_url = Some(base);
    }
    if ollama_config.api_key.is_none() {
        ollama_config.api_key = Some("ollama".to_string());
    }

    openai::complete(client, &ollama_config, messages, tools).await
}

pub async fn complete_stream(
    client: &Client,
    config: &ProviderConfig,
    messages: &[LLMMessage],
    tools: &[ToolDefinition],
    on_token: &mut (dyn FnMut(&str) + Send),
) -> Result<LLMResponse, String> {
    let mut ollama_config = config.clone();
    if ollama_config.base_url.is_none() {
        let base =
            env::var("OLLAMA_BASE_URL").unwrap_or_else(|_| "http://localhost:11434/v1".to_string());
        ollama_config.base_url = Some(base);
    }
    if ollama_config.api_key.is_none() {
        ollama_config.api_key = Some("ollama".to_string());
    }

    openai::complete_stream(client, &ollama_config, messages, tools, on_token).await
}
