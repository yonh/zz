//! Anthropic → OpenAI Chat request body converter
//!
//! Implements conversion from Anthropic Messages API format to OpenAI Chat Completions format.
//! See [field-mapping.md §2](../../../docs/plans/2026-05-04-api-converter-plan/field-mapping.md#2-请求体anthropic--openai-chat-a2o)
//! for detailed mapping rules.

use crate::converter::{
    ApiConverter, ApiType, Bytes, ConversionError, ConversionErrorKind, NoopTelemetry,
    TargetQuirks, TelemetryContext,
};
use serde_json::{json, Map, Value};

/// Known Anthropic request fields for telemetry
const KNOWN_ANTHROPIC_FIELDS: &[&str] = &[
    "model",
    "messages",
    "system",
    "max_tokens",
    "temperature",
    "top_p",
    "top_k",
    "stop_sequences",
    "stream",
    "tools",
    "tool_choice",
    "metadata",
    "anthropic_beta",
    "anthropic_version",
    "service_tier",
];

/// Converter from Anthropic Messages API to OpenAI Chat Completions API
pub struct AnthropicToOpenAIConverter;

impl ApiConverter for AnthropicToOpenAIConverter {
    fn convert_request(&self, body: &Bytes, _target: ApiType) -> Result<Bytes, ConversionError> {
        let ctx = NoopTelemetry;
        self.convert_request_with_ctx(body, TargetQuirks::default(), &ctx)
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
                "AnthropicToOpenAIConverter::convert_response for streaming not implemented yet (P5)",
            ));
        }

        let ctx = NoopTelemetry;
        self.convert_response_with_ctx(body, &ctx)
    }
}

impl AnthropicToOpenAIConverter {
    /// Convert request body with telemetry context
    pub fn convert_request_with_ctx(
        &self,
        body: &Bytes,
        quirks: TargetQuirks,
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
            if !KNOWN_ANTHROPIC_FIELDS.contains(&key.as_str()) {
                ctx.report_unknown_field(&format!("request.{}", key));
            }
        }

        // Build output object
        let mut output = Map::new();

        // Handle required messages field
        let messages = self.convert_messages(&obj, ctx)?;
        if messages.is_empty() {
            return Err(ConversionError::new(
                ConversionErrorKind::SchemaMismatch,
                "missing_field",
                "Missing required field: messages",
            )
            .with_field_path("request.messages"));
        }

        output.insert("messages".to_string(), json!(messages));

        // Handle system field
        if let Some(system) = obj.get("system") {
            let system_msg = self.convert_system(system, ctx)?;
            ctx.report_field_mapped("system", "messages[0]");
            let mut all_messages = Vec::with_capacity(messages.len() + 1);
            all_messages.push(system_msg);
            all_messages.extend(messages);
            output.insert("messages".to_string(), json!(all_messages));
        }

        // Copy model field
        if let Some(model) = obj.get("model") {
            ctx.report_field_mapped("model", "model");
            output.insert("model".to_string(), model.clone());
        }

        // Handle max_tokens (or max_completion_tokens for reasoning models)
        if let Some(max_tokens) = obj.get("max_tokens") {
            let key = if quirks.use_max_completion_tokens {
                "max_completion_tokens"
            } else {
                "max_tokens"
            };
            ctx.report_field_mapped("max_tokens", key);
            output.insert(key.to_string(), max_tokens.clone());
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

        // Skip top_k with telemetry
        if obj.contains_key("top_k") {
            ctx.report_field_skipped("request.top_k", "unsupported_in_target");
        }

        // Copy stop_sequences as stop array
        if let Some(stop_sequences) = obj.get("stop_sequences") {
            if let Some(arr) = stop_sequences.as_array() {
                ctx.report_field_mapped("stop_sequences", "stop");
                output.insert("stop".to_string(), json!(arr));
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
            let (converted, parallel_tool_calls) = self.convert_tool_choice(tool_choice, &obj, ctx)?;
            ctx.report_field_mapped("tool_choice", "tool_choice");
            output.insert("tool_choice".to_string(), converted);
            if let Some(parallel) = parallel_tool_calls {
                output.insert("parallel_tool_calls".to_string(), json!(parallel));
            }
        }

        // Handle metadata.user_id → user
        if let Some(metadata) = obj.get("metadata") {
            if let Some(meta_obj) = metadata.as_object() {
                if let Some(user_id) = meta_obj.get("user_id") {
                    ctx.report_field_mapped("metadata.user_id", "user");
                    output.insert("user".to_string(), user_id.clone());
                }
                // Skip other metadata fields
                for key in meta_obj.keys() {
                    if key != "user_id" {
                        ctx.report_field_skipped(
                            &format!("request.metadata.{}", key),
                            "unsupported_in_target",
                        );
                    }
                }
            }
        }

        // Skip anthropic_beta
        if obj.contains_key("anthropic_beta") {
            ctx.report_field_skipped("request.anthropic_beta", "anthropic_specific");
        }

        // Skip anthropic_version
        if obj.contains_key("anthropic_version") {
            ctx.report_field_skipped("request.anthropic_version", "anthropic_specific");
        }

        // Copy service_tier if present
        if let Some(service_tier) = obj.get("service_tier") {
            ctx.report_field_mapped("service_tier", "service_tier");
            output.insert("service_tier".to_string(), service_tier.clone());
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

    /// Convert system field to OpenAI system message
    fn convert_system(
        &self,
        system: &Value,
        ctx: &dyn TelemetryContext,
    ) -> Result<Value, ConversionError> {
        let content = if let Some(s) = system.as_str() {
            s.to_string()
        } else if let Some(arr) = system.as_array() {
            // Array of {type:"text",text} blocks - concatenate with \n\n
            let mut parts = Vec::new();
            for (i, block) in arr.iter().enumerate() {
                if let Some(obj) = block.as_object() {
                    if obj.get("type").and_then(|t| t.as_str()) == Some("text") {
                        if let Some(text) = obj.get("text").and_then(|t| t.as_str()) {
                            parts.push(text.to_string());
                        }
                    } else {
                        ctx.report_field_skipped(
                            &format!("request.system[{}].type", i),
                            "non_text_block_in_system",
                        );
                    }
                }
            }
            parts.join("\n\n")
        } else {
            return Err(ConversionError::new(
                ConversionErrorKind::SchemaMismatch,
                "bad_type",
                "system must be string or array",
            )
            .with_field_path("request.system"));
        };

        Ok(json!({
            "role": "system",
            "content": content
        }))
    }

    /// Convert messages array
    fn convert_messages(
        &self,
        obj: &Map<String, Value>,
        ctx: &dyn TelemetryContext,
    ) -> Result<Vec<Value>, ConversionError> {
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
        let mut tool_result_index = 0;

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

            // Validate role
            if !matches!(role, "user" | "assistant") {
                return Err(ConversionError::new(
                    ConversionErrorKind::SchemaMismatch,
                    "bad_type",
                    format!("messages[{}].role must be 'user' or 'assistant'", i),
                )
                .with_field_path(format!("request.messages[{}].role", i)));
            }

            let content = msg_obj.get("content");

            // Handle tool_result blocks - they become separate tool messages
            if let Some(content_arr) = content.and_then(|c| c.as_array()) {
                let mut tool_results = Vec::new();
                let mut non_tool_content = Vec::new();

                for (j, block) in content_arr.iter().enumerate() {
                    if let Some(block_obj) = block.as_object() {
                        let block_type = block_obj.get("type").and_then(|t| t.as_str());

                        if block_type == Some("tool_result") {
                            // Extract tool_result and create separate message
                            let tool_use_id = block_obj
                                .get("tool_use_id")
                                .and_then(|id| id.as_str())
                                .ok_or_else(|| {
                                    ConversionError::new(
                                        ConversionErrorKind::SchemaMismatch,
                                        "missing_field",
                                        format!(
                                            "messages[{}].content[{}].tool_result missing tool_use_id",
                                            i, j
                                        ),
                                    )
                                    .with_field_path(format!(
                                        "request.messages[{}].content[{}].tool_use_id",
                                        i, j
                                    ))
                                })?;

                            let result_content = block_obj.get("content").cloned().unwrap_or_else(|| json!(""));

                            tool_results.push(json!({
                                "role": "tool",
                                "tool_call_id": tool_use_id,
                                "content": result_content
                            }));

                            ctx.report_field_mapped(
                                &format!("messages[{}].content[{}]", i, j),
                                &format!("messages[{}+{}]", i, tool_result_index),
                            );
                            tool_result_index += 1;
                        } else if block_type == Some("tool_use") {
                            // tool_use in user message - convert to tool_calls
                            let id = block_obj
                                .get("id")
                                .and_then(|id| id.as_str())
                                .ok_or_else(|| {
                                    ConversionError::new(
                                        ConversionErrorKind::SchemaMismatch,
                                        "missing_field",
                                        format!("messages[{}].content[{}].tool_use missing id", i, j),
                                    )
                                    .with_field_path(format!(
                                        "request.messages[{}].content[{}].id",
                                        i, j
                                    ))
                                })?;

                            let name = block_obj
                                .get("name")
                                .and_then(|name| name.as_str())
                                .ok_or_else(|| {
                                    ConversionError::new(
                                        ConversionErrorKind::SchemaMismatch,
                                        "missing_field",
                                        format!(
                                            "messages[{}].content[{}].tool_use missing name",
                                            i, j
                                        ),
                                    )
                                    .with_field_path(format!(
                                        "request.messages[{}].content[{}].name",
                                        i, j
                                    ))
                                })?;

                            let input = block_obj.get("input").cloned().unwrap_or_else(|| json!({}));

                            non_tool_content.push(json!({
                                "role": role,
                                "content": Value::Null,
                                "tool_calls": [{
                                    "id": id,
                                    "type": "function",
                                    "function": {
                                        "name": name,
                                        "arguments": serde_json::to_string(&input).unwrap_or_else(|_| "{}".to_string())
                                    }
                                }]
                            }));
                        } else if block_type == Some("text") {
                            if let Some(text) = block_obj.get("text").and_then(|t| t.as_str()) {
                                non_tool_content.push(json!(text));
                            }
                        } else if block_type == Some("image") {
                            // Handle image block
                            if let Some(image_obj) = self.convert_image_block(block_obj, i, j, ctx)? {
                                non_tool_content.push(image_obj);
                            }
                        } else {
                            ctx.report_field_skipped(
                                &format!("request.messages[{}].content[{}].type", i, j),
                                "unsupported_block_type",
                            );
                        }
                    }
                }

                // If we have tool_results, add them as separate messages
                for tr in tool_results {
                    result.push(tr);
                }

                // Add non-tool content
                if !non_tool_content.is_empty() {
                    let msg_value = if non_tool_content.len() == 1 {
                        // Single string content
                        if let Some(s) = non_tool_content.first().and_then(|v| v.as_str()) {
                            json!({
                                "role": role,
                                "content": s
                            })
                        } else {
                            // It's an object (image or tool_calls)
                            json!({
                                "role": role,
                                "content": non_tool_content
                            })
                        }
                    } else {
                        // Multiple blocks - use array
                        json!({
                            "role": role,
                            "content": non_tool_content
                        })
                    };
                    result.push(msg_value);
                }
            } else if let Some(content_str) = content.and_then(|c| c.as_str()) {
                // Simple string content
                result.push(json!({
                    "role": role,
                    "content": content_str
                }));
            } else {
                // Empty content or null
                result.push(json!({
                    "role": role,
                    "content": ""
                }));
            }
        }

        Ok(result)
    }

    /// Convert image block to OpenAI format
    /// Returns None if the image source type is unsupported
    fn convert_image_block(
        &self,
        block: &Map<String, Value>,
        msg_idx: usize,
        block_idx: usize,
        ctx: &dyn TelemetryContext,
    ) -> Result<Option<Value>, ConversionError> {
        let source = block.get("source").ok_or_else(|| {
            ConversionError::new(
                ConversionErrorKind::SchemaMismatch,
                "missing_field",
                format!("messages[{}].content[{}].image missing source", msg_idx, block_idx),
            )
            .with_field_path(format!(
                "request.messages[{}].content[{}].source",
                msg_idx, block_idx
            ))
        })?;

        let source_obj = source.as_object().ok_or_else(|| {
            ConversionError::new(
                ConversionErrorKind::SchemaMismatch,
                "bad_type",
                format!("messages[{}].content[{}].source must be object", msg_idx, block_idx),
            )
            .with_field_path(format!(
                "request.messages[{}].content[{}].source",
                msg_idx, block_idx
            ))
        })?;

        let source_type = source_obj.get("type").and_then(|t| t.as_str()).unwrap_or("");

        let image_url = if source_type == "base64" {
            let media_type = source_obj
                .get("media_type")
                .and_then(|mt| mt.as_str())
                .unwrap_or("image/jpeg");
            let data = source_obj.get("data").and_then(|d| d.as_str()).unwrap_or("");
            format!("data:{};base64,{}", media_type, data)
        } else if source_type == "url" {
            source_obj
                .get("url")
                .and_then(|u| u.as_str())
                .unwrap_or("")
                .to_string()
        } else {
            ctx.report_field_skipped(
                &format!("request.messages[{}].content[{}].source.type", msg_idx, block_idx),
                "unsupported_image_source_type",
            );
            return Ok(None); // Skip unsupported image types
        };

        Ok(Some(json!({
            "type": "image_url",
            "image_url": {
                "url": image_url
            }
        })))
    }

    /// Convert tools array
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

            let name = tool_obj.get("name").and_then(|n| n.as_str()).ok_or_else(|| {
                ConversionError::new(
                    ConversionErrorKind::SchemaMismatch,
                    "missing_field",
                    format!("tools[{}] missing name", i),
                )
                .with_field_path(format!("request.tools[{}].name", i))
            })?;

            let description = tool_obj.get("description").and_then(|d| d.as_str());

            let input_schema = tool_obj.get("input_schema").ok_or_else(|| {
                ConversionError::new(
                    ConversionErrorKind::SchemaMismatch,
                    "missing_field",
                    format!("tools[{}] missing input_schema", i),
                )
                .with_field_path(format!("request.tools[{}].input_schema", i))
            })?;

            if !input_schema.is_object() {
                return Err(ConversionError::new(
                    ConversionErrorKind::SchemaMismatch,
                    "bad_type",
                    format!("tools[{}].input_schema must be an object", i),
                )
                .with_field_path(format!("request.tools[{}].input_schema", i)));
            }

            let mut function_obj = json!({
                "name": name,
                "parameters": input_schema
            });

            if let Some(desc) = description {
                function_obj
                    .as_object_mut()
                    .unwrap()
                    .insert("description".to_string(), json!(desc));
            }

            result.push(json!({
                "type": "function",
                "function": function_obj
            }));

            // Skip cache_control if present
            if tool_obj.contains_key("cache_control") {
                ctx.report_field_skipped(
                    &format!("request.tools[{}].cache_control", i),
                    "anthropic_specific",
                );
            }
        }

        Ok(result)
    }

    /// Convert tool_choice
    ///
    /// Returns a tuple of (tool_choice_value, parallel_tool_calls_option)
    /// where parallel_tool_calls_option is Some(false) if disable_parallel_tool_use is true
    fn convert_tool_choice(
        &self,
        tool_choice: &Value,
        _obj: &Map<String, Value>,
        ctx: &dyn TelemetryContext,
    ) -> Result<(Value, Option<bool>), ConversionError> {
        // Check for disable_parallel_tool_use (nested in tool_choice object per Anthropic spec)
        let disable_parallel = if let Some(tc_obj) = tool_choice.as_object() {
            if let Some(disable) = tc_obj.get("disable_parallel_tool_use").and_then(|d| d.as_bool()) {
                if disable {
                    ctx.report_field_mapped(
                        "tool_choice.disable_parallel_tool_use",
                        "parallel_tool_calls",
                    );
                    Some(false)
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
                "auto" => return Ok((json!("auto"), disable_parallel)),
                "any" => return Ok((json!("required"), disable_parallel)),
                "none" => return Ok((json!("none"), disable_parallel)),
                _ => {}
            }
        }

        if let Some(tc_obj) = tool_choice.as_object() {
            let tc_type = tc_obj.get("type").and_then(|t| t.as_str());

            if tc_type == Some("auto") {
                return Ok((json!("auto"), disable_parallel));
            } else if tc_type == Some("any") {
                return Ok((json!("required"), disable_parallel));
            } else if tc_type == Some("none") {
                return Ok((json!("none"), disable_parallel));
            } else if tc_type == Some("tool") {
                let name = tc_obj.get("name").and_then(|n| n.as_str()).ok_or_else(|| {
                    ConversionError::new(
                        ConversionErrorKind::SchemaMismatch,
                        "missing_field",
                        "tool_choice.tool missing name",
                    )
                    .with_field_path("request.tool_choice.name")
                })?;

                return Ok((
                    json!({
                        "type": "function",
                        "function": {
                            "name": name
                        }
                    }),
                    disable_parallel,
                ));
            }
        }

        // Default to auto
        Ok((json!("auto"), disable_parallel))
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
        if let Some(error_type) = obj.get("type").and_then(|t| t.as_str()) {
            if error_type == "error" {
                return self.convert_error_response(&obj, ctx);
            }
        }

        // Report unknown fields
        for key in obj.keys() {
            if !["type", "id", "role", "content", "model", "stop_reason", "stop_sequence", "usage"]
                .contains(&key.as_str())
            {
                ctx.report_unknown_field(&format!("response.{}", key));
            }
        }

        // Build OpenAI response
        let mut output = Map::new();

        // Copy id
        if let Some(id) = obj.get("id") {
            ctx.report_field_mapped("id", "id");
            output.insert("id".to_string(), id.clone());
        } else {
            // Generate a fallback id if missing
            output.insert("id".to_string(), json!(format!("chatcmpl-{}", uuid::Uuid::new_v4())));
        }

        // object is always "chat.completion"
        output.insert("object".to_string(), json!("chat.completion"));

        // created timestamp (use current time if not present)
        output.insert("created".to_string(), json!(chrono::Utc::now().timestamp()));

        // Copy model
        if let Some(model) = obj.get("model") {
            ctx.report_field_mapped("model", "model");
            output.insert("model".to_string(), model.clone());
        }

        // Convert content to choices
        let content = obj.get("content");
        let stop_reason = obj.get("stop_reason");

        let mut choice = Map::new();
        choice.insert("index".to_string(), json!(0));

        let mut message = Map::new();
        message.insert("role".to_string(), json!("assistant"));

        // Convert content
        if let Some(content_arr) = content.and_then(|c| c.as_array()) {
            let mut text_content = String::new();
            let mut tool_calls = Vec::new();

            for (i, block) in content_arr.iter().enumerate() {
                if let Some(block_obj) = block.as_object() {
                    let block_type = block_obj.get("type").and_then(|t| t.as_str());

                    if block_type == Some("text") {
                        if let Some(text) = block_obj.get("text").and_then(|t| t.as_str()) {
                            text_content.push_str(text);
                        }
                    } else if block_type == Some("tool_use") {
                        // Convert tool_use to OpenAI tool_calls
                        let id = block_obj.get("id").and_then(|id| id.as_str()).unwrap_or("");
                        let name = block_obj.get("name").and_then(|n| n.as_str()).unwrap_or("");
                        let input = block_obj.get("input").cloned().unwrap_or_else(|| json!({}));

                        tool_calls.push(json!({
                            "id": id,
                            "type": "function",
                            "function": {
                                "name": name,
                                "arguments": serde_json::to_string(&input).unwrap_or_else(|_| "{}".to_string())
                            }
                        }));

                        ctx.report_field_mapped(
                            &format!("content[{}]", i),
                            "choices[0].message.tool_calls",
                        );
                    } else {
                        ctx.report_field_skipped(
                            &format!("response.content[{}].type", i),
                            "unsupported_content_type",
                        );
                    }
                }
            }

            if !text_content.is_empty() {
                message.insert("content".to_string(), json!(text_content));
                ctx.report_field_mapped("content", "choices[0].message.content");
            } else if tool_calls.is_empty() {
                message.insert("content".to_string(), json!(""));
            }

            if !tool_calls.is_empty() {
                message.insert("tool_calls".to_string(), json!(tool_calls));
            }
        } else if let Some(content_str) = content.and_then(|c| c.as_str()) {
            message.insert("content".to_string(), json!(content_str));
            ctx.report_field_mapped("content", "choices[0].message.content");
        } else {
            message.insert("content".to_string(), json!(""));
        }

        choice.insert("message".to_string(), json!(message));

        // Convert stop_reason to finish_reason
        if let Some(reason) = stop_reason.and_then(|r| r.as_str()) {
            let finish_reason = match reason {
                "end_turn" => "stop",
                "max_tokens" => "length",
                "tool_use" => "tool_calls",
                "stop_sequence" => "stop",
                _ => {
                    ctx.report_field_skipped("response.stop_reason", "unknown_stop_reason");
                    "stop"
                }
            };
            ctx.report_field_mapped("stop_reason", "choices[0].finish_reason");
            choice.insert("finish_reason".to_string(), json!(finish_reason));
        } else {
            choice.insert("finish_reason".to_string(), json!("stop"));
        }

        output.insert("choices".to_string(), json!([choice]));

        // Convert usage
        if let Some(usage) = obj.get("usage") {
            let converted_usage = self.convert_usage(usage, ctx);
            output.insert("usage".to_string(), json!(converted_usage));
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

    /// Convert usage field from Anthropic to OpenAI format
    fn convert_usage(&self, usage: &Value, ctx: &dyn TelemetryContext) -> Value {
        let mut result = Map::new();

        if let Some(input_tokens) = usage.get("input_tokens") {
            ctx.report_field_mapped("usage.input_tokens", "usage.prompt_tokens");
            result.insert("prompt_tokens".to_string(), input_tokens.clone());
        }

        if let Some(output_tokens) = usage.get("output_tokens") {
            ctx.report_field_mapped("usage.output_tokens", "usage.completion_tokens");
            result.insert("completion_tokens".to_string(), output_tokens.clone());
        }

        if let Some(cache_read) = usage.get("cache_read_input_tokens") {
            ctx.report_field_mapped(
                "usage.cache_read_input_tokens",
                "usage.prompt_tokens_details.cached_tokens",
            );
            let mut details = Map::new();
            details.insert("cached_tokens".to_string(), cache_read.clone());
            result.insert("prompt_tokens_details".to_string(), json!(details));
        }

        json!(result)
    }

    /// Convert Anthropic error response to OpenAI error format
    fn convert_error_response(
        &self,
        input: &Map<String, Value>,
        ctx: &dyn TelemetryContext,
    ) -> Result<Bytes, ConversionError> {
        let error = input.get("error").ok_or_else(|| {
            ConversionError::new(
                ConversionErrorKind::SchemaMismatch,
                "missing_field",
                "error field missing in error response",
            )
            .with_field_path("response.error")
        })?;

        let error_obj = error.as_object().ok_or_else(|| {
            ConversionError::new(
                ConversionErrorKind::SchemaMismatch,
                "bad_type",
                "error must be an object",
            )
            .with_field_path("response.error")
        })?;

        let error_type = error_obj
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("api_error");
        let error_message = error_obj
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("Unknown error");

        // Map error type per field-mapping.md §5 (reverse mapping)
        let openai_type = match error_type {
            "invalid_request_error" => "invalid_request_error",
            "authentication_error" => "authentication_error",
            "permission_error" => "permission_error",
            "not_found_error" => "not_found_error",
            "rate_limit_error" => "rate_limit_exceeded",
            "api_error" | _ => {
                if error_type != "api_error" {
                    ctx.report_field_skipped("response.error.type", "unknown_error_type");
                }
                "api_error"
            }
        };

        let output = json!({
            "error": {
                "message": error_message,
                "type": openai_type
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
    fn test_simple_text_request() {
        let converter = AnthropicToOpenAIConverter;
        let input = json!({
            "model": "claude-3-5-sonnet-20241022",
            "max_tokens": 1024,
            "messages": [
                {"role": "user", "content": "Hello, world!"}
            ]
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_request(&input_bytes, ApiType::OpenAIChat).unwrap();
        let output: Value = serde_json::from_slice(&result).unwrap();

        assert_eq!(output["model"], "claude-3-5-sonnet-20241022");
        assert_eq!(output["max_tokens"], 1024);
        assert_eq!(output["messages"][0]["role"], "user");
        assert_eq!(output["messages"][0]["content"], "Hello, world!");
    }

    #[test]
    fn test_system_string() {
        let converter = AnthropicToOpenAIConverter;
        let input = json!({
            "model": "claude-3-5-sonnet-20241022",
            "max_tokens": 1024,
            "system": "You are a helpful assistant.",
            "messages": [
                {"role": "user", "content": "Hello!"}
            ]
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_request(&input_bytes, ApiType::OpenAIChat).unwrap();
        let output: Value = serde_json::from_slice(&result).unwrap();

        assert_eq!(output["messages"][0]["role"], "system");
        assert_eq!(output["messages"][0]["content"], "You are a helpful assistant.");
        assert_eq!(output["messages"][1]["role"], "user");
    }

    #[test]
    fn test_system_array() {
        let converter = AnthropicToOpenAIConverter;
        let input = json!({
            "model": "claude-3-5-sonnet-20241022",
            "max_tokens": 1024,
            "system": [
                {"type": "text", "text": "First part"},
                {"type": "text", "text": "Second part"}
            ],
            "messages": [
                {"role": "user", "content": "Hello!"}
            ]
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_request(&input_bytes, ApiType::OpenAIChat).unwrap();
        let output: Value = serde_json::from_slice(&result).unwrap();

        assert_eq!(output["messages"][0]["role"], "system");
        assert_eq!(output["messages"][0]["content"], "First part\n\nSecond part");
    }

    #[test]
    fn test_tools_conversion() {
        let converter = AnthropicToOpenAIConverter;
        let input = json!({
            "model": "claude-3-5-sonnet-20241022",
            "max_tokens": 1024,
            "messages": [{"role": "user", "content": "Get weather"}],
            "tools": [
                {
                    "name": "get_weather",
                    "description": "Get current weather",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "location": {"type": "string"}
                        }
                    }
                }
            ]
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_request(&input_bytes, ApiType::OpenAIChat).unwrap();
        let output: Value = serde_json::from_slice(&result).unwrap();

        assert_eq!(output["tools"][0]["type"], "function");
        assert_eq!(output["tools"][0]["function"]["name"], "get_weather");
        assert_eq!(
            output["tools"][0]["function"]["description"],
            "Get current weather"
        );
        assert_eq!(output["tools"][0]["function"]["parameters"]["type"], "object");
    }

    #[test]
    fn test_tool_choice_auto() {
        let converter = AnthropicToOpenAIConverter;
        let input = json!({
            "model": "claude-3-5-sonnet-20241022",
            "max_tokens": 1024,
            "messages": [{"role": "user", "content": "Hello"}],
            "tool_choice": {"type": "auto"}
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_request(&input_bytes, ApiType::OpenAIChat).unwrap();
        let output: Value = serde_json::from_slice(&result).unwrap();

        assert_eq!(output["tool_choice"], "auto");
    }

    #[test]
    fn test_tool_choice_any() {
        let converter = AnthropicToOpenAIConverter;
        let input = json!({
            "model": "claude-3-5-sonnet-20241022",
            "max_tokens": 1024,
            "messages": [{"role": "user", "content": "Hello"}],
            "tool_choice": {"type": "any"}
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_request(&input_bytes, ApiType::OpenAIChat).unwrap();
        let output: Value = serde_json::from_slice(&result).unwrap();

        assert_eq!(output["tool_choice"], "required");
    }

    #[test]
    fn test_tool_choice_tool() {
        let converter = AnthropicToOpenAIConverter;
        let input = json!({
            "model": "claude-3-5-sonnet-20241022",
            "max_tokens": 1024,
            "messages": [{"role": "user", "content": "Hello"}],
            "tool_choice": {"type": "tool", "name": "get_weather"}
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_request(&input_bytes, ApiType::OpenAIChat).unwrap();
        let output: Value = serde_json::from_slice(&result).unwrap();

        assert_eq!(output["tool_choice"]["type"], "function");
        assert_eq!(output["tool_choice"]["function"]["name"], "get_weather");
    }

    #[test]
    fn test_max_tokens_with_quirks() {
        let converter = AnthropicToOpenAIConverter;
        let input = json!({
            "model": "claude-3-5-sonnet-20241022",
            "max_tokens": 1024,
            "messages": [{"role": "user", "content": "Hello"}]
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let ctx = NoopTelemetry;

        // Default quirks
        let result = converter
            .convert_request_with_ctx(&input_bytes, TargetQuirks::default(), &ctx)
            .unwrap();
        let output: Value = serde_json::from_slice(&result).unwrap();
        assert!(output.get("max_tokens").is_some());
        assert!(output.get("max_completion_tokens").is_none());

        // Reasoning model quirks
        let result = converter
            .convert_request_with_ctx(
                &input_bytes,
                TargetQuirks {
                    use_max_completion_tokens: true,
                },
                &ctx,
            )
            .unwrap();
        let output: Value = serde_json::from_slice(&result).unwrap();
        assert!(output.get("max_completion_tokens").is_some());
        assert!(output.get("max_tokens").is_none());
    }

    #[test]
    fn test_metadata_user_id() {
        let converter = AnthropicToOpenAIConverter;
        let input = json!({
            "model": "claude-3-5-sonnet-20241022",
            "max_tokens": 1024,
            "messages": [{"role": "user", "content": "Hello"}],
            "metadata": {"user_id": "user123"}
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_request(&input_bytes, ApiType::OpenAIChat).unwrap();
        let output: Value = serde_json::from_slice(&result).unwrap();

        assert_eq!(output["user"], "user123");
    }

    #[test]
    fn test_stop_sequences() {
        let converter = AnthropicToOpenAIConverter;
        let input = json!({
            "model": "claude-3-5-sonnet-20241022",
            "max_tokens": 1024,
            "messages": [{"role": "user", "content": "Hello"}],
            "stop_sequences": ["\n\nHuman:", "\n\nAssistant:"]
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_request(&input_bytes, ApiType::OpenAIChat).unwrap();
        let output: Value = serde_json::from_slice(&result).unwrap();

        assert_eq!(output["stop"][0], "\n\nHuman:");
        assert_eq!(output["stop"][1], "\n\nAssistant:");
    }

    // Error case tests

    #[test]
    fn test_invalid_json() {
        let converter = AnthropicToOpenAIConverter;
        let input_bytes = Bytes::from("{invalid json");

        let result = converter.convert_request(&input_bytes, ApiType::OpenAIChat);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind, ConversionErrorKind::InvalidJson);
        assert_eq!(err.code, "invalid_json");
        assert!(err.field_path.is_none());
    }

    #[test]
    fn test_missing_messages() {
        let converter = AnthropicToOpenAIConverter;
        let input = json!({
            "model": "claude-3-5-sonnet-20241022",
            "max_tokens": 1024
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_request(&input_bytes, ApiType::OpenAIChat);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind, ConversionErrorKind::SchemaMismatch);
        assert_eq!(err.code, "missing_field");
        assert_eq!(err.field_path, Some("request.messages".to_string()));
    }

    #[test]
    fn test_messages_not_array() {
        let converter = AnthropicToOpenAIConverter;
        let input = json!({
            "model": "claude-3-5-sonnet-20241022",
            "max_tokens": 1024,
            "messages": "not an array"
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_request(&input_bytes, ApiType::OpenAIChat);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind, ConversionErrorKind::SchemaMismatch);
        assert_eq!(err.code, "bad_type");
        assert_eq!(err.field_path, Some("request.messages".to_string()));
    }

    #[test]
    fn test_invalid_role() {
        let converter = AnthropicToOpenAIConverter;
        let input = json!({
            "model": "claude-3-5-sonnet-20241022",
            "max_tokens": 1024,
            "messages": [{"role": "invalid", "content": "Hello"}]
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_request(&input_bytes, ApiType::OpenAIChat);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind, ConversionErrorKind::SchemaMismatch);
        assert_eq!(err.code, "bad_type");
        assert_eq!(err.field_path, Some("request.messages[0].role".to_string()));
    }

    #[test]
    fn test_tools_input_schema_not_object() {
        let converter = AnthropicToOpenAIConverter;
        let input = json!({
            "model": "claude-3-5-sonnet-20241022",
            "max_tokens": 1024,
            "messages": [{"role": "user", "content": "Hello"}],
            "tools": [{"name": "test", "input_schema": "not an object"}]
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_request(&input_bytes, ApiType::OpenAIChat);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind, ConversionErrorKind::SchemaMismatch);
        assert_eq!(err.code, "bad_type");
        assert_eq!(err.field_path, Some("request.tools[0].input_schema".to_string()));
    }

    #[test]
    fn test_system_bad_type() {
        let converter = AnthropicToOpenAIConverter;
        let input = json!({
            "model": "claude-3-5-sonnet-20241022",
            "max_tokens": 1024,
            "system": 123,
            "messages": [{"role": "user", "content": "Hello"}]
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_request(&input_bytes, ApiType::OpenAIChat);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind, ConversionErrorKind::SchemaMismatch);
        assert_eq!(err.code, "bad_type");
        assert_eq!(err.field_path, Some("request.system".to_string()));
    }

    // Response conversion tests

    #[test]
    fn test_simple_text_response() {
        let converter = AnthropicToOpenAIConverter;
        let input = json!({
            "type": "message",
            "id": "msg_123",
            "role": "assistant",
            "content": [
                {"type": "text", "text": "Hello, world!"}
            ],
            "model": "claude-3-5-sonnet-20241022",
            "stop_reason": "end_turn",
            "stop_sequence": null,
            "usage": {
                "input_tokens": 10,
                "output_tokens": 5
            }
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_response(&input_bytes, ApiType::Anthropic, ApiType::OpenAIChat, false).unwrap();
        let output: Value = serde_json::from_slice(&result).unwrap();

        assert_eq!(output["id"], "msg_123");
        assert_eq!(output["object"], "chat.completion");
        assert_eq!(output["model"], "claude-3-5-sonnet-20241022");
        assert_eq!(output["choices"][0]["index"], 0);
        assert_eq!(output["choices"][0]["message"]["role"], "assistant");
        assert_eq!(output["choices"][0]["message"]["content"], "Hello, world!");
        assert_eq!(output["choices"][0]["finish_reason"], "stop");
        assert_eq!(output["usage"]["prompt_tokens"], 10);
        assert_eq!(output["usage"]["completion_tokens"], 5);
    }

    #[test]
    fn test_response_with_tool_use() {
        let converter = AnthropicToOpenAIConverter;
        let input = json!({
            "type": "message",
            "id": "msg_123",
            "role": "assistant",
            "content": [
                {"type": "text", "text": "Let me check the weather"},
                {"type": "tool_use", "id": "toolu_123", "name": "get_weather", "input": {"location": "SF"}}
            ],
            "model": "claude-3-5-sonnet-20241022",
            "stop_reason": "tool_use",
            "usage": {"input_tokens": 10, "output_tokens": 20}
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_response(&input_bytes, ApiType::Anthropic, ApiType::OpenAIChat, false).unwrap();
        let output: Value = serde_json::from_slice(&result).unwrap();

        assert_eq!(output["choices"][0]["message"]["content"], "Let me check the weather");
        assert_eq!(output["choices"][0]["message"]["tool_calls"][0]["id"], "toolu_123");
        assert_eq!(output["choices"][0]["message"]["tool_calls"][0]["type"], "function");
        assert_eq!(output["choices"][0]["message"]["tool_calls"][0]["function"]["name"], "get_weather");
        assert_eq!(output["choices"][0]["finish_reason"], "tool_calls");
    }

    #[test]
    fn test_response_max_tokens_stop_reason() {
        let converter = AnthropicToOpenAIConverter;
        let input = json!({
            "type": "message",
            "id": "msg_123",
            "role": "assistant",
            "content": [{"type": "text", "text": "Text"}],
            "model": "claude-3-5-sonnet-20241022",
            "stop_reason": "max_tokens",
            "usage": {"input_tokens": 10, "output_tokens": 5}
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_response(&input_bytes, ApiType::Anthropic, ApiType::OpenAIChat, false).unwrap();
        let output: Value = serde_json::from_slice(&result).unwrap();

        assert_eq!(output["choices"][0]["finish_reason"], "length");
    }

    #[test]
    fn test_response_with_cached_tokens() {
        let converter = AnthropicToOpenAIConverter;
        let input = json!({
            "type": "message",
            "id": "msg_123",
            "role": "assistant",
            "content": [{"type": "text", "text": "Text"}],
            "model": "claude-3-5-sonnet-20241022",
            "stop_reason": "end_turn",
            "usage": {
                "input_tokens": 100,
                "output_tokens": 50,
                "cache_read_input_tokens": 30
            }
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_response(&input_bytes, ApiType::Anthropic, ApiType::OpenAIChat, false).unwrap();
        let output: Value = serde_json::from_slice(&result).unwrap();

        assert_eq!(output["usage"]["prompt_tokens"], 100);
        assert_eq!(output["usage"]["completion_tokens"], 50);
        assert_eq!(output["usage"]["prompt_tokens_details"]["cached_tokens"], 30);
    }

    #[test]
    fn test_response_error_mapping() {
        let converter = AnthropicToOpenAIConverter;
        let input = json!({
            "type": "error",
            "error": {
                "type": "invalid_request_error",
                "message": "Invalid request"
            }
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_response(&input_bytes, ApiType::Anthropic, ApiType::OpenAIChat, false).unwrap();
        let output: Value = serde_json::from_slice(&result).unwrap();

        assert_eq!(output["error"]["type"], "invalid_request_error");
        assert_eq!(output["error"]["message"], "Invalid request");
    }

    #[test]
    fn test_response_rate_limit_error_mapping() {
        let converter = AnthropicToOpenAIConverter;
        let input = json!({
            "type": "error",
            "error": {
                "type": "rate_limit_error",
                "message": "Rate limit exceeded"
            }
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_response(&input_bytes, ApiType::Anthropic, ApiType::OpenAIChat, false).unwrap();
        let output: Value = serde_json::from_slice(&result).unwrap();

        assert_eq!(output["error"]["type"], "rate_limit_exceeded");
    }

    #[test]
    fn test_response_missing_id_generates_fallback() {
        let converter = AnthropicToOpenAIConverter;
        let input = json!({
            "type": "message",
            "role": "assistant",
            "content": [{"type": "text", "text": "Text"}],
            "model": "claude-3-5-sonnet-20241022",
            "stop_reason": "end_turn"
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_response(&input_bytes, ApiType::Anthropic, ApiType::OpenAIChat, false).unwrap();
        let output: Value = serde_json::from_slice(&result).unwrap();

        // Should have generated an id
        assert!(output["id"].is_string());
        assert!(output["id"].as_str().unwrap().starts_with("chatcmpl-"));
    }

    #[test]
    fn test_response_streaming_not_implemented() {
        let converter = AnthropicToOpenAIConverter;
        let input = json!({
            "type": "message",
            "role": "assistant",
            "content": [{"type": "text", "text": "Text"}]
        });

        let input_bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
        let result = converter.convert_response(&input_bytes, ApiType::Anthropic, ApiType::OpenAIChat, true);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind, ConversionErrorKind::NotImplemented);
        assert_eq!(err.code, "not_implemented");
    }
}
