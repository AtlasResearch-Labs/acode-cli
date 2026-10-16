//! Atlas Daedalus — Universal Provider Router
//! Orchestrates LLM completions and streaming across Gemini, Anthropic, OpenAI/Powerbox, and Ollama.

use reqwest::Client;
use std::env;

use crate::providers::anthropic;
use crate::providers::gemini;
use crate::providers::ollama;
use crate::providers::openai;
use crate::providers::types::{LLMMessage, LLMResponse, ProviderConfig};
use crate::tools::registry::ToolDefinition;

pub struct ProviderRouter {
    pub config: ProviderConfig,
    client: Client,
}

impl ProviderRouter {
    pub fn new(override_model: Option<String>, override_provider: Option<String>) -> Self {
        let config = Self::resolve_config(override_model, override_provider);
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(45))
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self { config, client }
    }

    fn resolve_config(
        override_model: Option<String>,
        override_provider: Option<String>,
    ) -> ProviderConfig {
        let requested_provider = override_provider
            .or_else(|| env::var("ACODE_PROVIDER").ok())
            .or_else(|| env::var("DAEDALUS_PROVIDER").ok());

        let get_powerbox_key = || {
            env::var("POWERBOX_API_KEY")
                .or_else(|_| env::var("POWERBOX_MASTER_KEY"))
                .ok()
        };
        let get_powerbox_base = || {
            env::var("POWERBOX_BASE_URL")
                .unwrap_or_else(|_| "https://powerbox.atlasresearchlabs.online/v1".to_string())
        };
        let get_powerbox_model = |ov: Option<String>| {
            ov.or_else(|| env::var("DAEDALUS_MODEL").ok())
                .or_else(|| env::var("POWERBOX_MODEL").ok())
                .or_else(|| env::var("DEFAULT_MODEL").ok())
                .unwrap_or_else(|| "auto".to_string())
        };

        // 1. Explicit provider requested
        if let Some(p) = requested_provider {
            match p.to_lowercase().as_str() {
                "powerbox" => {
                    return ProviderConfig {
                        provider: "powerbox".to_string(),
                        model: get_powerbox_model(override_model),
                        api_key: get_powerbox_key(),
                        base_url: Some(get_powerbox_base()),
                        temperature: 0.2,
                    };
                }
                "gemini" => {
                    return ProviderConfig {
                        provider: "gemini".to_string(),
                        model: override_model
                            .or_else(|| env::var("GEMINI_MODEL").ok())
                            .unwrap_or_else(|| "gemini-2.5-flash".to_string()),
                        api_key: env::var("GEMINI_API_KEY").ok(),
                        base_url: None,
                        temperature: 0.2,
                    };
                }
                "anthropic" => {
                    return ProviderConfig {
                        provider: "anthropic".to_string(),
                        model: override_model
                            .or_else(|| env::var("ANTHROPIC_MODEL").ok())
                            .unwrap_or_else(|| "claude-3-5-sonnet-20241022".to_string()),
                        api_key: env::var("ANTHROPIC_API_KEY").ok(),
                        base_url: None,
                        temperature: 0.2,
                    };
                }
                "openai" => {
                    return ProviderConfig {
                        provider: "openai".to_string(),
                        model: override_model
                            .or_else(|| env::var("OPENAI_MODEL").ok())
                            .unwrap_or_else(|| "gpt-4o".to_string()),
                        api_key: env::var("OPENAI_API_KEY").ok(),
                        base_url: env::var("OPENAI_BASE_URL").ok(),
                        temperature: 0.2,
                    };
                }
                "ollama" => {
                    return ProviderConfig {
                        provider: "ollama".to_string(),
                        model: override_model
                            .or_else(|| env::var("OLLAMA_MODEL").ok())
                            .unwrap_or_else(|| "llama3".to_string()),
                        api_key: Some("ollama".to_string()),
                        base_url: Some(
                            env::var("OLLAMA_BASE_URL")
                                .unwrap_or_else(|_| "http://localhost:11434/v1".to_string()),
                        ),
                        temperature: 0.2,
                    };
                }
                _ => {}
            }
        }

        // 2. Priority check based on available credentials in environment
        // Powerbox is Priority #1 for Atlas Research Labs fleet
        if let Some(key) = get_powerbox_key() {
            return ProviderConfig {
                provider: "powerbox".to_string(),
                model: get_powerbox_model(override_model),
                api_key: Some(key),
                base_url: Some(get_powerbox_base()),
                temperature: 0.2,
            };
        }

        if let Ok(key) = env::var("ANTHROPIC_API_KEY") {
            return ProviderConfig {
                provider: "anthropic".to_string(),
                model: override_model.unwrap_or_else(|| {
                    env::var("ANTHROPIC_MODEL")
                        .unwrap_or_else(|_| "claude-3-5-sonnet-20241022".to_string())
                }),
                api_key: Some(key),
                base_url: None,
                temperature: 0.2,
            };
        }

        if let Ok(key) = env::var("GEMINI_API_KEY") {
            return ProviderConfig {
                provider: "gemini".to_string(),
                model: override_model.unwrap_or_else(|| {
                    env::var("GEMINI_MODEL").unwrap_or_else(|_| "gemini-2.5-flash".to_string())
                }),
                api_key: Some(key),
                base_url: None,
                temperature: 0.2,
            };
        }

        if let Ok(key) = env::var("OPENAI_API_KEY") {
            return ProviderConfig {
                provider: "openai".to_string(),
                model: override_model.unwrap_or_else(|| {
                    env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o".to_string())
                }),
                api_key: Some(key),
                base_url: env::var("OPENAI_BASE_URL").ok(),
                temperature: 0.2,
            };
        }

        // 3. Fallback default
        ProviderConfig {
            provider: "powerbox".to_string(),
            model: override_model.unwrap_or_else(|| "auto".to_string()),
            api_key: get_powerbox_key(),
            base_url: Some(get_powerbox_base()),
            temperature: 0.2,
        }
    }

    pub async fn complete(
        &self,
        messages: &[LLMMessage],
        tools: &[ToolDefinition],
    ) -> Result<LLMResponse, String> {
        match self.config.provider.as_str() {
            "gemini" => gemini::complete(&self.client, &self.config, messages, tools).await,
            "anthropic" => anthropic::complete(&self.client, &self.config, messages, tools).await,
            "ollama" => ollama::complete(&self.client, &self.config, messages, tools).await,
            "openai" | "powerbox" => {
                openai::complete(&self.client, &self.config, messages, tools).await
            }
            _ => gemini::complete(&self.client, &self.config, messages, tools).await,
        }
    }

    pub async fn complete_stream<F>(
        &self,
        messages: &[LLMMessage],
        tools: &[ToolDefinition],
        mut on_token: F,
    ) -> Result<LLMResponse, String>
    where
        F: FnMut(&str) + Send,
    {
        let res = match self.config.provider.as_str() {
            "gemini" => {
                gemini::complete_stream(&self.client, &self.config, messages, tools, &mut on_token)
                    .await
            }
            "anthropic" => {
                anthropic::complete_stream(
                    &self.client,
                    &self.config,
                    messages,
                    tools,
                    &mut on_token,
                )
                .await
            }
            "ollama" => {
                ollama::complete_stream(&self.client, &self.config, messages, tools, &mut on_token)
                    .await
            }
            "powerbox" if !tools.is_empty() => {
                let r = openai::complete(&self.client, &self.config, messages, tools).await;
                if let Ok(ref resp) = r {
                    if !resp.content.is_empty() {
                        on_token(&resp.content);
                    }
                }
                r
            }
            "openai" | "powerbox" => {
                openai::complete_stream(&self.client, &self.config, messages, tools, &mut on_token)
                    .await
            }
            _ => {
                gemini::complete_stream(&self.client, &self.config, messages, tools, &mut on_token)
                    .await
            }
        };

        // Fall back gracefully to standard completion if streaming fails
        match res {
            Ok(resp) => Ok(resp),
            Err(e) => self.complete(messages, tools).await.map_err(|orig_err| {
                format!(
                    "Streaming failed ({}) and non-streaming fallback failed: {}",
                    e, orig_err
                )
            }),
        }
    }
}

pub(crate) async fn post_with_retry(
    builder: reqwest::RequestBuilder,
    provider_name: &str,
) -> Result<reqwest::Response, String> {
    let mut attempts = 0;
    let max_attempts = 5;
    loop {
        attempts += 1;
        let req = match builder.try_clone() {
            Some(r) => r,
            None => {
                return Err(format!(
                    "{} request cannot be cloned for retry",
                    provider_name
                ))
            }
        };

        match req.send().await {
            Ok(resp) => {
                let status = resp.status();
                if (status.as_u16() == 429
                    || status.as_u16() == 502
                    || status.as_u16() == 503
                    || status.as_u16() == 504)
                    && attempts < max_attempts
                {
                    let delay = std::time::Duration::from_millis(1500 * attempts as u64);
                    tokio::time::sleep(delay).await;
                    continue;
                }
                return Ok(resp);
            }
            Err(e) => {
                if attempts >= max_attempts {
                    return Err(format!(
                        "{} network error (after {} attempts): {}",
                        provider_name, attempts, e
                    ));
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(1000 * attempts as u64))
                    .await;
            }
        }
    }
}
