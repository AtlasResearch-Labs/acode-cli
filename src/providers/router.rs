//! Atlas Daedalus — Universal Provider Router

use chrono::Utc;
use reqwest::Client;
use serde_json::{json, Value};
use std::env;

use crate::providers::types::{LLMMessage, LLMResponse, ProviderConfig, ToolCall};
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

    async fn post_with_retry(
        &self,
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
            "gemini" => self.complete_gemini(messages, tools).await,
            "anthropic" => self.complete_anthropic(messages, tools).await,
            "openai" | "powerbox" | "ollama" => {
                self.complete_openai_compatible(messages, tools).await
            }
            _ => self.complete_gemini(messages, tools).await,
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
                self.complete_stream_gemini(messages, tools, &mut on_token)
                    .await
            }
            "anthropic" => {
                self.complete_stream_anthropic(messages, tools, &mut on_token)
                    .await
            }
            "powerbox" if !tools.is_empty() => {
                let r = self.complete_openai_compatible(messages, tools).await;
                if let Ok(ref resp) = r {
                    if !resp.content.is_empty() {
                        on_token(&resp.content);
                    }
                }
                r
            }
            "openai" | "powerbox" | "ollama" => {
                self.complete_stream_openai_compatible(messages, tools, &mut on_token)
                    .await
            }
            _ => {
                self.complete_stream_gemini(messages, tools, &mut on_token)
                    .await
            }
        };

        // Fall back gracefully to standard completion if streaming stream error occurs
        match res {
            Ok(resp) => Ok(resp),
            Err(e) => {
                // If stream connection failed, try standard fallback
                self.complete(messages, tools).await.map_err(|orig_err| {
                    format!(
                        "Streaming failed ({}) and non-streaming fallback failed: {}",
                        e, orig_err
                    )
                })
            }
        }
    }

    async fn complete_gemini(
        &self,
        messages: &[LLMMessage],
        tools: &[ToolDefinition],
    ) -> Result<LLMResponse, String> {
        let api_key = self
            .config
            .api_key
            .clone()
            .or_else(|| env::var("GEMINI_API_KEY").ok())
            .unwrap_or_default();
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
            self.config.model
        );

        let system_text = messages
            .iter()
            .find(|m| m.role == "system")
            .map(|m| m.content.clone());

        let mut contents = Vec::new();
        for m in messages.iter().filter(|m| m.role != "system") {
            let role = if m.role == "assistant" {
                "model"
            } else {
                "user"
            };

            if let Some(tool_calls) = &m.tool_calls {
                let parts: Vec<Value> = tool_calls
                    .iter()
                    .map(|tc| {
                        json!({
                            "functionCall": {
                                "name": tc.name,
                                "args": tc.arguments
                            }
                        })
                    })
                    .collect();
                contents.push(json!({ "role": "model", "parts": parts }));
                continue;
            }

            if m.role == "tool" {
                contents.push(json!({
                    "role": "user",
                    "parts": [{
                        "functionResponse": {
                            "name": m.name.clone().unwrap_or_else(|| "tool".to_string()),
                            "response": { "output": m.content }
                        }
                    }]
                }));
                continue;
            }

            contents.push(json!({
                "role": role,
                "parts": [{ "text": m.content }]
            }));
        }

        let func_decls: Vec<Value> = tools
            .iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.parameters
                })
            })
            .collect();

        let mut body = json!({
            "contents": contents
        });

        if let Some(sys) = system_text {
            body["systemInstruction"] = json!({ "parts": [{ "text": sys }] });
        }

        if !func_decls.is_empty() {
            body["tools"] = json!([{ "functionDeclarations": func_decls }]);
        }

        let req = self
            .client
            .post(&url)
            .header("x-goog-api-key", &api_key)
            .json(&body);
        let mut resp = self.post_with_retry(req, "GEMINI").await?;

        if resp.status().as_u16() == 429 && self.config.model != "gemini-2.0-flash" {
            let fallback_url = "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent";
            println!("\n{}[ACODE] Active model '{}' reached rate limit (429). Automatically falling back to gemini-2.0-flash (1,500 RPD)...{}\n", crate::cli::tui::TuiRenderer::YELLOW, self.config.model, crate::cli::tui::TuiRenderer::RESET);
            let fb_req = self
                .client
                .post(fallback_url)
                .header("x-goog-api-key", &api_key)
                .json(&body);
            if let Ok(fb_resp) = fb_req.send().await {
                if fb_resp.status().is_success() {
                    resp = fb_resp;
                }
            }
        }

        if !resp.status().is_success() {
            let status = resp.status();
            let err_body = resp.text().await.unwrap_or_default();
            if status.as_u16() == 429 {
                return Err(format!(
                    "Gemini API Quota Exceeded (429). The model '{}' reached its rate limit.\n\
                    Suggested Actions:\n\
                      1. Switch model: /model gemini-2.0-flash (1,500 requests/day on free tier)\n\
                      2. Configure key: /key gemini <your_key> or /key anthropic <key>\n\
                      3. Check rate limits: https://ai.dev/rate-limit",
                    self.config.model
                ));
            }
            return Err(format!("Gemini API error {}: {}", status, err_body));
        }

        let json_resp: Value = resp
            .json()
            .await
            .map_err(|e| format!("Gemini parse error: {}", e))?;

        let candidate = json_resp.get("candidates").and_then(|c| c.get(0));
        let parts = candidate
            .and_then(|c| c.get("content"))
            .and_then(|c| c.get("parts"))
            .and_then(|p| p.as_array());

        let mut text = String::new();
        let mut tool_calls = Vec::new();

        if let Some(parts_arr) = parts {
            for part in parts_arr {
                if let Some(t) = part.get("text").and_then(|t| t.as_str()) {
                    text.push_str(t);
                }
                if let Some(fc) = part.get("functionCall") {
                    let name = fc
                        .get("name")
                        .and_then(|n| n.as_str())
                        .unwrap_or("")
                        .to_string();
                    let args = fc.get("args").cloned().unwrap_or(json!({}));
                    tool_calls.push(ToolCall {
                        id: format!("call_{}", Utc::now().timestamp_millis()),
                        name,
                        arguments: args,
                    });
                }
            }
        }

        let prompt_tokens =
            (serde_json::to_string(&body).map(|s| s.len()).unwrap_or(100) / 4).max(1);
        let completion_tokens = (text.len() / 4).max(1);

        Ok(LLMResponse {
            content: text,
            tool_calls: if tool_calls.is_empty() {
                None
            } else {
                Some(tool_calls)
            },
            model: self.config.model.clone(),
            prompt_tokens,
            completion_tokens,
        })
    }

    async fn complete_anthropic(
        &self,
        messages: &[LLMMessage],
        tools: &[ToolDefinition],
    ) -> Result<LLMResponse, String> {
        let api_key = self
            .config
            .api_key
            .clone()
            .or_else(|| env::var("ANTHROPIC_API_KEY").ok())
            .unwrap_or_default();
        let url = "https://api.anthropic.com/v1/messages";

        let system_text = messages
            .iter()
            .find(|m| m.role == "system")
            .map(|m| m.content.clone());

        let mut anthropic_messages = Vec::new();
        for m in messages.iter().filter(|m| m.role != "system") {
            if m.role == "tool" {
                anthropic_messages.push(json!({
                    "role": "user",
                    "content": [{
                        "type": "tool_result",
                        "tool_use_id": m.tool_call_id.clone().unwrap_or_else(|| "call_1".to_string()),
                        "content": m.content
                    }]
                }));
                continue;
            }

            if let Some(tool_calls) = &m.tool_calls {
                let parts: Vec<Value> = tool_calls
                    .iter()
                    .map(|tc| {
                        json!({
                            "type": "tool_use",
                            "id": tc.id,
                            "name": tc.name,
                            "input": tc.arguments
                        })
                    })
                    .collect();
                anthropic_messages.push(json!({ "role": "assistant", "content": parts }));
                continue;
            }

            anthropic_messages.push(json!({
                "role": m.role,
                "content": m.content
            }));
        }

        let anthropic_tools: Vec<Value> = tools
            .iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": t.parameters
                })
            })
            .collect();

        let mut body = json!({
            "model": self.config.model,
            "max_tokens": 8192,
            "messages": anthropic_messages
        });

        if let Some(sys) = system_text {
            body["system"] = json!(sys);
        }

        if !anthropic_tools.is_empty() {
            body["tools"] = json!(anthropic_tools);
        }

        let req = self
            .client
            .post(url)
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body);
        let resp = self.post_with_retry(req, "ANTHROPIC").await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err_body = resp.text().await.unwrap_or_default();
            return Err(format!("Anthropic API error {}: {}", status, err_body));
        }

        let json_resp: Value = resp
            .json()
            .await
            .map_err(|e| format!("Anthropic parse error: {}", e))?;

        let mut text = String::new();
        let mut tool_calls = Vec::new();

        if let Some(content_arr) = json_resp.get("content").and_then(|c| c.as_array()) {
            for block in content_arr {
                if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                    if let Some(t) = block.get("text").and_then(|t| t.as_str()) {
                        text.push_str(t);
                    }
                } else if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                    let id = block
                        .get("id")
                        .and_then(|i| i.as_str())
                        .unwrap_or("")
                        .to_string();
                    let name = block
                        .get("name")
                        .and_then(|n| n.as_str())
                        .unwrap_or("")
                        .to_string();
                    let input = block.get("input").cloned().unwrap_or(json!({}));
                    tool_calls.push(ToolCall {
                        id,
                        name,
                        arguments: input,
                    });
                }
            }
        }

        let prompt_tokens = json_resp
            .get("usage")
            .and_then(|u| u.get("input_tokens"))
            .and_then(|t| t.as_u64())
            .unwrap_or(100) as usize;
        let completion_tokens = json_resp
            .get("usage")
            .and_then(|u| u.get("output_tokens"))
            .and_then(|t| t.as_u64())
            .unwrap_or(50) as usize;

        Ok(LLMResponse {
            content: text,
            tool_calls: if tool_calls.is_empty() {
                None
            } else {
                Some(tool_calls)
            },
            model: self.config.model.clone(),
            prompt_tokens,
            completion_tokens,
        })
    }

    async fn complete_openai_compatible(
        &self,
        messages: &[LLMMessage],
        tools: &[ToolDefinition],
    ) -> Result<LLMResponse, String> {
        let base_url = self.config.base_url.clone().unwrap_or_else(|| {
            if self.config.provider == "powerbox" {
                "https://powerbox.atlasresearchlabs.online/v1".to_string()
            } else {
                "https://api.openai.com/v1".to_string()
            }
        });
        let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
        let api_key = self
            .config
            .api_key
            .clone()
            .or_else(|| {
                if self.config.provider == "powerbox" {
                    env::var("POWERBOX_API_KEY")
                        .or_else(|_| env::var("POWERBOX_MASTER_KEY"))
                        .ok()
                } else {
                    env::var("OPENAI_API_KEY").ok()
                }
            })
            .unwrap_or_else(|| "dummy".to_string());

        let mut formatted_msgs = Vec::new();
        for m in messages {
            if m.role == "tool" {
                let mut tool_obj = json!({
                    "role": "tool",
                    "tool_call_id": m.tool_call_id.clone().unwrap_or_else(|| "call_1".to_string()),
                    "content": m.content
                });
                if let Some(n) = &m.name {
                    tool_obj["name"] = json!(n);
                }
                formatted_msgs.push(tool_obj);
                continue;
            }

            if let Some(tool_calls) = &m.tool_calls {
                let calls: Vec<Value> = tool_calls
                    .iter()
                    .map(|tc| {
                        json!({
                            "id": tc.id,
                            "type": "function",
                            "function": {
                                "name": tc.name,
                                "arguments": tc.arguments.to_string()
                            }
                        })
                    })
                    .collect();
                formatted_msgs.push(json!({
                    "role": "assistant",
                    "content": if m.content.is_empty() { Value::Null } else { json!(m.content) },
                    "tool_calls": calls
                }));
                continue;
            }

            formatted_msgs.push(json!({
                "role": m.role,
                "content": m.content
            }));
        }

        let openai_tools: Vec<Value> = tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters
                    }
                })
            })
            .collect();

        let mut body = json!({
            "model": self.config.model,
            "messages": formatted_msgs,
            "temperature": self.config.temperature
        });

        if !openai_tools.is_empty() {
            body["tools"] = json!(openai_tools);
        }

        let req = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&body);
        let resp = self
            .post_with_retry(req, &self.config.provider.to_uppercase())
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err_body = resp.text().await.unwrap_or_default();
            return Err(format!(
                "{} API error {}: {}",
                self.config.provider.to_uppercase(),
                status,
                err_body
            ));
        }

        let json_resp: Value = resp
            .json()
            .await
            .map_err(|e| format!("{} parse error: {}", self.config.provider.to_uppercase(), e))?;

        let choice = json_resp.get("choices").and_then(|c| c.get(0));
        let message = choice.and_then(|c| c.get("message"));
        let mut text = message
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .to_string();
        if text.is_empty() {
            if let Some(reasoning) = message
                .and_then(|m| m.get("reasoning"))
                .and_then(|c| c.as_str())
            {
                text = reasoning.to_string();
            }
        }

        let mut tool_calls = Vec::new();
        if let Some(calls_arr) = message
            .and_then(|m| m.get("tool_calls"))
            .and_then(|c| c.as_array())
        {
            for call in calls_arr {
                let id = call
                    .get("id")
                    .and_then(|i| i.as_str())
                    .unwrap_or("")
                    .to_string();
                let func = call.get("function");
                let name = func
                    .and_then(|f| f.get("name"))
                    .and_then(|n| n.as_str())
                    .unwrap_or("")
                    .to_string();
                let raw_args = func
                    .and_then(|f| f.get("arguments"))
                    .and_then(|a| a.as_str())
                    .unwrap_or("{}");
                let args: Value = serde_json::from_str(raw_args).unwrap_or(json!({}));
                tool_calls.push(ToolCall {
                    id,
                    name,
                    arguments: args,
                });
            }
        }

        let prompt_tokens = json_resp
            .get("usage")
            .and_then(|u| u.get("prompt_tokens"))
            .and_then(|t| t.as_u64())
            .unwrap_or(100) as usize;
        let completion_tokens = json_resp
            .get("usage")
            .and_then(|u| u.get("completion_tokens"))
            .and_then(|t| t.as_u64())
            .unwrap_or(50) as usize;

        Ok(LLMResponse {
            content: text,
            tool_calls: if tool_calls.is_empty() {
                None
            } else {
                Some(tool_calls)
            },
            model: self.config.model.clone(),
            prompt_tokens,
            completion_tokens,
        })
    }

    async fn complete_stream_gemini(
        &self,
        messages: &[LLMMessage],
        tools: &[ToolDefinition],
        on_token: &mut (dyn FnMut(&str) + Send),
    ) -> Result<LLMResponse, String> {
        let api_key = self
            .config
            .api_key
            .clone()
            .or_else(|| env::var("GEMINI_API_KEY").ok())
            .unwrap_or_default();
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse",
            self.config.model
        );

        let system_text = messages
            .iter()
            .find(|m| m.role == "system")
            .map(|m| m.content.clone());

        let mut contents = Vec::new();
        for m in messages.iter().filter(|m| m.role != "system") {
            let role = if m.role == "assistant" {
                "model"
            } else {
                "user"
            };

            if let Some(tool_calls) = &m.tool_calls {
                let parts: Vec<Value> = tool_calls
                    .iter()
                    .map(|tc| {
                        json!({
                            "functionCall": {
                                "name": tc.name,
                                "args": tc.arguments
                            }
                        })
                    })
                    .collect();
                contents.push(json!({ "role": "model", "parts": parts }));
                continue;
            }

            if m.role == "tool" {
                contents.push(json!({
                    "role": "user",
                    "parts": [{
                        "functionResponse": {
                            "name": m.name.clone().unwrap_or_else(|| "tool".to_string()),
                            "response": { "output": m.content }
                        }
                    }]
                }));
                continue;
            }

            contents.push(json!({
                "role": role,
                "parts": [{ "text": m.content }]
            }));
        }

        let func_decls: Vec<Value> = tools
            .iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.parameters
                })
            })
            .collect();

        let mut body = json!({
            "contents": contents
        });

        if let Some(sys) = system_text {
            body["systemInstruction"] = json!({ "parts": [{ "text": sys }] });
        }

        if !func_decls.is_empty() {
            body["tools"] = json!([{ "functionDeclarations": func_decls }]);
        }

        let req = self
            .client
            .post(&url)
            .header("x-goog-api-key", &api_key)
            .json(&body);
        let resp = self.post_with_retry(req, "GEMINI_STREAM").await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err_body = resp.text().await.unwrap_or_default();
            return Err(format!("Gemini API error {}: {}", status, err_body));
        }

        use futures_util::StreamExt;
        let mut stream = resp.bytes_stream();
        let mut buffer = String::new();
        let mut full_text = String::new();
        let mut tool_calls = Vec::new();

        while let Some(chunk_res) = stream.next().await {
            let chunk = chunk_res.map_err(|e| format!("Stream read error: {}", e))?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(pos) = buffer.find('\n') {
                let line = buffer[..pos].trim().to_string();
                buffer.drain(..=pos);

                if line.is_empty() || line.starts_with(':') {
                    continue;
                }

                if let Some(payload) = line.strip_prefix("data:") {
                    let data = payload.trim();
                    if let Ok(val) = serde_json::from_str::<Value>(data) {
                        if let Some(candidates) = val.get("candidates").and_then(|c| c.as_array()) {
                            if let Some(candidate) = candidates.first() {
                                if let Some(parts) = candidate
                                    .get("content")
                                    .and_then(|c| c.get("parts"))
                                    .and_then(|p| p.as_array())
                                {
                                    for part in parts {
                                        if let Some(txt) = part.get("text").and_then(|t| t.as_str())
                                        {
                                            full_text.push_str(txt);
                                            on_token(txt);
                                        }
                                        if let Some(fc) = part.get("functionCall") {
                                            let name = fc
                                                .get("name")
                                                .and_then(|n| n.as_str())
                                                .unwrap_or("")
                                                .to_string();
                                            let args = fc.get("args").cloned().unwrap_or(json!({}));
                                            tool_calls.push(ToolCall {
                                                id: format!(
                                                    "call_{}",
                                                    Utc::now().timestamp_millis()
                                                ),
                                                name,
                                                arguments: args,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let prompt_tokens =
            (serde_json::to_string(&body).map(|s| s.len()).unwrap_or(100) / 4).max(1);
        let completion_tokens = (full_text.len() / 4).max(1);

        Ok(LLMResponse {
            content: full_text,
            tool_calls: if tool_calls.is_empty() {
                None
            } else {
                Some(tool_calls)
            },
            model: self.config.model.clone(),
            prompt_tokens,
            completion_tokens,
        })
    }

    async fn complete_stream_anthropic(
        &self,
        messages: &[LLMMessage],
        tools: &[ToolDefinition],
        on_token: &mut (dyn FnMut(&str) + Send),
    ) -> Result<LLMResponse, String> {
        let api_key = self
            .config
            .api_key
            .clone()
            .or_else(|| env::var("ANTHROPIC_API_KEY").ok())
            .unwrap_or_default();
        let url = "https://api.anthropic.com/v1/messages";

        let system_text = messages
            .iter()
            .find(|m| m.role == "system")
            .map(|m| m.content.clone());

        let mut anthropic_messages = Vec::new();
        for m in messages.iter().filter(|m| m.role != "system") {
            if m.role == "tool" {
                anthropic_messages.push(json!({
                    "role": "user",
                    "content": [{
                        "type": "tool_result",
                        "tool_use_id": m.tool_call_id.clone().unwrap_or_else(|| "call_1".to_string()),
                        "content": m.content
                    }]
                }));
                continue;
            }

            if let Some(tool_calls) = &m.tool_calls {
                let parts: Vec<Value> = tool_calls
                    .iter()
                    .map(|tc| {
                        json!({
                            "type": "tool_use",
                            "id": tc.id,
                            "name": tc.name,
                            "input": tc.arguments
                        })
                    })
                    .collect();
                anthropic_messages.push(json!({ "role": "assistant", "content": parts }));
                continue;
            }

            anthropic_messages.push(json!({
                "role": m.role,
                "content": m.content
            }));
        }

        let anthropic_tools: Vec<Value> = tools
            .iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": t.parameters
                })
            })
            .collect();

        let mut body = json!({
            "model": self.config.model,
            "max_tokens": 8192,
            "messages": anthropic_messages,
            "stream": true
        });

        if let Some(sys) = system_text {
            body["system"] = json!(sys);
        }

        if !anthropic_tools.is_empty() {
            body["tools"] = json!(anthropic_tools);
        }

        let req = self
            .client
            .post(url)
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body);
        let resp = self.post_with_retry(req, "ANTHROPIC_STREAM").await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err_body = resp.text().await.unwrap_or_default();
            return Err(format!("Anthropic API error {}: {}", status, err_body));
        }

        use futures_util::StreamExt;
        let mut stream = resp.bytes_stream();
        let mut buffer = String::new();
        let mut full_text = String::new();
        let mut tool_builders: std::collections::BTreeMap<usize, (String, String, String)> =
            std::collections::BTreeMap::new();
        let mut prompt_tokens = 100;
        let mut completion_tokens = 50;

        while let Some(chunk_res) = stream.next().await {
            let chunk = chunk_res.map_err(|e| format!("Stream read error: {}", e))?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(pos) = buffer.find('\n') {
                let line = buffer[..pos].trim().to_string();
                buffer.drain(..=pos);

                if line.is_empty() || line.starts_with(':') {
                    continue;
                }

                if let Some(payload) = line.strip_prefix("data:") {
                    let data = payload.trim();
                    if let Ok(val) = serde_json::from_str::<Value>(data) {
                        let event_type = val.get("type").and_then(|t| t.as_str()).unwrap_or("");
                        match event_type {
                            "content_block_start" => {
                                let idx =
                                    val.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;
                                if let Some(cb) = val.get("content_block") {
                                    if cb.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                                        let id = cb
                                            .get("id")
                                            .and_then(|i| i.as_str())
                                            .unwrap_or("")
                                            .to_string();
                                        let name = cb
                                            .get("name")
                                            .and_then(|n| n.as_str())
                                            .unwrap_or("")
                                            .to_string();
                                        tool_builders.insert(idx, (id, name, String::new()));
                                    }
                                }
                            }
                            "content_block_delta" => {
                                let idx =
                                    val.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;
                                if let Some(delta) = val.get("delta") {
                                    if delta.get("type").and_then(|t| t.as_str())
                                        == Some("text_delta")
                                    {
                                        if let Some(txt) =
                                            delta.get("text").and_then(|t| t.as_str())
                                        {
                                            full_text.push_str(txt);
                                            on_token(txt);
                                        }
                                    } else if delta.get("type").and_then(|t| t.as_str())
                                        == Some("input_json_delta")
                                    {
                                        if let Some(part) =
                                            delta.get("partial_json").and_then(|p| p.as_str())
                                        {
                                            if let Some(entry) = tool_builders.get_mut(&idx) {
                                                entry.2.push_str(part);
                                            }
                                        }
                                    }
                                }
                            }
                            "message_delta" => {
                                if let Some(usage) = val.get("usage") {
                                    if let Some(out) =
                                        usage.get("output_tokens").and_then(|t| t.as_u64())
                                    {
                                        completion_tokens = out as usize;
                                    }
                                }
                            }
                            "message_start" => {
                                if let Some(msg) = val.get("message") {
                                    if let Some(usage) = msg.get("usage") {
                                        if let Some(inp) =
                                            usage.get("input_tokens").and_then(|t| t.as_u64())
                                        {
                                            prompt_tokens = inp as usize;
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        let mut tool_calls = Vec::new();
        for (_, (id, name, args_str)) in tool_builders {
            let final_args: Value = serde_json::from_str(&args_str).unwrap_or(json!({}));
            tool_calls.push(ToolCall {
                id,
                name,
                arguments: final_args,
            });
        }

        Ok(LLMResponse {
            content: full_text,
            tool_calls: if tool_calls.is_empty() {
                None
            } else {
                Some(tool_calls)
            },
            model: self.config.model.clone(),
            prompt_tokens,
            completion_tokens,
        })
    }

    async fn complete_stream_openai_compatible(
        &self,
        messages: &[LLMMessage],
        tools: &[ToolDefinition],
        on_token: &mut (dyn FnMut(&str) + Send),
    ) -> Result<LLMResponse, String> {
        let base_url = self.config.base_url.clone().unwrap_or_else(|| {
            if self.config.provider == "powerbox" {
                "https://powerbox.atlasresearchlabs.online/v1".to_string()
            } else {
                "https://api.openai.com/v1".to_string()
            }
        });
        let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
        let api_key = self
            .config
            .api_key
            .clone()
            .or_else(|| {
                if self.config.provider == "powerbox" {
                    env::var("POWERBOX_API_KEY")
                        .or_else(|_| env::var("POWERBOX_MASTER_KEY"))
                        .ok()
                } else {
                    env::var("OPENAI_API_KEY").ok()
                }
            })
            .unwrap_or_else(|| "dummy".to_string());

        let mut formatted_msgs = Vec::new();
        for m in messages {
            if m.role == "tool" {
                let mut tool_obj = json!({
                    "role": "tool",
                    "tool_call_id": m.tool_call_id.clone().unwrap_or_else(|| "call_1".to_string()),
                    "content": m.content
                });
                if let Some(n) = &m.name {
                    tool_obj["name"] = json!(n);
                }
                formatted_msgs.push(tool_obj);
                continue;
            }

            if let Some(tool_calls) = &m.tool_calls {
                let calls: Vec<Value> = tool_calls.iter().map(|tc| {
                    json!({
                        "id": tc.id,
                        "type": "function",
                        "function": {
                            "name": tc.name,
                            "arguments": serde_json::to_string(&tc.arguments).unwrap_or_default()
                        }
                    })
                }).collect();
                formatted_msgs.push(json!({
                    "role": "assistant",
                    "content": if m.content.is_empty() { Value::Null } else { json!(m.content) },
                    "tool_calls": calls
                }));
                continue;
            }

            formatted_msgs.push(json!({
                "role": m.role,
                "content": m.content
            }));
        }

        let open_ai_tools: Vec<Value> = tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters
                    }
                })
            })
            .collect();

        let mut body = json!({
            "model": self.config.model,
            "messages": formatted_msgs,
            "temperature": self.config.temperature,
            "stream": true
        });

        if !open_ai_tools.is_empty() {
            body["tools"] = json!(open_ai_tools);
        }

        let req = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&body);
        let resp = self.post_with_retry(req, "OPENAI_STREAM").await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err_body = resp.text().await.unwrap_or_default();
            return Err(format!(
                "OpenAI/Powerbox API error {}: {}",
                status, err_body
            ));
        }

        use futures_util::StreamExt;
        let mut stream = resp.bytes_stream();
        let mut buffer = String::new();
        let mut full_text = String::new();
        let mut tool_builders: std::collections::BTreeMap<usize, (String, String, String)> =
            std::collections::BTreeMap::new();
        let mut prompt_tokens =
            (serde_json::to_string(&body).map(|s| s.len()).unwrap_or(100) / 4).max(1);
        let mut completion_tokens = 0;

        while let Some(chunk_res) = stream.next().await {
            let chunk = chunk_res.map_err(|e| format!("Stream read error: {}", e))?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(pos) = buffer.find('\n') {
                let line = buffer[..pos].trim().to_string();
                buffer.drain(..=pos);

                if line.is_empty() || line.starts_with(':') {
                    continue;
                }

                if let Some(payload) = line.strip_prefix("data:") {
                    let data = payload.trim();
                    if data == "[DONE]" {
                        break;
                    }

                    if let Ok(val) = serde_json::from_str::<Value>(data) {
                        if let Some(choices) = val.get("choices").and_then(|c| c.as_array()) {
                            if let Some(choice) = choices.first() {
                                if let Some(delta) = choice.get("delta") {
                                    if let Some(txt) = delta.get("content").and_then(|c| c.as_str())
                                    {
                                        if !txt.is_empty() {
                                            full_text.push_str(txt);
                                            on_token(txt);
                                        }
                                    }

                                    if let Some(tc_arr) =
                                        delta.get("tool_calls").and_then(|t| t.as_array())
                                    {
                                        for tc_delta in tc_arr {
                                            let idx = tc_delta
                                                .get("index")
                                                .and_then(|i| i.as_u64())
                                                .unwrap_or(0)
                                                as usize;
                                            let entry =
                                                tool_builders.entry(idx).or_insert_with(|| {
                                                    (String::new(), String::new(), String::new())
                                                });

                                            if let Some(id) =
                                                tc_delta.get("id").and_then(|i| i.as_str())
                                            {
                                                entry.0.push_str(id);
                                            }
                                            if let Some(func) = tc_delta.get("function") {
                                                if let Some(name) =
                                                    func.get("name").and_then(|n| n.as_str())
                                                {
                                                    entry.1.push_str(name);
                                                }
                                                if let Some(args) =
                                                    func.get("arguments").and_then(|a| a.as_str())
                                                {
                                                    entry.2.push_str(args);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        if let Some(usage) = val.get("usage") {
                            if let Some(pt) = usage.get("prompt_tokens").and_then(|t| t.as_u64()) {
                                prompt_tokens = pt as usize;
                            }
                            if let Some(ct) =
                                usage.get("completion_tokens").and_then(|t| t.as_u64())
                            {
                                completion_tokens = ct as usize;
                            }
                        }
                    }
                }
            }
        }

        let mut tool_calls = Vec::new();
        for (_, (id, name, args_str)) in tool_builders {
            let final_id = if id.is_empty() {
                format!("call_{}", Utc::now().timestamp_millis())
            } else {
                id
            };
            let final_args: Value = serde_json::from_str(&args_str).unwrap_or(json!({}));
            tool_calls.push(ToolCall {
                id: final_id,
                name,
                arguments: final_args,
            });
        }

        if completion_tokens == 0 {
            completion_tokens = (full_text.len() / 4).max(1);
        }

        Ok(LLMResponse {
            content: full_text,
            tool_calls: if tool_calls.is_empty() {
                None
            } else {
                Some(tool_calls)
            },
            model: self.config.model.clone(),
            prompt_tokens,
            completion_tokens,
        })
    }
}
