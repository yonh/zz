//! OpenAI Responses → OpenAI Chat Completion converter
//!
//! Converts between OpenAI Responses API and Chat Completion API formats.
//! Used by the `/r2c/` route prefix for Codex → Chat API provider proxying.

use bytes::Bytes;
use serde_json::{json, Value};

use super::{ApiConverter, ApiType, ConversionError, ConversionErrorKind};

/// Converter for OpenAI Responses ↔ Chat Completion API
pub struct OpenAIResponsesToChatConverter;

impl ApiConverter for OpenAIResponsesToChatConverter {
    fn convert_request(&self, body: &Bytes, _target: ApiType) -> Result<Bytes, ConversionError> {
        let v: Value = serde_json::from_slice(body).map_err(|e| {
            ConversionError::new(ConversionErrorKind::InvalidJson, "invalid_json", e.to_string())
                .with_original_body(body.clone())
        })?;

        let mut chat = json!({});

        // model
        if let Some(model) = v.get("model") {
            chat["model"] = model.clone();
        }

        // input → messages
        if let Some(input) = v.get("input") {
            let mut messages = Vec::new();

            // instructions → system message (inserted first)
            if let Some(instructions) = v.get("instructions") {
                if let Some(s) = instructions.as_str() {
                    messages.push(json!({"role": "system", "content": s}));
                }
            }

            if let Some(s) = input.as_str() {
                // Simple string input → user message
                messages.push(json!({"role": "user", "content": s}));
            } else if let Some(arr) = input.as_array() {
                // Group consecutive function_call items into a single assistant message
                let mut pending_tool_calls: Vec<Value> = Vec::new();

                for item in arr {
                    let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("message");

                    if item_type == "function_call" {
                        // Buffer tool calls
                        let id = item.get("call_id").or(item.get("id")).and_then(|v| v.as_str()).unwrap_or("");
                        let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("");
                        let args = item.get("arguments").and_then(|v| v.as_str()).unwrap_or("{}");
                        pending_tool_calls.push(json!({
                            "id": id,
                            "type": "function",
                            "function": {
                                "name": name,
                                "arguments": args
                            }
                        }));
                    } else {
                        // Flush pending tool_calls as a single assistant message
                        if !pending_tool_calls.is_empty() {
                            messages.push(json!({
                                "role": "assistant",
                                "content": null,
                                "tool_calls": pending_tool_calls.drain(..).collect::<Vec<_>>()
                            }));
                        }
                        let msg = convert_input_item(item)?;
                        messages.push(msg);
                    }
                }

                // Flush any remaining tool_calls
                if !pending_tool_calls.is_empty() {
                    messages.push(json!({
                        "role": "assistant",
                        "content": null,
                        "tool_calls": pending_tool_calls
                    }));
                }
            }

            chat["messages"] = Value::Array(messages);
        }

        // max_output_tokens → max_tokens
        if let Some(max) = v.get("max_output_tokens") {
            chat["max_tokens"] = max.clone();
        }

        // passthrough: temperature, top_p, stop, metadata
        for field in &["temperature", "top_p", "stop", "metadata"] {
            if let Some(val) = v.get(*field) {
                chat[*field] = val.clone();
            }
        }

        // tools + tool_choice
        // Chat API only supports type:"function" tools. Filter out unsupported types
        // (e.g. "namespace", "web_search") that the Responses API may include.
        if let Some(tools) = v.get("tools") {
            if let Some(arr) = tools.as_array() {
                let converted: Vec<Value> = arr.iter().filter_map(|t| {
                    if t.get("type").and_then(|v| v.as_str()) == Some("function") {
                        // Responses format: {type:"function", name, description, parameters}
                        // Chat format: {type:"function", function:{name, description, parameters}}
                        Some(json!({
                            "type": "function",
                            "function": {
                                "name": t.get("name"),
                                "description": t.get("description"),
                                "parameters": t.get("parameters")
                            }
                        }))
                    } else {
                        // Skip non-function tools (namespace, web_search, etc.)
                        None
                    }
                }).collect();
                chat["tools"] = Value::Array(converted);
            }
        }
        if let Some(tc) = v.get("tool_choice") {
            // "any" in Responses == "required" in Chat
            if tc.as_str() == Some("any") {
                chat["tool_choice"] = json!("required");
            } else {
                chat["tool_choice"] = tc.clone();
            }
        }

        // stream: pass through so upstream returns SSE chunks
        if let Some(stream) = v.get("stream") {
            chat["stream"] = stream.clone();
        }

        // Drop: store, previous_response_id (zz is stateless)

        Ok(Bytes::from(serde_json::to_vec(&chat).unwrap()))
    }

    fn convert_response(
        &self,
        body: &Bytes,
        _source: ApiType,
        _target: ApiType,
        _is_stream: bool,
    ) -> Result<Bytes, ConversionError> {
        let v: Value = serde_json::from_slice(body).map_err(|e| {
            ConversionError::new(ConversionErrorKind::InvalidJson, "invalid_json", e.to_string())
                .with_original_body(body.clone())
        })?;

        // Check if this is an error response
        if let Some(error) = v.get("error") {
            // Chat API error format → pass through
            return Ok(Bytes::from(serde_json::to_vec(&json!({
                "type": "error",
                "error": {
                    "type": error.get("type").cloned().unwrap_or(json!("invalid_request_error")),
                    "message": error.get("message").cloned().unwrap_or(json!("Unknown error"))
                }
            })).unwrap()));
        }

        // choices[0].message → output[0] (message type)
        let mut output = Vec::new();

        if let Some(choices) = v.get("choices").and_then(|c| c.as_array()) {
            for choice in choices {
                if let Some(message) = choice.get("message") {
                    let mut content_items = Vec::new();

                    // text content
                    if let Some(text) = message.get("content").and_then(|c| c.as_str()) {
                        content_items.push(json!({
                            "type": "output_text",
                            "text": text,
                            "annotations": []
                        }));
                    }

                    // tool_calls → function_call output items
                    let has_tool_calls = message.get("tool_calls")
                        .and_then(|tc| tc.as_array())
                        .map(|arr| !arr.is_empty())
                        .unwrap_or(false);
                    if let Some(tool_calls) = message.get("tool_calls").and_then(|tc| tc.as_array()) {
                        for tc in tool_calls {
                            let func = tc.get("function");
                            output.push(json!({
                                "type": "function_call",
                                "id": tc.get("id"),
                                "call_id": tc.get("id"),
                                "name": func.and_then(|f| f.get("name")),
                                "arguments": func.and_then(|f| f.get("arguments")).and_then(|a| a.as_str()).unwrap_or("{}")
                            }));
                        }
                    }

                    // message output item — emit even when content is null
                    // if tool_calls are present (common pattern)
                    if !content_items.is_empty() || has_tool_calls {
                        let stop_reason = map_finish_reason(
                            choice.get("finish_reason").and_then(|f| f.as_str())
                        );
                        output.push(json!({
                            "type": "message",
                            "id": format!("msg_{}", v.get("id").and_then(|id| id.as_str()).unwrap_or("gen")),
                            "role": "assistant",
                            "content": content_items,
                            "stop_reason": stop_reason
                        }));
                    }
                }
            }
        }

        // usage mapping
        let usage = if let Some(u) = v.get("usage") {
            let input = u.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
            let output = u.get("completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
            json!({
                "input_tokens": input,
                "output_tokens": output,
                "total_tokens": input + output
            })
        } else {
            json!({"input_tokens": 0, "output_tokens": 0, "total_tokens": 0})
        };

        let resp = json!({
            "id": format!("resp_{}", v.get("id").and_then(|id| id.as_str()).unwrap_or("gen")),
            "object": "response",
            "created": v.get("created").cloned().unwrap_or(json!(0)),
            "model": v.get("model").cloned().unwrap_or(json!("unknown")),
            "output": output,
            "usage": usage
        });

        Ok(Bytes::from(serde_json::to_vec(&resp).unwrap()))
    }
}

/// Convert an input_item from Responses API to a Chat message
fn convert_input_item(item: &Value) -> Result<Value, ConversionError> {
    let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("message");

    match item_type {
        "message" => {
            let role = item.get("role").and_then(|v| v.as_str()).unwrap_or("user");
            // Responses API "developer" role == Chat API "system" role
            let chat_role = if role == "developer" { "system" } else { role };
            let content = extract_content(item);
            Ok(json!({"role": chat_role, "content": content}))
        }
        "function_call" => {
            // Responses function_call → Chat assistant message with tool_calls
            let id = item.get("call_id").or(item.get("id")).and_then(|v| v.as_str()).unwrap_or("");
            let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let args = item.get("arguments").and_then(|v| v.as_str()).unwrap_or("{}");
            Ok(json!({
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": id,
                    "type": "function",
                    "function": {
                        "name": name,
                        "arguments": args
                    }
                }]
            }))
        }
        "function_call_output" => {
            // Responses function_call_output → Chat tool message
            let call_id = item.get("call_id").and_then(|v| v.as_str()).unwrap_or("");
            let output = item.get("output").and_then(|v| v.as_str()).unwrap_or("");
            Ok(json!({
                "role": "tool",
                "tool_call_id": call_id,
                "content": output
            }))
        }
        _ => {
            // Unknown types — skip gracefully instead of erroring
            tracing::warn!(item_type = item_type, "Skipping unknown input_item type");
            Err(ConversionError::new(
                ConversionErrorKind::UnsupportedFeature,
                "unsupported_input_item_type",
                format!("Unsupported input_item type: {}", item_type),
            ))
        }
    }
}

/// Extract content from a Responses message item.
/// Returns a Chat-compatible content value (string or array).
fn extract_content(item: &Value) -> Value {
    if let Some(content) = item.get("content") {
        if let Some(s) = content.as_str() {
            return json!(s);
        }
        if let Some(arr) = content.as_array() {
            // Check if there are any non-text items (images, etc.)
            let has_media = arr.iter().any(|c| {
                matches!(c.get("type").and_then(|v| v.as_str()), Some("input_image"))
            });

            if has_media {
                // Build multimodal content array for Chat API
                let parts: Vec<Value> = arr.iter().filter_map(|c| {
                    let ctype = c.get("type").and_then(|v| v.as_str()).unwrap_or("input_text");
                    match ctype {
                        "input_text" | "output_text" => {
                            c.get("text").and_then(|t| t.as_str()).map(|text| {
                                json!({"type": "text", "text": text})
                            })
                        }
                        "input_image" => {
                            // Responses: {"type": "input_image", "image_url": "https://..."}
                            // Chat: {"type": "image_url", "image_url": {"url": "https://..."}}
                            let url = c.get("image_url").and_then(|u| u.as_str())
                                .or_else(|| c.get("image_url").and_then(|u| u.get("url")).and_then(|u| u.as_str()))
                                .unwrap_or("");
                            Some(json!({
                                "type": "image_url",
                                "image_url": {"url": url}
                            }))
                        }
                        _ => None,
                    }
                }).collect();
                if !parts.is_empty() {
                    return json!(parts);
                }
            } else {
                // Text-only — concatenate as string
                let text: String = arr.iter().filter_map(|c| {
                    if matches!(c.get("type").and_then(|v| v.as_str()), Some("input_text") | Some("output_text")) {
                        c.get("text").and_then(|t| t.as_str())
                    } else {
                        None
                    }
                }).collect();
                return json!(text);
            }
        }
    }
    json!("")
}

/// Map Chat API finish_reason to Responses API stop_reason
fn map_finish_reason(reason: Option<&str>) -> &'static str {
    match reason {
        Some("stop") => "end_turn",
        Some("length") => "max_tokens",
        Some("tool_calls") => "tool_use",
        Some("content_filter") => "content_filter",
        _ => "end_turn",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_simple_string_input() {
        let req = r#"{"input":"Hello","model":"gpt-4o"}"#;
        let result = OpenAIResponsesToChatConverter.convert_request(
            &Bytes::from(req), ApiType::OpenAIChat
        ).unwrap();
        let chat: Value = serde_json::from_slice(&result).unwrap();
        assert_eq!(chat["messages"][0]["role"], "user");
        assert_eq!(chat["messages"][0]["content"], "Hello");
        assert_eq!(chat["model"], "gpt-4o");
    }

    #[test]
    fn convert_instructions_to_system() {
        let req = r#"{"input":"Hi","model":"gpt-4o","instructions":"Be concise."}"#;
        let result = OpenAIResponsesToChatConverter.convert_request(
            &Bytes::from(req), ApiType::OpenAIChat
        ).unwrap();
        let chat: Value = serde_json::from_slice(&result).unwrap();
        assert_eq!(chat["messages"][0]["role"], "system");
        assert_eq!(chat["messages"][0]["content"], "Be concise.");
        assert_eq!(chat["messages"][1]["role"], "user");
    }

    #[test]
    fn convert_max_output_tokens() {
        let req = r#"{"input":"Test","model":"gpt-4o","max_output_tokens":200}"#;
        let result = OpenAIResponsesToChatConverter.convert_request(
            &Bytes::from(req), ApiType::OpenAIChat
        ).unwrap();
        let chat: Value = serde_json::from_slice(&result).unwrap();
        assert_eq!(chat["max_tokens"], 200);
    }

    #[test]
    fn convert_tools_mapping() {
        let req = r#"{"input":"Weather?","model":"gpt-4o","tools":[{"type":"function","name":"get_weather","description":"Get weather","parameters":{"type":"object"}}],"tool_choice":"auto"}"#;
        let result = OpenAIResponsesToChatConverter.convert_request(
            &Bytes::from(req), ApiType::OpenAIChat
        ).unwrap();
        let chat: Value = serde_json::from_slice(&result).unwrap();
        assert_eq!(chat["tools"][0]["function"]["name"], "get_weather");
        assert_eq!(chat["tool_choice"], "auto");
    }

    #[test]
    fn convert_tool_choice_any_to_required() {
        let req = r#"{"input":"Hi","model":"gpt-4o","tool_choice":"any"}"#;
        let result = OpenAIResponsesToChatConverter.convert_request(
            &Bytes::from(req), ApiType::OpenAIChat
        ).unwrap();
        let chat: Value = serde_json::from_slice(&result).unwrap();
        assert_eq!(chat["tool_choice"], "required");
    }

    #[test]
    fn drop_store_and_previous_response_id() {
        let req = r#"{"input":"Hi","model":"gpt-4o","store":true,"previous_response_id":"resp_abc"}"#;
        let result = OpenAIResponsesToChatConverter.convert_request(
            &Bytes::from(req), ApiType::OpenAIChat
        ).unwrap();
        let chat: Value = serde_json::from_slice(&result).unwrap();
        assert!(chat.get("store").is_none());
        assert!(chat.get("previous_response_id").is_none());
    }

    #[test]
    fn passthrough_params() {
        let req = r#"{"input":"Hi","model":"gpt-4o","temperature":0.5,"top_p":0.9,"stop":["END"],"metadata":{"user_id":"abc"}}"#;
        let result = OpenAIResponsesToChatConverter.convert_request(
            &Bytes::from(req), ApiType::OpenAIChat
        ).unwrap();
        let chat: Value = serde_json::from_slice(&result).unwrap();
        assert_eq!(chat["temperature"], 0.5);
        assert_eq!(chat["top_p"], 0.9);
        assert_eq!(chat["stop"][0], "END");
        assert_eq!(chat["metadata"]["user_id"], "abc");
    }

    #[test]
    fn preserves_stream() {
        let req = r#"{"input":"Hi","model":"gpt-4o","stream":true}"#;
        let result = OpenAIResponsesToChatConverter.convert_request(
            &Bytes::from(req), ApiType::OpenAIChat
        ).unwrap();
        let chat: Value = serde_json::from_slice(&result).unwrap();
        assert_eq!(chat["stream"], true);
    }

    #[test]
    fn convert_simple_text_response() {
        let resp = r#"{"id":"chatcmpl-abc","object":"chat.completion","created":1748332800,"model":"gpt-4o","choices":[{"index":0,"message":{"role":"assistant","content":"Hi!"},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":2}}"#;
        let result = OpenAIResponsesToChatConverter.convert_response(
            &Bytes::from(resp), ApiType::OpenAIChat, ApiType::OpenAIResponses, false
        ).unwrap();
        let r: Value = serde_json::from_slice(&result).unwrap();
        assert_eq!(r["object"], "response");
        assert!(r["id"].as_str().unwrap().starts_with("resp_"));
        assert_eq!(r["output"][0]["type"], "message");
        assert_eq!(r["output"][0]["content"][0]["type"], "output_text");
        assert_eq!(r["output"][0]["content"][0]["text"], "Hi!");
        assert_eq!(r["usage"]["input_tokens"], 10);
        assert_eq!(r["usage"]["output_tokens"], 2);
    }

    #[test]
    fn convert_tool_call_response() {
        let resp = r#"{"id":"chatcmpl-abc","object":"chat.completion","created":1748332800,"model":"gpt-4o","choices":[{"index":0,"message":{"role":"assistant","content":null,"tool_calls":[{"id":"call_1","type":"function","function":{"name":"get_weather","arguments":"{\"location\":\"Tokyo\"}"}}]},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":50,"completion_tokens":10}}"#;
        let result = OpenAIResponsesToChatConverter.convert_response(
            &Bytes::from(resp), ApiType::OpenAIChat, ApiType::OpenAIResponses, false
        ).unwrap();
        let r: Value = serde_json::from_slice(&result).unwrap();
        let has_fc = r["output"].as_array().unwrap().iter().any(|o| o["type"] == "function_call");
        assert!(has_fc);
    }

    #[test]
    fn filters_non_function_tools() {
        let req = r#"{"input":"Hi","model":"gpt-4o","tools":[{"type":"function","name":"do_thing","description":"Does a thing","parameters":{"type":"object"}},{"type":"namespace","name":"multi_agent_v1","tools":[{"type":"function","name":"spawn","description":"Spawn agent","parameters":{}}]},{"type":"web_search","external_web_access":false}]}"#;
        let result = OpenAIResponsesToChatConverter.convert_request(
            &Bytes::from(req), ApiType::OpenAIChat
        ).unwrap();
        let chat: Value = serde_json::from_slice(&result).unwrap();
        let tools = chat["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 1, "Only function tools should be kept");
        assert_eq!(tools[0]["function"]["name"], "do_thing");
    }

    #[test]
    fn finish_reason_mapping() {
        assert_eq!(map_finish_reason(Some("stop")), "end_turn");
        assert_eq!(map_finish_reason(Some("length")), "max_tokens");
        assert_eq!(map_finish_reason(Some("tool_calls")), "tool_use");
    }
}
