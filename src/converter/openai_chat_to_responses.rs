//! OpenAI Chat Completion → OpenAI Responses converter
//!
//! Converts between OpenAI Chat Completion API and Responses API formats.
//! Used by the `/c2r/` route prefix for Chat API client → Responses API provider proxying.

use bytes::Bytes;
use serde_json::{json, Value};

use super::{ApiConverter, ApiType, ConversionError, ConversionErrorKind};

/// Converter for OpenAI Chat ↔ Responses API
pub struct OpenAIChatToResponsesConverter;

impl ApiConverter for OpenAIChatToResponsesConverter {
    fn convert_request(&self, body: &Bytes, _target: ApiType) -> Result<Bytes, ConversionError> {
        let v: Value = serde_json::from_slice(body).map_err(|e| {
            ConversionError::new(ConversionErrorKind::InvalidJson, "invalid_json", e.to_string())
                .with_original_body(body.clone())
        })?;

        let mut resp = json!({});

        // model
        if let Some(model) = v.get("model") {
            resp["model"] = model.clone();
        }

        // messages → input + instructions
        if let Some(messages) = v.get("messages").and_then(|m| m.as_array()) {
            let mut instructions: Option<String> = None;
            let mut input_items = Vec::new();

            for msg in messages {
                let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("user");
                let content = msg.get("content");

                match role {
                    "system" => {
                        // system messages → instructions (concatenated)
                        if let Some(text) = content.and_then(|c| c.as_str()) {
                            match &mut instructions {
                                Some(existing) => {
                                    existing.push_str("\n\n");
                                    existing.push_str(text);
                                }
                                None => instructions = Some(text.to_string()),
                            }
                        }
                    }
                    "user" => {
                        if let Some(text) = content.and_then(|c| c.as_str()) {
                            input_items.push(json!({
                                "type": "message",
                                "role": "user",
                                "content": [{"type": "input_text", "text": text}]
                            }));
                        } else if let Some(arr) = content.and_then(|c| c.as_array()) {
                            // Array content (multimodal) — convert each part
                            let mut parts = Vec::new();
                            for part in arr {
                                if let Some(ptype) = part.get("type").and_then(|t| t.as_str()) {
                                    match ptype {
                                        "text" => {
                                            if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                                                parts.push(json!({"type": "input_text", "text": text}));
                                            }
                                        }
                                        "image_url" => {
                                            parts.push(json!({
                                                "type": "input_image",
                                                "image_url": part.get("url")
                                            }));
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            if !parts.is_empty() {
                                input_items.push(json!({
                                    "type": "message",
                                    "role": "user",
                                    "content": parts
                                }));
                            }
                        }
                    }
                    "assistant" => {
                        // assistant messages → output items
                        if let Some(text) = content.and_then(|c| c.as_str()) {
                            input_items.push(json!({
                                "type": "message",
                                "role": "assistant",
                                "content": [{"type": "output_text", "text": text}]
                            }));
                        }
                        // Handle tool_calls in assistant messages
                        if let Some(tool_calls) = msg.get("tool_calls").and_then(|tc| tc.as_array()) {
                            for tc in tool_calls {
                                let id = tc.get("id").and_then(|v| v.as_str()).unwrap_or("");
                                let func = tc.get("function");
                                let name = func.and_then(|f| f.get("name")).and_then(|n| n.as_str()).unwrap_or("");
                                let args = func.and_then(|f| f.get("arguments")).and_then(|a| a.as_str()).unwrap_or("{}");
                                input_items.push(json!({
                                    "type": "function_call",
                                    "id": id,
                                    "call_id": id,
                                    "name": name,
                                    "arguments": args
                                }));
                            }
                        }
                    }
                    "tool" => {
                        // tool messages → function_call_output items
                        let tool_call_id = msg.get("tool_call_id").and_then(|v| v.as_str()).unwrap_or("");
                        let content_str = content.and_then(|c| c.as_str()).unwrap_or("");
                        input_items.push(json!({
                            "type": "function_call_output",
                            "call_id": tool_call_id,
                            "output": content_str
                        }));
                    }
                    _ => {}
                }
            }

            if let Some(instr) = instructions {
                resp["instructions"] = json!(instr);
            }
            resp["input"] = json!(input_items);
        }

        // tools → tools (filter to function type only)
        if let Some(tools) = v.get("tools").and_then(|t| t.as_array()) {
            let resp_tools: Vec<Value> = tools.iter().filter_map(|tool| {
                let func = tool.get("function")?;
                Some(json!({
                    "type": "function",
                    "name": func.get("name")?,
                    "description": func.get("description").cloned().unwrap_or(json!("")),
                    "parameters": func.get("parameters").cloned().unwrap_or(json!({}))
                }))
            }).collect();
            if !resp_tools.is_empty() {
                resp["tools"] = json!(resp_tools);
            }
        }

        // tool_choice → tool_choice
        if let Some(tc) = v.get("tool_choice") {
            if let Some(tc_str) = tc.as_str() {
                match tc_str {
                    "auto" => resp["tool_choice"] = json!("auto"),
                    "required" => resp["tool_choice"] = json!("required"),
                    "none" => resp["tool_choice"] = json!("none"),
                    _ => resp["tool_choice"] = json!("auto"),
                }
            } else if let Some(tc_obj) = tc.as_object() {
                // {"type": "function", "function": {"name": "..."}} → {"type": "function", "name": "..."}
                if tc_obj.get("type").and_then(|t| t.as_str()) == Some("function") {
                    if let Some(name) = tc_obj.get("function").and_then(|f| f.get("name")).and_then(|n| n.as_str()) {
                        resp["tool_choice"] = json!({"type": "function", "name": name});
                    }
                }
            }
        }

        // max_tokens → max_output_tokens
        if let Some(max_tokens) = v.get("max_tokens").or(v.get("max_completion_tokens")) {
            resp["max_output_tokens"] = max_tokens.clone();
        }

        // stream — pass through
        if let Some(stream) = v.get("stream") {
            resp["stream"] = stream.clone();
        }

        // temperature, top_p — pass through
        if let Some(temp) = v.get("temperature") {
            resp["temperature"] = temp.clone();
        }
        if let Some(top_p) = v.get("top_p") {
            resp["top_p"] = top_p.clone();
        }

        // stop → stop_sequences (Responses API uses stop_sequences for text stop)
        if let Some(stop) = v.get("stop") {
            resp["stop_sequences"] = stop.clone();
        }

        // user → metadata.user_id? No — Responses API doesn't have a direct user field.
        // Drop it silently.

        serde_json::to_vec(&resp).map(Bytes::from).map_err(|e| {
            ConversionError::new(ConversionErrorKind::InvalidJson, "serialization_error", e.to_string())
        })
    }

    fn convert_response(&self, body: &Bytes, _target: ApiType, _source: ApiType, _is_stream: bool) -> Result<Bytes, ConversionError> {
        let v: Value = serde_json::from_slice(body).map_err(|e| {
            ConversionError::new(ConversionErrorKind::InvalidJson, "invalid_json", e.to_string())
                .with_original_body(body.clone())
        })?;

        let mut chat = json!({});

        // id
        if let Some(id) = v.get("id") {
            chat["id"] = id.clone();
        }

        // object
        chat["object"] = json!("chat.completion");

        // created_at → created (Responses uses created_at, Chat uses created)
        if let Some(created_at) = v.get("created_at") {
            chat["created"] = created_at.clone();
        }

        // model
        if let Some(model) = v.get("model") {
            chat["model"] = model.clone();
        }

        // output → choices
        if let Some(output) = v.get("output").and_then(|o| o.as_array()) {
            let mut choices = Vec::new();

            for (idx, item) in output.iter().enumerate() {
                let item_type = item.get("type").and_then(|t| t.as_str()).unwrap_or("message");

                if item_type == "message" {
                    // Extract text content
                    let content = item.get("content").and_then(|c| c.as_array());
                    let mut text = String::new();
                    if let Some(content_arr) = content {
                        for part in content_arr {
                            if part.get("type").and_then(|t| t.as_str()) == Some("output_text") {
                                if let Some(t) = part.get("text").and_then(|t| t.as_str()) {
                                    text.push_str(t);
                                }
                            }
                        }
                    }

                    let stop_reason = item.get("stop_reason").and_then(|s| s.as_str()).unwrap_or("end_turn");
                    let finish_reason = map_stop_reason(stop_reason);

                    choices.push(json!({
                        "index": idx,
                        "message": {
                            "role": "assistant",
                            "content": if text.is_empty() { Value::Null } else { json!(text) }
                        },
                        "finish_reason": finish_reason
                    }));
                } else if item_type == "function_call" {
                    // function_call → tool_calls in a choice
                    let id = item.get("call_id").or(item.get("id")).and_then(|v| v.as_str()).unwrap_or("");
                    let name = item.get("name").and_then(|n| n.as_str()).unwrap_or("");
                    let args = item.get("arguments").and_then(|a| a.as_str()).unwrap_or("{}");

                    choices.push(json!({
                        "index": idx,
                        "message": {
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
                        },
                        "finish_reason": "tool_calls"
                    }));
                }
            }

            if choices.is_empty() {
                choices.push(json!({
                    "index": 0,
                    "message": {"role": "assistant", "content": ""},
                    "finish_reason": "stop"
                }));
            }

            chat["choices"] = json!(choices);
        } else {
            chat["choices"] = json!([{
                "index": 0,
                "message": {"role": "assistant", "content": ""},
                "finish_reason": "stop"
            }]);
        }

        // usage — convert input_tokens/output_tokens → prompt_tokens/completion_tokens/total_tokens
        if let Some(usage) = v.get("usage") {
            let input = usage.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
            let output = usage.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
            chat["usage"] = json!({
                "prompt_tokens": input,
                "completion_tokens": output,
                "total_tokens": input + output
            });
        }

        serde_json::to_vec(&chat).map(Bytes::from).map_err(|e| {
            ConversionError::new(ConversionErrorKind::InvalidJson, "serialization_error", e.to_string())
        })
    }
}

fn map_stop_reason(reason: &str) -> &'static str {
    match reason {
        "end_turn" => "stop",
        "max_tokens" => "length",
        "tool_use" => "tool_calls",
        "stop_sequence" => "stop",
        "content_filter" => "content_filter",
        _ => "stop",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_simple_request() {
        let req = r#"{"model":"gpt-4o","messages":[{"role":"user","content":"Hello"}]}"#;
        let result = OpenAIChatToResponsesConverter
            .convert_request(&Bytes::from(req), ApiType::OpenAIResponses)
            .unwrap();
        let v: Value = serde_json::from_slice(&result).unwrap();
        assert_eq!(v["model"], "gpt-4o");
        assert!(v["input"].is_array());
        assert_eq!(v["input"][0]["type"], "message");
        assert_eq!(v["input"][0]["role"], "user");
    }

    #[test]
    fn convert_system_to_instructions() {
        let req = r#"{"model":"gpt-4o","messages":[{"role":"system","content":"Be helpful."},{"role":"user","content":"Hi"}]}"#;
        let result = OpenAIChatToResponsesConverter
            .convert_request(&Bytes::from(req), ApiType::OpenAIResponses)
            .unwrap();
        let v: Value = serde_json::from_slice(&result).unwrap();
        assert_eq!(v["instructions"], "Be helpful.");
    }

    #[test]
    fn convert_tool_calls_to_function_call() {
        let req = r#"{"model":"gpt-4o","messages":[{"role":"assistant","content":null,"tool_calls":[{"id":"call_1","type":"function","function":{"name":"get_weather","arguments":"{\"loc\":\"SF\"}"}}]}]}"#;
        let result = OpenAIChatToResponsesConverter
            .convert_request(&Bytes::from(req), ApiType::OpenAIResponses)
            .unwrap();
        let v: Value = serde_json::from_slice(&result).unwrap();
        assert_eq!(v["input"][0]["type"], "function_call");
        assert_eq!(v["input"][0]["name"], "get_weather");
    }

    #[test]
    fn convert_tool_message_to_function_call_output() {
        let req = r#"{"model":"gpt-4o","messages":[{"role":"tool","tool_call_id":"call_1","content":"sunny"}]}"#;
        let result = OpenAIChatToResponsesConverter
            .convert_request(&Bytes::from(req), ApiType::OpenAIResponses)
            .unwrap();
        let v: Value = serde_json::from_slice(&result).unwrap();
        assert_eq!(v["input"][0]["type"], "function_call_output");
        assert_eq!(v["input"][0]["call_id"], "call_1");
    }

    #[test]
    fn convert_tools_format() {
        let req = r#"{"model":"gpt-4o","messages":[],"tools":[{"type":"function","function":{"name":"fn","description":"A function","parameters":{"type":"object","properties":{}}}}]}"#;
        let result = OpenAIChatToResponsesConverter
            .convert_request(&Bytes::from(req), ApiType::OpenAIResponses)
            .unwrap();
        let v: Value = serde_json::from_slice(&result).unwrap();
        assert_eq!(v["tools"][0]["type"], "function");
        assert_eq!(v["tools"][0]["name"], "fn");
    }

    #[test]
    fn convert_tool_choice_required() {
        let req = r#"{"model":"gpt-4o","messages":[],"tool_choice":"required"}"#;
        let result = OpenAIChatToResponsesConverter
            .convert_request(&Bytes::from(req), ApiType::OpenAIResponses)
            .unwrap();
        let v: Value = serde_json::from_slice(&result).unwrap();
        assert_eq!(v["tool_choice"], "required");
    }

    #[test]
    fn convert_response_simple() {
        let resp = r#"{"id":"resp-1","object":"response","created_at":1234567890,"model":"gpt-4o","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"Hello!"}],"stop_reason":"end_turn"}],"usage":{"input_tokens":10,"output_tokens":5,"total_tokens":15}}"#;
        let result = OpenAIChatToResponsesConverter
            .convert_response(&Bytes::from(resp), ApiType::OpenAIChat, ApiType::OpenAIResponses, false)
            .unwrap();
        let v: Value = serde_json::from_slice(&result).unwrap();
        assert_eq!(v["object"], "chat.completion");
        assert_eq!(v["choices"][0]["message"]["content"], "Hello!");
        assert_eq!(v["choices"][0]["finish_reason"], "stop");
        assert_eq!(v["usage"]["prompt_tokens"], 10);
        assert_eq!(v["usage"]["total_tokens"], 15);
    }

    #[test]
    fn convert_response_tool_use() {
        let resp = r#"{"id":"resp-1","object":"response","model":"gpt-4o","output":[{"type":"function_call","id":"call_1","call_id":"call_1","name":"get_weather","arguments":"{\"loc\":\"SF\"}","status":"completed"}],"usage":{"input_tokens":10,"output_tokens":5}}"#;
        let result = OpenAIChatToResponsesConverter
            .convert_response(&Bytes::from(resp), ApiType::OpenAIChat, ApiType::OpenAIResponses, false)
            .unwrap();
        let v: Value = serde_json::from_slice(&result).unwrap();
        assert_eq!(v["choices"][0]["finish_reason"], "tool_calls");
        assert_eq!(v["choices"][0]["message"]["tool_calls"][0]["function"]["name"], "get_weather");
    }
}
