//! Atlas Daedalus — Anthropic Claude Provider Implementation

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
        "model": config.model,
        "max_tokens": 8192,
        "messages": anthropic_messages
    });

    if let Some(sys) = system_text {
        body["system"] = json!(sys);
    }

    if !anthropic_tools.is_empty() {
        body["tools"] = json!(anthropic_tools);
    }

    let req = client
        .post(url)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&body);
    let resp = post_with_retry(req, "ANTHROPIC").await?;

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
        "model": config.model,
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

    let req = client
        .post(url)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&body);
    let resp = post_with_retry(req, "ANTHROPIC_STREAM").await?;

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
                                if delta.get("type").and_then(|t| t.as_str()) == Some("text_delta")
                                {
                                    if let Some(txt) = delta.get("text").and_then(|t| t.as_str()) {
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
        model: config.model.clone(),
        prompt_tokens,
        completion_tokens,
    })
}
