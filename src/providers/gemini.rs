//! Atlas Daedalus — Google Gemini Provider Implementation

use chrono::Utc;
use reqwest::Client;
use serde_json::{json, Value};
use std::env;

use crate::providers::router::post_with_retry;
use crate::providers::types::{LLMMessage, LLMResponse, ProviderConfig, ToolCall};
use crate::tools::registry::ToolDefinition;

pub async fn complete(
    client: &Client,
    config: &ProviderConfig,
    messages: &[LLMMessage],
    tools: &[ToolDefinition],
) -> Result<LLMResponse, String> {
    let api_key = config
        .api_key
        .clone()
        .or_else(|| env::var("GEMINI_API_KEY").ok())
        .unwrap_or_default();
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
        config.model
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

    let req = client
        .post(&url)
        .header("x-goog-api-key", &api_key)
        .json(&body);
    let mut resp = post_with_retry(req, "GEMINI").await?;

    if resp.status().as_u16() == 429 && config.model != "gemini-2.0-flash" {
        let fallback_url = "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent";
        println!(
            "\n{}[ACODE] Active model '{}' reached rate limit (429). Automatically falling back to gemini-2.0-flash (1,500 RPD)...{}\n",
            crate::cli::tui::TuiRenderer::YELLOW,
            config.model,
            crate::cli::tui::TuiRenderer::RESET
        );
        let fb_req = client
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
                config.model
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

    let prompt_tokens = (serde_json::to_string(&body).map(|s| s.len()).unwrap_or(100) / 4).max(1);
    let completion_tokens = (text.len() / 4).max(1);

    Ok(LLMResponse {
        content: text,
        tool_calls: if tool_calls.is_empty() {
            None
        } else {
            Some(tool_calls)
        },
        model: config.model.clone(),
        prompt_tokens,
        completion_tokens,
    })
}

pub async fn complete_stream(
    client: &Client,
    config: &ProviderConfig,
    messages: &[LLMMessage],
    tools: &[ToolDefinition],
    on_token: &mut (dyn FnMut(&str) + Send),
) -> Result<LLMResponse, String> {
    let api_key = config
        .api_key
        .clone()
        .or_else(|| env::var("GEMINI_API_KEY").ok())
        .unwrap_or_default();
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse",
        config.model
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

    let req = client
        .post(&url)
        .header("x-goog-api-key", &api_key)
        .json(&body);
    let resp = post_with_retry(req, "GEMINI_STREAM").await?;

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
                                    if let Some(txt) = part.get("text").and_then(|t| t.as_str()) {
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
                                            id: format!("call_{}", Utc::now().timestamp_millis()),
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

    let prompt_tokens = (serde_json::to_string(&body).map(|s| s.len()).unwrap_or(100) / 4).max(1);
    let completion_tokens = (full_text.len() / 4).max(1);

    Ok(LLMResponse {
        content: full_text,
        tool_calls: if tool_calls.is_empty() {
            None
        } else {
            Some(tool_calls)
        },
        model: config.model.clone(),
        prompt_tokens,
        completion_tokens,
    })
}
