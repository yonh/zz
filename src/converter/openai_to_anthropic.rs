//! OpenAI Chat → Anthropic response body converter
//!
//! Implements conversion from OpenAI Chat Completions format to Anthropic Messages format.
//! See [field-mapping.md §3](../../../docs/plans/2026-05-04-api-converter-plan/field-mapping.md#3-响应体openai-chat--anthropic-o2a)
//! for detailed mapping rules.

use crate::converter::{
    ApiConverter, ApiType, Bytes, ConversionError, ConversionErrorKind, NoopTelemetry,
    TelemetryContext,
};
use serde_json::{json, Map, Value};

/// Known OpenAI request fields for telemetry
const KNOWN_OPENAI_REQUEST_FIELDS: &[&str] = &[
    "model",
    "messages",
    "temperature",
    "top_p",
    "n",
    "max_tokens",
    "max_completion_tokens",
    "stream",
    "stop",
    "tools",
    "tool_choice",
    "user",
    "parallel_tool_calls",
];

/// Known OpenAI response fields for telemetry
const KNOWN_OPENAI_RESPONSE_FIELDS: &[&str] = &[
    "id",
    "object",
    "created",
    "model",
    "choices",
    "usage",
    "system_fingerprint",
];

/// Converter from OpenAI Chat Completions API to Anthropic Messages API
pub struct OpenAIChatToAnthropicConverter;

impl ApiConverter for OpenAIChatToAnthropicConverter {
    fn convert_request(&self, body: &Bytes, _target: ApiType) -> Result<Bytes, ConversionError> {
        let ctx = NoopTelemetry;
        self.convert_request_with_ctx(body, &ctx)
    }

    fn convert_response(
        &self,
        body: &Bytes,
        _source: ApiType,
        _target: ApiType,
        is_stream: bool,
    ) -> Result<Bytes, ConversionError> {
        if is_stream {
            return Err(ConversionError::new(
                ConversionErrorKind::NotImplemented,
                "not_implemented",
                "OpenAIChatToAnthropicConverter::convert_response for streaming not implemented yet (P5)",
            ));
        }

        let ctx = NoopTelemetry;
        self.convert_response_with_ctx(body, &ctx)
    }
}

impl OpenAIChatToAnthropicConverter {
    /// Convert request body with telemetry context
    pub fn convert_request_with_ctx(
        &self,
        body: &Bytes,
        ctx: &dyn TelemetryContext,
    ) -> Result<Bytes, ConversionError> {
        // Parse JSON
        let input: Value = serde_json::from_slice(body).map_err(|e| {
            ConversionError::new(
                ConversionErrorKind::InvalidJson,
                "invalid_json",
                format!("Failed to parse JSON: {}", e),
            )
            .with_original_body(body.clone())
        })?;

        let obj = input
            .as_object()
            .ok_or_else(|| {
                ConversionError::new(
                    ConversionErrorKind::InvalidJson,
                    "invalid_json",
                    "Request body must be a JSON object",
                )
            })?
            .clone();

        // Report unknown fields
        for key in obj.keys() {
            if !KNOWN_OPENAI_REQUEST_FIELDS.contains(&key.as_str()) {
                ctx.report_unknown_field(&format!("request.{}", key));
            }
        }

        // Build output object
        let mut output = Map::new();

        // Handle messages and extract system
        let (messages, system) = self.convert_messages(&obj, ctx)?;
        if messages.is_empty() {
            return Err(ConversionError::new(
                ConversionErrorKind::SchemaMismatch,
                "missing_field",
                "Missing required field: messages",
            )
            .with_field_path("request.messages"));
        }

        output.insert("messages".to_string(), json!(messages));

        // Insert system if present
        if let Some(sys) = system {
            ctx.report_field_mapped("messages[0]", "system");
            output.insert("system".to_string(), sys);
        }

        // Copy model
        if let Some(model) = obj.get("model") {
            ctx.report_field_mapped("model", "model");
            output.insert("model".to_string(), model.clone());
        }

        // Handle max_tokens or max_completion_tokens
        if let Some(max_tokens) = obj.get("max_tokens") {
            ctx.report_field_mapped("max_tokens", "max_tokens");
            output.insert("max_tokens".to_string(), max_tokens.clone());
        } else if let Some(max_completion_tokens) = obj.get("max_completion_tokens") {
            ctx.report_field_mapped("max_completion_tokens", "max_tokens");
            output.insert("max_tokens".to_string(), max_completion_tokens.clone());
        }

        // Copy temperature
        if let Some(temp) = obj.get("temperature") {
            ctx.report_field_mapped("temperature", "temperature");
            output.insert("temperature".to_string(), temp.clone());
        }

        // Copy top_p
        if let Some(top_p) = obj.get("top_p") {
            ctx.report_field_mapped("top_p", "top_p");
            output.insert("top_p".to_string(), top_p.clone());
        }

        // Skip n with telemetry
        if obj.contains_key("n") {
            ctx.report_field_skipped("request.n", "anthropic_single_choice");
        }

        // Copy stop as stop_sequences
        if let Some(stop) = obj.get("stop") {
            if let Some(stop_str) = stop.as_str() {
                ctx.report_field_mapped("stop", "stop_sequences");
                output.insert("stop_sequences".to_string(), json!([stop_str]));
            } else if let Some(stop_arr) = stop.as_array() {
                ctx.report_field_mapped("stop", "stop_sequences");
                output.insert("stop_sequences".to_string(), json!(stop_arr));
            }
        }

        // Copy stream
        if let Some(stream) = obj.get("stream") {
            ctx.report_field_mapped("stream", "stream");
            output.insert("stream".to_string(), stream.clone());
        }

        // Convert tools
        if let Some(tools) = obj.get("tools") {
            let converted_tools = self.convert_tools(tools, ctx)?;
            ctx.report_field_mapped("tools", "tools");
            output.insert("tools".to_string(), json!(converted_tools));
        }

        // Convert tool_choice
        if let Some(tool_choice) = obj.get("tool_choice") {
            let (converted, disable_parallel) = self.convert_tool_choice(tool_choice, &obj, ctx)?;
            ctx.report_field_mapped("tool_choice", "tool_choice");
            // Merge disable_parallel into tool_choice object
            let mut tool_choice_obj = converted.as_object().unwrap().clone();
            if let Some(disable) = disable_parallel {
                tool_choice_obj.insert("disable_parallel_tool_use".to_string(), json!(disable));
            }
            output.insert("tool_choice".to_string(), json!(tool_choice_obj));
        } else {
            // Handle parallel_tool_calls even without tool_choice
            if let Some(parallel) = obj.get("parallel_tool_calls") {
                if let Some(parallel_val) = parallel.as_bool() {
                    if !parallel_val {
                        ctx.report_field_mapped("parallel_tool_calls", "tool_choice.disable_parallel_tool_use");
                        output.insert("tool_choice".to_string(), json!({
                            "type": "auto",
                            "disable_parallel_tool_use": true
                        }));
                    }
                }
            }
        }

        // Convert user to metadata.user_id
        if let Some(user) = obj.get("user") {
            let mut metadata = Map::new();
            metadata.insert("user_id".to_string(), user.clone());
            ctx.report_field_mapped("user", "metadata.user_id");
            output.insert("metadata".to_string(), json!(metadata));
        }

        Ok(serde_json::to_vec(&json!(output)).map_err(|e| {
            ConversionError::new(
                ConversionErrorKind::Internal,
                "internal",
                format!("Failed to serialize output: {}", e),
            )
        })?
        .into())
    }

    /// Convert messages array and extract system message
    fn convert_messages(
        &self,
        obj: &Map<String, Value>,
        ctx: &dyn TelemetryContext,
    ) -> Result<(Vec<Value>, Option<Value>), ConversionError> {
        let messages = match obj.get("messages") {
            Some(v) => v.as_array().ok_or_else(|| {
                ConversionError::new(
                    ConversionErrorKind::SchemaMismatch,
                    "bad_type",
                    "messages must be an array",
                )
                .with_field_path("request.messages")
            })?,
            None => {
                return Err(ConversionError::new(
                    ConversionErrorKind::SchemaMismatch,
                    "missing_field",
                    "Missing required field: messages",
                )
                .with_field_path("request.messages"));
            }
        };

        let mut result = Vec::new();
        let mut system = None;

        for (i, msg) in messages.iter().enumerate() {
            let msg_obj = msg.as_object().ok_or_else(|| {
                ConversionError::new(
                    ConversionErrorKind::SchemaMismatch,
                    "bad_type",
                    format!("messages[{}] must be an object", i),
                )
                .with_field_path(format!("request.messages[{}]", i))
            })?;

            let role = msg_obj.get("role").and_then(|r| r.as_str()).ok_or_else(|| {
                ConversionError::new(
                    ConversionErrorKind::SchemaMismatch,
                    "bad_type",
                    format!("messages[{}].role must be a string", i),
                )
                .with_field_path(format!("request.messages[{}].role", i))
            })?;

            // Extract system message
            if role == "system" {
                if system.is_some() {
                    ctx.report_field_skipped(&format!("request.messages[{}]", i), "multiple_system_messages");
                } else {
                    let content = msg_obj.get("content");
                    if let Some(content_str) = content.and_then(|c| c.as_str()) {
                        system = Some(json!(content_str));
                    } else if let Some(content_arr) = content.and_then(|c| c.as_array()) {
                        // Convert array to concatenated string
                        let parts: Vec<&str> = content_arr
                            .iter()
                            .filter_map(|v| v.as_str())
                            .collect();
                        system = Some(json!(parts.join("\n\n")));
                    }
                }
                continue;
            }

            // Convert other messages
            let content = msg_obj.get("content");
            let tool_calls = msg_obj.get("tool_calls");

            // Handle tool role (from OpenAI tool messages)
            if role == "tool" {
                let tool_call_id = msg_obj
                    .get("tool_call_id")
                    .and_then(|id| id.as_str())
                    .ok_or_else(|| {
                        ConversionError::new(
                            ConversionErrorKind::SchemaMismatch,
                            "missing_field",
                            format!("messages[{}] missing tool_call_id", i),
                        )
                        .with_field_path(format!("request.messages[{}].tool_call_id", i))
                    })?;

                let tool_content = msg_obj.get("content").cloned().unwrap_or_else(|| json!(""));

                result.push(json!({
                    "role": "user",
                    "content": [{
                        "type": "tool_result",
                        "tool_use_id": tool_call_id,
                        "content": tool_content
                    }]
                }));

                ctx.report_field_mapped(&format!("messages[{}]", i), &format!("messages[{}]", result.len() - 1));
                continue;
            }

            // Handle assistant with tool_calls
            if role == "assistant" {
                if let Some(tc) = tool_calls {
                    let tc_arr = tc.as_array().ok_or_else(|| {
                        ConversionError::new(
                            ConversionErrorKind::SchemaMismatch,
                            "bad_type",
                            format!("messages[{}].tool_calls must be an array", i),
                        )
                        .with_field_path(format!("request.messages[{}].tool_calls", i))
                    })?;

                    let mut content_blocks = Vec::new();

                    // Add text content if present
                    if let Some(content_val) = content {
                        if let Some(content_str) = content_val.as_str() {
                            if !content_str.is_empty() {
                                content_blocks.push(json!({
                                    "type": "text",
                                    "text": content_str
                                }));
                            }
                        } else if let Some(content_arr) = content_val.as_array() {
                            for block in content_arr {
                                if let Some(block_obj) = block.as_object() {
                                    if block_obj.get("type").and_then(|t| t.as_str()) == Some("text") {
                                        if let Some(text) = block_obj.get("text").and_then(|t| t.as_str()) {
                                            content_blocks.push(json!({
                                                "type": "text",
                                                "text": text
                                            }));
                                        }
                                    } else {
                                        ctx.report_field_skipped(
                                            &format!("request.messages[{}].content", i),
                                            "unsupported_content_type"
                                        );
                                    }
                                }
                            }
                        }
                    }

                    // Convert tool_calls to tool_use blocks
                    for (j, tc_item) in tc_arr.iter().enumerate() {
                        let tc_obj = tc_item.as_object().ok_or_else(|| {
                            ConversionError::new(
                                ConversionErrorKind::SchemaMismatch,
                                "bad_type",
                                format!("messages[{}].tool_calls[{}] must be an object", i, j),
                            )
                            .with_field_path(format!("request.messages[{}].tool_calls[{}]", i, j))
                        })?;

                        let id = tc_obj.get("id").and_then(|id| id.as_str()).unwrap_or("");
                        let function = tc_obj.get("function").ok_or_else(|| {
                            ConversionError::new(
                                ConversionErrorKind::SchemaMismatch,
                                "missing_field",
                                format!("messages[{}].tool_calls[{}] missing function", i, j),
                            )
                            .with_field_path(format!("request.messages[{}].tool_calls[{}].function", i, j))
                        })?;

                        let func_obj = function.as_object().ok_or_else(|| {
                            ConversionError::new(
                                ConversionErrorKind::SchemaMismatch,
                                "bad_type",
                                format!("messages[{}].tool_calls[{}].function must be an object", i, j),
                            )
                            .with_field_path(format!("request.messages[{}].tool_calls[{}].function", i, j))
                        })?;

                        let name = func_obj.get("name").and_then(|n| n.as_str()).unwrap_or("");
                        let arguments = func_obj.get("arguments").and_then(|a| a.as_str()).unwrap_or("{}");

                        let input = match serde_json::from_str::<Value>(arguments) {
                            Ok(parsed) => parsed,
                            Err(_) => {
                                ctx.report_field_skipped(
                                    &format!("request.messages[{}].tool_calls[{}].function.arguments", i, j),
                                    "tool_args_invalid_json"
                                );
                                json!({})
                            }
                        };

                        content_blocks.push(json!({
                            "type": "tool_use",
                            "id": id,
                            "name": name,
                            "input": input
                        }));
                    }

                    result.push(json!({
                        "role": "assistant",
                        "content": content_blocks
                    }));

                    ctx.report_field_mapped(&format!("messages[{}]", i), &format!("messages[{}]", result.len() - 1));
                    continue;
                }
            }

            // Handle regular user/assistant messages
            let content_value = if let Some(content_str) = content.and_then(|c| c.as_str()) {
                json!(content_str)
            } else if let Some(content_arr) = content.and_then(|c| c.as_array()) {
                // Convert array content
                let mut blocks = Vec::new();
                for block in content_arr {
                    if let Some(block_obj) = block.as_object() {
                        let block_type = block_obj.get("type").and_then(|t| t.as_str());
                        if block_type == Some("text") {
                            if let Some(text) = block_obj.get("text").and_then(|t| t.as_str()) {
                                blocks.push(json!({
                                    "type": "text",
                                    "text": text
                                }));
                            }
                        } else if block_type == Some("image_url") {
                            let converted = self.convert_image_url_block_request(block_obj, ctx)?;
                            if let Some(conv) = converted {
                                blocks.push(conv);
                            }
                        } else {
                            ctx.report_field_skipped(
                                &format!("request.messages[{}].content", i),
                                "unsupported_content_type"
                            );
                        }
                    }
                }
                json!(blocks)
            } else {
                json!("")
            };

            result.push(json!({
                "role": role,
                "content": content_value
            }));

            ctx.report_field_mapped(&format!("messages[{}]", i), &format!("messages[{}]", result.len() - 1));
        }

        Ok((result, system))
    }

    /// Convert image_url block to Anthropic format (for request conversion)
    fn convert_image_url_block_request(
        &self,
        block: &Map<String, Value>,
        ctx: &dyn TelemetryContext,
    ) -> Result<Option<Value>, ConversionError> {
        let image_url = block.get("image_url").ok_or_else(|| {
            ConversionError::new(
                ConversionErrorKind::SchemaMismatch,
                "missing_field",
                "image_url missing",
            )
            .with_field_path("content.image_url")
        })?;

        let image_url_obj = image_url.as_object().ok_or_else(|| {
            ConversionError::new(
                ConversionErrorKind::SchemaMismatch,
                "bad_type",
                "image_url must be an object",
            )
            .with_field_path("content.image_url")
        })?;

        let url = image_url_obj.get("url").and_then(|u| u.as_str()).unwrap_or("");

        // Try to parse data URL
        if url.starts_with("data:") {
            let parts: Vec<&str> = url.splitn(2, ',').collect();
            if parts.len() == 2 {
                let header_parts: Vec<&str> = parts[0].split(';').collect();
                let media_type = header_parts.first().unwrap_or(&"image/jpeg");
                let data = parts[1];

                return Ok(Some(json!({
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": media_type.trim_start_matches("data:"),
                        "data": data
                    }
                })));
            }
        }

        // Skip regular URLs (can't convert to base64 without fetching)
        ctx.report_field_skipped("content.image_url.url", "non_base64_image_url");
        Ok(None)
    }

    /// Convert tools array from OpenAI to Anthropic format
    fn convert_tools(
        &self,
        tools: &Value,
        ctx: &dyn TelemetryContext,
    ) -> Result<Vec<Value>, ConversionError> {
        let tools_arr = tools.as_array().ok_or_else(|| {
            ConversionError::new(
                ConversionErrorKind::SchemaMismatch,
                "bad_type",
                "tools must be an array",
            )
            .with_field_path("request.tools")
        })?;

        let mut result = Vec::new();
        for (i, tool) in tools_arr.iter().enumerate() {
            let tool_obj = tool.as_object().ok_or_else(|| {
                ConversionError::new(
                    ConversionErrorKind::SchemaMismatch,
                    "bad_type",
                    format!("tools[{}] must be an object", i),
                )
                .with_field_path(format!("request.tools[{}]", i))
            })?;

            let tool_type = tool_obj.get("type").and_then(|t| t.as_str()).unwrap_or("function");
            if tool_type != "function" {
                ctx.report_field_skipped(&format!("request.tools[{}].type", i), "non_function_tool");
                continue;
            }

            let function = tool_obj.get("function").ok_or_else(|| {
                ConversionError::new(
                    ConversionErrorKind::SchemaMismatch,
                    "missing_field",
                    format!("tools[{}] missing function", i),
                )
                .with_field_path(format!("request.tools[{}].function", i))
            })?;

            let func_obj = function.as_object().ok_or_else(|| {
                ConversionError::new(
                    ConversionErrorKind::SchemaMismatch,
                    "bad_type",
                    format!("tools[{}].function must be an object", i),
                )
                .with_field_path(format!("request.tools[{}].function", i))
            })?;

            let name = func_obj.get("name").and_then(|n| n.as_str()).ok_or_else(|| {
                ConversionError::new(
                    ConversionErrorKind::SchemaMismatch,
                    "missing_field",
                    format!("tools[{}].function missing name", i),
                )
                .with_field_path(format!("request.tools[{}].function.name", i))
            })?;

            let description = func_obj.get("description");
            let parameters = func_obj.get("parameters").cloned().unwrap_or_else(|| json!({}));

            let mut output_tool = json!({
                "name": name,
                "input_schema": parameters
            });

            if let Some(desc) = description {
                output_tool
                    .as_object_mut()
                    .unwrap()
                    .insert("description".to_string(), desc.clone());
            }

            result.push(output_tool);
        }

        Ok(result)
    }

    /// Convert tool_choice from OpenAI to Anthropic format
    fn convert_tool_choice(
        &self,
        tool_choice: &Value,
        obj: &Map<String, Value>,
        ctx: &dyn TelemetryContext,
    ) -> Result<(Value, Option<bool>), ConversionError> {
        // Check for parallel_tool_calls
        let disable_parallel = if let Some(parallel) = obj.get("parallel_tool_calls") {
            if let Some(parallel_val) = parallel.as_bool() {
                if !parallel_val {
                    ctx.report_field_mapped("parallel_tool_calls", "tool_choice.disable_parallel_tool_use");
                    Some(true)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        if let Some(tc_str) = tool_choice.as_str() {
            match tc_str {
                "auto" => return Ok((json!({"type": "auto"}), disable_parallel)),
                "required" => return Ok((json!({"type": "any"}), disable_parallel)),
                "none" => return Ok((json!({"type": "none"}), disable_parallel)),
                _ => {}
            }
        }

        if let Some(tc_obj) = tool_choice.as_object() {
            let tc_type = tc_obj.get("type").and_then(|t| t.as_str());

            if tc_type == Some("auto") {
                return Ok((json!({"type": "auto"}), disable_parallel));
            } else if tc_type == Some("required") {
                return Ok((json!({"type": "any"}), disable_parallel));
            } else if tc_type == Some("none") {
                return Ok((json!({"type": "none"}), disable_parallel));
            } else if tc_type == Some("function") {
                let function = tc_obj.get("function").ok_or_else(|| {
                    ConversionError::new(
                        ConversionErrorKind::SchemaMismatch,
                        "missing_field",
                        "tool_choice.function missing",
                    )
                    .with_field_path("request.tool_choice.function")
                })?;

                let func_obj = function.as_object().ok_or_else(|| {
                    ConversionError::new(
                        ConversionErrorKind::SchemaMismatch,
                        "bad_type",
                        "tool_choice.function must be an object",
                    )
                    .with_field_path("request.tool_choice.function")
                })?;

                let name = func_obj.get("name").and_then(|n| n.as_str()).ok_or_else(|| {
                    ConversionError::new(
                        ConversionErrorKind::SchemaMismatch,
                        "missing_field",
                        "tool_choice.function.name missing",
                    )
                    .with_field_path("request.tool_choice.function.name")
                })?;

                return Ok((
                    json!({
                        "type": "tool",
                        "name": name
                    }),
                    disable_parallel,
                ));
            }
        }

        // Default to auto
        Ok((json!({"type": "auto"}), disable_parallel))
    }

    /// Convert response body with telemetry context
    pub fn convert_response_with_ctx(
        &self,
        body: &Bytes,
        ctx: &dyn TelemetryContext,
    ) -> Result<Bytes, ConversionError> {
        // Parse JSON
        let input: Value = serde_json::from_slice(body).map_err(|e| {
            ConversionError::new(
                ConversionErrorKind::InvalidJson,
                "invalid_json",
                format!("Failed to parse JSON: {}", e),
            )
            .with_original_body(body.clone())
        })?;

        let obj = input
            .as_object()
            .ok_or_else(|| {
                ConversionError::new(
                    ConversionErrorKind::InvalidJson,
                    "invalid_json",
                    "Response body must be a JSON object",
                )
            })?
            .clone();

        // Check if this is an error response from upstream
        if let Some(error) = obj.get("error") {
            return self.convert_error_response(&obj, error, ctx);
        }

        // Report unknown fields
        for key in obj.keys() {
            if !KNOWN_OPENAI_RESPONSE_FIELDS.contains(&key.as_str()) {
                ctx.report_unknown_field(&format!("response.{}", key));
            }
        }

        // Build Anthropic response
        let mut output = Map::new();

        // Top-level wrapper
        output.insert("type".to_string(), json!("message"));
        output.insert("role".to_string(), json!("assistant"));

        // Copy id
        if let Some(id) = obj.get("id") {
            ctx.report_field_mapped("id", "id");
            output.insert("id".to_string(), id.clone());
        }

        // Copy model
        if let Some(model) = obj.get("model") {
            ctx.report_field_mapped("model", "model");
            output.insert("model".to_string(), model.clone());
        }

        // stop_sequence is always null (OpenAI has no equivalent)
        output.insert("stop_sequence".to_string(), json!(null));

        // Process choices
        let choices = obj.get("choices").and_then(|c| c.as_array()).ok_or_else(|| {
            ConversionError::new(
                ConversionErrorKind::SchemaMismatch,
                "missing_field",
                "Missing required field: choices",
            )
            .with_field_path("response.choices")
        })?;

        if choices.is_empty() {
            return Err(ConversionError::new(
                ConversionErrorKind::SchemaMismatch,
                "missing_field",
                "choices array is empty",
            )
            .with_field_path("response.choices"));
        }

        // Warn about extra choices (Anthropic only supports single choice)
        if choices.len() > 1 {
            for i in 1..choices.len() {
                ctx.report_field_skipped(
                    &format!("response.choices[{}]", i),
                    "anthropic_single_choice",
                );
            }
        }

        let choice = &choices[0];
        let message = choice
            .get("message")
            .ok_or_else(|| {
                ConversionError::new(
                    ConversionErrorKind::SchemaMismatch,
                    "missing_field",
                    "choices[0].message missing",
                )
                .with_field_path("response.choices[0].message")
            })?
            .as_object()
            .ok_or_else(|| {
                ConversionError::new(
                    ConversionErrorKind::SchemaMismatch,
                    "bad_type",
                    "choices[0].message must be an object",
                )
                .with_field_path("response.choices[0].message")
            })?
            .clone();

        // Convert content
        let content = self.convert_content(&message, choice, ctx)?;
        output.insert("content".to_string(), json!(content));

        // Convert tool_calls if present
        if let Some(tool_calls) = message.get("tool_calls") {
            let tool_use_blocks = self.convert_tool_calls(tool_calls, ctx)?;
            if let Some(existing_content) = output.get_mut("content") {
                if let Some(arr) = existing_content.as_array_mut() {
                    arr.extend(tool_use_blocks);
                }
            }
        }

        // Convert stop_reason
        if let Some(finish_reason) = choice.get("finish_reason") {
            let stop_reason = self.convert_stop_reason(finish_reason, ctx);
            ctx.report_field_mapped("choices[0].finish_reason", "stop_reason");
            output.insert("stop_reason".to_string(), json!(stop_reason));
        }

        // Convert usage
        if let Some(usage) = obj.get("usage") {
            let converted_usage = self.convert_usage(usage, ctx);
            output.insert("usage".to_string(), json!(converted_usage));
        }

        // Skip system_fingerprint
        if obj.contains_key("system_fingerprint") {
            ctx.report_field_skipped("response.system_fingerprint", "anthropic_specific");
        }

        Ok(serde_json::to_vec(&json!(output)).map_err(|e| {
            ConversionError::new(
                ConversionErrorKind::Internal,
                "internal",
                format!("Failed to serialize output: {}", e),
            )
        })?
        .into())
    }

    /// Convert content field
    fn convert_content(
        &self,
        message: &Map<String, Value>,
        _choice: &Value,
        ctx: &dyn TelemetryContext,
    ) -> Result<Vec<Value>, ConversionError> {
        let content = message.get("content");
        let tool_calls = message.get("tool_calls");

        // If both content and tool_calls are missing, that's an error
        if content.is_none() && tool_calls.is_none() {
            return Err(ConversionError::new(
                ConversionErrorKind::SchemaMismatch,
                "missing_field",
                "Both content and tool_calls are missing",
            )
            .with_field_path("response.choices[0].message"));
        }

        // If content is empty and no tool_calls, return empty array
        if content.is_none() || content.and_then(|c| c.as_str()).map_or(false, |s| s.is_empty()) {
            if tool_calls.is_none() {
                ctx.report_field_mapped("choices[0].message.content", "content");
                return Ok(vec![]);
            }
        }

        // If content is a non-empty string, wrap in text block
        if let Some(content_str) = content.and_then(|c| c.as_str()) {
            if !content_str.is_empty() {
                ctx.report_field_mapped("choices[0].message.content", "content");
                return Ok(vec![json!({
                    "type": "text",
                    "text": content_str
                })]);
            }
        }

        // If content is an array, map each item
        if let Some(content_arr) = content.and_then(|c| c.as_array()) {
            let mut result = Vec::new();
            for (i, item) in content_arr.iter().enumerate() {
                if let Some(item_obj) = item.as_object() {
                    let item_type = item_obj.get("type").and_then(|t| t.as_str());

                    if item_type == Some("text") {
                        if let Some(text) = item_obj.get("text").and_then(|t| t.as_str()) {
                            result.push(json!({
                                "type": "text",
                                "text": text
                            }));
                        }
                    } else if item_type == Some("image_url") {
                        // Convert image_url back to Anthropic image format
                        let image_block = self.convert_image_url_block(item_obj, i, ctx)?;
                        if let Some(block) = image_block {
                            result.push(block);
                        }
                    } else {
                        ctx.report_field_skipped(
                            &format!("response.choices[0].message.content[{}].type", i),
                            "unsupported_content_type",
                        );
                    }
                }
            }
            ctx.report_field_mapped("choices[0].message.content", "content");
            return Ok(result);
        }

        // Default empty content if we have tool_calls
        Ok(vec![])
    }

    /// Convert image_url block back to Anthropic format
    fn convert_image_url_block(
        &self,
        block: &Map<String, Value>,
        idx: usize,
        ctx: &dyn TelemetryContext,
    ) -> Result<Option<Value>, ConversionError> {
        let image_url = block.get("image_url").ok_or_else(|| {
            ConversionError::new(
                ConversionErrorKind::SchemaMismatch,
                "missing_field",
                format!("content[{}].image_url missing", idx),
            )
            .with_field_path(format!("response.choices[0].message.content[{}].image_url", idx))
        })?;

        let image_url_obj = image_url.as_object().ok_or_else(|| {
            ConversionError::new(
                ConversionErrorKind::SchemaMismatch,
                "bad_type",
                format!("content[{}].image_url must be an object", idx),
            )
            .with_field_path(format!(
                "response.choices[0].message.content[{}].image_url",
                idx
            ))
        })?;

        let url = image_url_obj.get("url").and_then(|u| u.as_str()).unwrap_or("");

        // Try to parse data URL
        if url.starts_with("data:") {
            // Format: data:<media_type>;base64,<data>
            let parts: Vec<&str> = url.splitn(2, ',').collect();
            if parts.len() == 2 {
                let header_parts: Vec<&str> = parts[0].split(';').collect();
                let media_type = header_parts.first().unwrap_or(&"image/jpeg");
                let data = parts[1];

                return Ok(Some(json!({
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": media_type.trim_start_matches("data:"),
                        "data": data
                    }
                })));
            }
        }

        // Otherwise, skip (we can't convert regular URLs back to base64 without fetching)
        ctx.report_field_skipped(
            &format!("response.choices[0].message.content[{}].image_url.url", idx),
            "non_base64_image_url",
        );
        Ok(None)
    }

    /// Convert tool_calls to tool_use blocks
    fn convert_tool_calls(
        &self,
        tool_calls: &Value,
        ctx: &dyn TelemetryContext,
    ) -> Result<Vec<Value>, ConversionError> {
        let tool_calls_arr = tool_calls.as_array().ok_or_else(|| {
            ConversionError::new(
                ConversionErrorKind::SchemaMismatch,
                "bad_type",
                "tool_calls must be an array",
            )
            .with_field_path("response.choices[0].message.tool_calls")
        })?;

        let mut result = Vec::new();
        for (i, tc) in tool_calls_arr.iter().enumerate() {
            let tc_obj = tc.as_object().ok_or_else(|| {
                ConversionError::new(
                    ConversionErrorKind::SchemaMismatch,
                    "bad_type",
                    format!("tool_calls[{}] must be an object", i),
                )
                .with_field_path(format!("response.choices[0].message.tool_calls[{}]", i))
            })?;

            let id = tc_obj.get("id").and_then(|id| id.as_str()).ok_or_else(|| {
                ConversionError::new(
                    ConversionErrorKind::SchemaMismatch,
                    "missing_field",
                    format!("tool_calls[{}] missing id", i),
                )
                .with_field_path(format!("response.choices[0].message.tool_calls[{}].id", i))
            })?;

            let function = tc_obj.get("function").ok_or_else(|| {
                ConversionError::new(
                    ConversionErrorKind::SchemaMismatch,
                    "missing_field",
                    format!("tool_calls[{}] missing function", i),
                )
                .with_field_path(format!(
                    "response.choices[0].message.tool_calls[{}].function",
                    i
                ))
            })?;

            let function_obj = function.as_object().ok_or_else(|| {
                ConversionError::new(
                    ConversionErrorKind::SchemaMismatch,
                    "bad_type",
                    format!("tool_calls[{}].function must be an object", i),
                )
                .with_field_path(format!(
                    "response.choices[0].message.tool_calls[{}].function",
                    i
                ))
            })?;

            let name = function_obj.get("name").and_then(|n| n.as_str()).ok_or_else(|| {
                ConversionError::new(
                    ConversionErrorKind::SchemaMismatch,
                    "missing_field",
                    format!("tool_calls[{}].function missing name", i),
                )
                .with_field_path(format!(
                    "response.choices[0].message.tool_calls[{}].function.name",
                    i
                ))
            })?;

            let arguments = function_obj.get("arguments").and_then(|a| a.as_str()).unwrap_or("{}");

            // Parse arguments JSON
            let input = match serde_json::from_str::<Value>(arguments) {
                Ok(parsed) => parsed,
                Err(_) => {
                    ctx.report_field_skipped(
                        &format!(
                            "response.choices[0].message.tool_calls[{}].function.arguments",
                            i
                        ),
                        "tool_args_invalid_json",
                    );
                    json!({})
                }
            };

            result.push(json!({
                "type": "tool_use",
                "id": id,
                "name": name,
                "input": input
            }));

            ctx.report_field_mapped(
                &format!("choices[0].message.tool_calls[{}]", i),
                &format!("content[{}]", i),
            );
        }

        Ok(result)
    }

    /// Convert finish_reason to stop_reason
    fn convert_stop_reason(&self, finish_reason: &Value, ctx: &dyn TelemetryContext) -> &str {
        if let Some(reason) = finish_reason.as_str() {
            match reason {
                "stop" => "end_turn",
                "length" => "max_tokens",
                "tool_calls" | "function_call" => "tool_use",
                "content_filter" => {
                    ctx.report_field_skipped("response.choices[0].finish_reason", "content_filter_mapped_to_end_turn");
                    "end_turn"
                }
                _ => {
                    ctx.report_field_skipped("response.choices[0].finish_reason", "unknown_finish_reason");
                    "end_turn"
                }
            }
        } else if finish_reason.is_null() {
            ctx.report_field_skipped("response.choices[0].finish_reason", "null_finish_reason");
            "end_turn"
        } else {
            ctx.report_field_skipped("response.choices[0].finish_reason", "unknown_finish_reason_type");
            "end_turn"
        }
    }

    /// Convert usage field
    fn convert_usage(&self, usage: &Value, ctx: &dyn TelemetryContext) -> Value {
        let mut result = Map::new();

        if let Some(prompt_tokens) = usage.get("prompt_tokens") {
            ctx.report_field_mapped("usage.prompt_tokens", "usage.input_tokens");
            result.insert("input_tokens".to_string(), prompt_tokens.clone());
        }

        if let Some(completion_tokens) = usage.get("completion_tokens") {
            ctx.report_field_mapped("usage.completion_tokens", "usage.output_tokens");
            result.insert("output_tokens".to_string(), completion_tokens.clone());
        }

        // Handle cached_tokens
        if let Some(prompt_tokens_details) = usage.get("prompt_tokens_details") {
            if let Some(details_obj) = prompt_tokens_details.as_object() {
                if let Some(cached_tokens) = details_obj.get("cached_tokens") {
                    ctx.report_field_mapped(
                        "usage.prompt_tokens_details.cached_tokens",
                        "usage.cache_read_input_tokens",
                    );
                    result.insert("cache_read_input_tokens".to_string(), cached_tokens.clone());
                }
            }
        }

        json!(result)
    }

    /// Convert OpenAI error response to Anthropic error format
    fn convert_error_response(
        &self,
        _input: &Map<String, Value>,
        error: &Value,
        ctx: &dyn TelemetryContext,
    ) -> Result<Bytes, ConversionError> {
        let error_obj = error.as_object().ok_or_else(|| {
            ConversionError::new(
                ConversionErrorKind::SchemaMismatch,
                "bad_type",
                "error must be an object",
            )
            .with_field_path("response.error")
        })?;

        let error_type = error_obj.get("type").and_then(|t| t.as_str()).unwrap_or("api_error");
        let error_message = error_obj.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown error");

        // Map error type per field-mapping.md §5
        let anthropic_type = match error_type {
            "invalid_request_error" => "invalid_request_error",
            "authentication_error" => "authentication_error",
            "permission_error" => "permission_error",
            "not_found_error" => "not_found_error",
            "rate_limit_exceeded" => "rate_limit_error",
            "server_error" | _ => {
                if error_type != "server_error" && error_type != "api_error" {
                    ctx.report_field_skipped("response.error.type", "unknown_error_type");
                }
                "api_error"
            }
        };

        let output = json!({
            "type": "error",
            "error": {
                "type": anthropic_type,
                "message": error_message
            }
        });

        Ok(serde_json::to_vec(&output).map_err(|e| {
            ConversionError::new(
                ConversionErrorKind::Internal,
                "internal",
                format!("Failed to serialize error output: {}", e),
            )
        })?
        .into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_simple_text_response() {
        let converter = OpenAIChatToAnthropicConverter;
        let input = json!({
            "id": "chatcmpl-123",
            "object": "chat.completion",
            "created": 1677652288,
            "model": "gpt-4",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Hello, world!"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 5
            }
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_response(&input_bytes, ApiType::OpenAIChat, ApiType::Anthropic, false).unwrap();
        let output: Value = serde_json::from_slice(&result).unwrap();

        assert_eq!(output["type"], "message");
        assert_eq!(output["role"], "assistant");
        assert_eq!(output["id"], "chatcmpl-123");
        assert_eq!(output["model"], "gpt-4");
        assert_eq!(output["stop_sequence"], json!(null));
        assert_eq!(output["content"][0]["type"], "text");
        assert_eq!(output["content"][0]["text"], "Hello, world!");
        assert_eq!(output["stop_reason"], "end_turn");
        assert_eq!(output["usage"]["input_tokens"], 10);
        assert_eq!(output["usage"]["output_tokens"], 5);
    }

    #[test]
    fn test_tool_calls_with_valid_args() {
        let converter = OpenAIChatToAnthropicConverter;
        let input = json!({
            "id": "chatcmpl-123",
            "model": "gpt-4",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_abc",
                        "type": "function",
                        "function": {
                            "name": "get_weather",
                            "arguments": "{\"location\":\"SF\"}"
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_response(&input_bytes, ApiType::OpenAIChat, ApiType::Anthropic, false).unwrap();
        let output: Value = serde_json::from_slice(&result).unwrap();

        assert_eq!(output["content"][0]["type"], "tool_use");
        assert_eq!(output["content"][0]["id"], "call_abc");
        assert_eq!(output["content"][0]["name"], "get_weather");
        assert_eq!(output["content"][0]["input"]["location"], "SF");
        assert_eq!(output["stop_reason"], "tool_use");
    }

    #[test]
    fn test_tool_calls_with_invalid_args() {
        let converter = OpenAIChatToAnthropicConverter;
        let input = json!({
            "id": "chatcmpl-123",
            "model": "gpt-4",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_abc",
                        "type": "function",
                        "function": {
                            "name": "get_weather",
                            "arguments": "invalid json{"
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_response(&input_bytes, ApiType::OpenAIChat, ApiType::Anthropic, false).unwrap();
        let output: Value = serde_json::from_slice(&result).unwrap();

        // Should still succeed with empty input object
        assert_eq!(output["content"][0]["type"], "tool_use");
        assert_eq!(output["content"][0]["id"], "call_abc");
        assert_eq!(output["content"][0]["name"], "get_weather");
        assert_eq!(output["content"][0]["input"], json!({}));
    }

    #[test]
    fn test_length_finish_reason() {
        let converter = OpenAIChatToAnthropicConverter;
        let input = json!({
            "id": "chatcmpl-123",
            "model": "gpt-4",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Text"
                },
                "finish_reason": "length"
            }]
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_response(&input_bytes, ApiType::OpenAIChat, ApiType::Anthropic, false).unwrap();
        let output: Value = serde_json::from_slice(&result).unwrap();

        assert_eq!(output["stop_reason"], "max_tokens");
    }

    #[test]
    fn test_empty_content_no_tool_calls() {
        let converter = OpenAIChatToAnthropicConverter;
        let input = json!({
            "id": "chatcmpl-123",
            "model": "gpt-4",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": ""
                },
                "finish_reason": "stop"
            }]
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_response(&input_bytes, ApiType::OpenAIChat, ApiType::Anthropic, false).unwrap();
        let output: Value = serde_json::from_slice(&result).unwrap();

        assert_eq!(output["content"], json!([]));
    }

    #[test]
    fn test_multiple_choices_warns() {
        let converter = OpenAIChatToAnthropicConverter;
        let input = json!({
            "id": "chatcmpl-123",
            "model": "gpt-4",
            "choices": [
                {
                    "index": 0,
                    "message": {"role": "assistant", "content": "First"},
                    "finish_reason": "stop"
                },
                {
                    "index": 1,
                    "message": {"role": "assistant", "content": "Second"},
                    "finish_reason": "stop"
                }
            ]
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_response(&input_bytes, ApiType::OpenAIChat, ApiType::Anthropic, false).unwrap();
        let output: Value = serde_json::from_slice(&result).unwrap();

        // Should use first choice only
        assert_eq!(output["content"][0]["text"], "First");
    }

    #[test]
    fn test_cached_tokens() {
        let converter = OpenAIChatToAnthropicConverter;
        let input = json!({
            "id": "chatcmpl-123",
            "model": "gpt-4",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "Text"},
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 100,
                "completion_tokens": 50,
                "prompt_tokens_details": {
                    "cached_tokens": 30
                }
            }
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_response(&input_bytes, ApiType::OpenAIChat, ApiType::Anthropic, false).unwrap();
        let output: Value = serde_json::from_slice(&result).unwrap();

        assert_eq!(output["usage"]["input_tokens"], 100);
        assert_eq!(output["usage"]["output_tokens"], 50);
        assert_eq!(output["usage"]["cache_read_input_tokens"], 30);
    }

    #[test]
    fn test_unknown_finish_reason() {
        let converter = OpenAIChatToAnthropicConverter;
        let input = json!({
            "id": "chatcmpl-123",
            "model": "gpt-4",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "Text"},
                "finish_reason": "unknown_reason"
            }]
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_response(&input_bytes, ApiType::OpenAIChat, ApiType::Anthropic, false).unwrap();
        let output: Value = serde_json::from_slice(&result).unwrap();

        // Should default to end_turn
        assert_eq!(output["stop_reason"], "end_turn");
    }

    #[test]
    fn test_error_response_mapping() {
        let converter = OpenAIChatToAnthropicConverter;
        let input = json!({
            "error": {
                "message": "Invalid API key",
                "type": "authentication_error",
                "code": "invalid_api_key"
            }
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_response(&input_bytes, ApiType::OpenAIChat, ApiType::Anthropic, false).unwrap();
        let output: Value = serde_json::from_slice(&result).unwrap();

        assert_eq!(output["type"], "error");
        assert_eq!(output["error"]["type"], "authentication_error");
        assert_eq!(output["error"]["message"], "Invalid API key");
    }

    // Error case tests

    #[test]
    fn test_invalid_json() {
        let converter = OpenAIChatToAnthropicConverter;
        let input_bytes = Bytes::from("{invalid json");

        let result = converter.convert_response(&input_bytes, ApiType::OpenAIChat, ApiType::Anthropic, false);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind, ConversionErrorKind::InvalidJson);
        assert_eq!(err.code, "invalid_json");
        assert!(err.field_path.is_none());
    }

    #[test]
    fn test_missing_choices() {
        let converter = OpenAIChatToAnthropicConverter;
        let input = json!({
            "id": "chatcmpl-123",
            "model": "gpt-4"
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_response(&input_bytes, ApiType::OpenAIChat, ApiType::Anthropic, false);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind, ConversionErrorKind::SchemaMismatch);
        assert_eq!(err.code, "missing_field");
        assert_eq!(err.field_path, Some("response.choices".to_string()));
    }

    #[test]
    fn test_empty_choices() {
        let converter = OpenAIChatToAnthropicConverter;
        let input = json!({
            "id": "chatcmpl-123",
            "model": "gpt-4",
            "choices": []
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_response(&input_bytes, ApiType::OpenAIChat, ApiType::Anthropic, false);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind, ConversionErrorKind::SchemaMismatch);
        assert_eq!(err.code, "missing_field");
        assert_eq!(err.field_path, Some("response.choices".to_string()));
    }

    #[test]
    fn test_missing_message() {
        let converter = OpenAIChatToAnthropicConverter;
        let input = json!({
            "id": "chatcmpl-123",
            "model": "gpt-4",
            "choices": [{
                "index": 0,
                "finish_reason": "stop"
            }]
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_response(&input_bytes, ApiType::OpenAIChat, ApiType::Anthropic, false);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind, ConversionErrorKind::SchemaMismatch);
        assert_eq!(err.code, "missing_field");
        assert_eq!(err.field_path, Some("response.choices[0].message".to_string()));
    }

    #[test]
    fn test_missing_content_and_tool_calls() {
        let converter = OpenAIChatToAnthropicConverter;
        let input = json!({
            "id": "chatcmpl-123",
            "model": "gpt-4",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant"
                },
                "finish_reason": "stop"
            }]
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_response(&input_bytes, ApiType::OpenAIChat, ApiType::Anthropic, false);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind, ConversionErrorKind::SchemaMismatch);
        assert_eq!(err.code, "missing_field");
        assert_eq!(err.field_path, Some("response.choices[0].message".to_string()));
    }

    #[test]
    fn test_streaming_returns_not_implemented() {
        let converter = OpenAIChatToAnthropicConverter;
        let input = json!({
            "id": "chatcmpl-123",
            "model": "gpt-4",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "Text"},
                "finish_reason": "stop"
            }]
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_response(&input_bytes, ApiType::OpenAIChat, ApiType::Anthropic, true);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind, ConversionErrorKind::NotImplemented);
        assert_eq!(err.code, "not_implemented");
    }

    // Request conversion tests

    #[test]
    fn test_simple_text_request() {
        let converter = OpenAIChatToAnthropicConverter;
        let input = json!({
            "model": "gpt-4",
            "messages": [
                {"role": "user", "content": "Hello, world!"}
            ]
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_request(&input_bytes, ApiType::Anthropic).unwrap();
        let output: Value = serde_json::from_slice(&result).unwrap();

        assert_eq!(output["model"], "gpt-4");
        assert_eq!(output["messages"][0]["role"], "user");
        assert_eq!(output["messages"][0]["content"], "Hello, world!");
    }

    #[test]
    fn test_request_with_system_message() {
        let converter = OpenAIChatToAnthropicConverter;
        let input = json!({
            "model": "gpt-4",
            "messages": [
                {"role": "system", "content": "You are a helpful assistant."},
                {"role": "user", "content": "Hello!"}
            ]
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_request(&input_bytes, ApiType::Anthropic).unwrap();
        let output: Value = serde_json::from_slice(&result).unwrap();

        assert_eq!(output["system"], "You are a helpful assistant.");
        assert_eq!(output["messages"][0]["role"], "user");
    }

    #[test]
    fn test_request_with_tools() {
        let converter = OpenAIChatToAnthropicConverter;
        let input = json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Get weather"}],
            "tools": [
                {
                    "type": "function",
                    "function": {
                        "name": "get_weather",
                        "description": "Get current weather",
                        "parameters": {
                            "type": "object",
                            "properties": {
                                "location": {"type": "string"}
                            }
                        }
                    }
                }
            ]
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_request(&input_bytes, ApiType::Anthropic).unwrap();
        let output: Value = serde_json::from_slice(&result).unwrap();

        assert_eq!(output["tools"][0]["name"], "get_weather");
        assert_eq!(output["tools"][0]["description"], "Get current weather");
        assert_eq!(output["tools"][0]["input_schema"]["type"], "object");
    }

    #[test]
    fn test_request_tool_choice_auto() {
        let converter = OpenAIChatToAnthropicConverter;
        let input = json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}],
            "tool_choice": "auto"
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_request(&input_bytes, ApiType::Anthropic).unwrap();
        let output: Value = serde_json::from_slice(&result).unwrap();

        assert_eq!(output["tool_choice"]["type"], "auto");
    }

    #[test]
    fn test_request_tool_choice_required() {
        let converter = OpenAIChatToAnthropicConverter;
        let input = json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}],
            "tool_choice": "required"
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_request(&input_bytes, ApiType::Anthropic).unwrap();
        let output: Value = serde_json::from_slice(&result).unwrap();

        assert_eq!(output["tool_choice"]["type"], "any");
    }

    #[test]
    fn test_request_tool_choice_function() {
        let converter = OpenAIChatToAnthropicConverter;
        let input = json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}],
            "tool_choice": {
                "type": "function",
                "function": {"name": "get_weather"}
            }
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_request(&input_bytes, ApiType::Anthropic).unwrap();
        let output: Value = serde_json::from_slice(&result).unwrap();

        assert_eq!(output["tool_choice"]["type"], "tool");
        assert_eq!(output["tool_choice"]["name"], "get_weather");
    }

    #[test]
    fn test_request_with_tool_calls() {
        let converter = OpenAIChatToAnthropicConverter;
        let input = json!({
            "model": "gpt-4",
            "messages": [
                {"role": "user", "content": "Get weather"},
                {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_abc",
                        "type": "function",
                        "function": {
                            "name": "get_weather",
                            "arguments": "{\"location\":\"SF\"}"
                        }
                    }]
                }
            ]
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_request(&input_bytes, ApiType::Anthropic).unwrap();
        let output: Value = serde_json::from_slice(&result).unwrap();

        assert_eq!(output["messages"][1]["role"], "assistant");
        assert_eq!(output["messages"][1]["content"][0]["type"], "tool_use");
        assert_eq!(output["messages"][1]["content"][0]["name"], "get_weather");
        assert_eq!(output["messages"][1]["content"][0]["input"]["location"], "SF");
    }

    #[test]
    fn test_request_with_tool_result() {
        let converter = OpenAIChatToAnthropicConverter;
        let input = json!({
            "model": "gpt-4",
            "messages": [
                {"role": "tool", "tool_call_id": "call_abc", "content": "The weather is sunny"}
            ]
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_request(&input_bytes, ApiType::Anthropic).unwrap();
        let output: Value = serde_json::from_slice(&result).unwrap();

        assert_eq!(output["messages"][0]["role"], "user");
        assert_eq!(output["messages"][0]["content"][0]["type"], "tool_result");
        assert_eq!(output["messages"][0]["content"][0]["tool_use_id"], "call_abc");
    }

    #[test]
    fn test_request_with_user_field() {
        let converter = OpenAIChatToAnthropicConverter;
        let input = json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}],
            "user": "user123"
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_request(&input_bytes, ApiType::Anthropic).unwrap();
        let output: Value = serde_json::from_slice(&result).unwrap();

        assert_eq!(output["metadata"]["user_id"], "user123");
    }

    #[test]
    fn test_request_with_max_completion_tokens() {
        let converter = OpenAIChatToAnthropicConverter;
        let input = json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}],
            "max_completion_tokens": 2048
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_request(&input_bytes, ApiType::Anthropic).unwrap();
        let output: Value = serde_json::from_slice(&result).unwrap();

        assert_eq!(output["max_tokens"], 2048);
    }

    #[test]
    fn test_request_with_stop_string() {
        let converter = OpenAIChatToAnthropicConverter;
        let input = json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}],
            "stop": "END"
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_request(&input_bytes, ApiType::Anthropic).unwrap();
        let output: Value = serde_json::from_slice(&result).unwrap();

        assert_eq!(output["stop_sequences"][0], "END");
    }

    #[test]
    fn test_request_with_parallel_tool_calls_false() {
        let converter = OpenAIChatToAnthropicConverter;
        let input = json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}],
            "parallel_tool_calls": false
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_request(&input_bytes, ApiType::Anthropic).unwrap();
        let output: Value = serde_json::from_slice(&result).unwrap();

        // disable_parallel_tool_use should be nested inside tool_choice
        assert_eq!(output["tool_choice"]["disable_parallel_tool_use"], true);
        assert_eq!(output["tool_choice"]["type"], "auto");
    }

    #[test]
    fn test_request_tool_choice_with_parallel_tool_calls() {
        let converter = OpenAIChatToAnthropicConverter;
        let input = json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}],
            "tool_choice": "auto",
            "parallel_tool_calls": false
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_request(&input_bytes, ApiType::Anthropic).unwrap();
        let output: Value = serde_json::from_slice(&result).unwrap();

        // disable_parallel_tool_use should be nested inside tool_choice
        assert_eq!(output["tool_choice"]["disable_parallel_tool_use"], true);
        assert_eq!(output["tool_choice"]["type"], "auto");
    }

    #[test]
    fn test_request_missing_messages() {
        let converter = OpenAIChatToAnthropicConverter;
        let input = json!({
            "model": "gpt-4"
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_request(&input_bytes, ApiType::Anthropic);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind, ConversionErrorKind::SchemaMismatch);
        assert_eq!(err.code, "missing_field");
        assert_eq!(err.field_path, Some("request.messages".to_string()));
    }

    #[test]
    fn test_request_invalid_json() {
        let converter = OpenAIChatToAnthropicConverter;
        let input_bytes = Bytes::from("{invalid json");

        let result = converter.convert_request(&input_bytes, ApiType::Anthropic);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind, ConversionErrorKind::InvalidJson);
        assert_eq!(err.code, "invalid_json");
    }
}
