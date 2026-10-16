//! Atlas Daedalus — OpenAI & Compatible Provider Implementation (OpenAI, Powerbox, etc.)

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
    let base_url = config.base_url.clone().unwrap_or_else(|| {
        if config.provider == "powerbox" {
            "https://powerbox.atlasresearchlabs.online/v1".to_string()
        } else {
            "https://api.openai.com/v1".to_string()
        }
    });
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let api_key = config
        .api_key
        .clone()
        .or_else(|| {
            if config.provider == "powerbox" {
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
        "model": config.model,
        "messages": formatted_msgs,
        "temperature": config.temperature
    });

    if !openai_tools.is_empty() {
        body["tools"] = json!(openai_tools);
    }

    let req = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&body);
    let resp = post_with_retry(req, &config.provider.to_uppercase()).await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let err_body = resp.text().await.unwrap_or_default();
        return Err(format!(
            "{} API error {}: {}",
            config.provider.to_uppercase(),
            status,
            err_body
        ));
    }

    let json_resp: Value = resp
        .json()
        .await
        .map_err(|e| format!("{} parse error: {}", config.provider.to_uppercase(), e))?;

    let choice = json_resp.get("choices").and_then(|c| c.get(0));
    let message = choice.and_then(|c| c.get("message"));
    let text = message
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string();

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
    let base_url = config.base_url.clone().unwrap_or_else(|| {
        if config.provider == "powerbox" {
            "https://powerbox.atlasresearchlabs.online/v1".to_string()
        } else {
            "https://api.openai.com/v1".to_string()
        }
    });
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let api_key = config
        .api_key
        .clone()
        .or_else(|| {
            if config.provider == "powerbox" {
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
                            "arguments": serde_json::to_string(&tc.arguments).unwrap_or_default()
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
        "model": config.model,
        "messages": formatted_msgs,
        "temperature": config.temperature,
        "stream": true
    });

    if !open_ai_tools.is_empty() {
        body["tools"] = json!(open_ai_tools);
    }

    let req = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&body);
    let resp = post_with_retry(req, "OPENAI_STREAM").await?;

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
                                if let Some(txt) = delta.get("content").and_then(|c| c.as_str()) {
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
                                        let entry = tool_builders.entry(idx).or_insert_with(|| {
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
                        if let Some(ct) = usage.get("completion_tokens").and_then(|t| t.as_u64()) {
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
        model: config.model.clone(),
        prompt_tokens,
        completion_tokens,
    })
}
