use std::collections::HashMap;
use bytes::Bytes;
use serde_json::{json, Value};

use crate::converter::ApiType;

/// Stream converter for SSE bidirectional conversion
pub struct StreamConverter {
    source: ApiType,
    target: ApiType,
    state: StreamState,
    buffer: String,
}

enum StreamState {
    OpenAIToAnthropic(OAToAnState),
    AnthropicToOpenAI(AnToOAState),
}

struct OAToAnState {
    message_id: Option<String>,
    model: Option<String>,
    started: bool,
    text_block_open: bool,
    text_block_index: Option<u32>,
    next_block_index: u32,
    tool_blocks: HashMap<u32, ToolBlockState>,
    cumulative_input_tokens: Option<u64>,
    cumulative_output_tokens: u64,
    finished: bool,
}

struct ToolBlockState {
    id: Option<String>,
    name: Option<String>,
    arguments_buffer: String,
    open: bool,
}

struct AnToOAState {
    message_id: Option<String>,
    model: Option<String>,
    started: bool,
    content_buffer: String,
    tool_call_index: u32,
    tool_calls: HashMap<u32, ToolCallState>,
    finished: bool,
}

struct ToolCallState {
    id: Option<String>,
    name: Option<String>,
    arguments_buffer: String,
}

impl StreamConverter {
    pub fn new(source: ApiType, target: ApiType) -> Self {
        let state = match (source, target) {
            (ApiType::OpenAIChat, ApiType::Anthropic) => {
                StreamState::OpenAIToAnthropic(OAToAnState::new())
            }
            (ApiType::Anthropic, ApiType::OpenAIChat) => {
                StreamState::AnthropicToOpenAI(AnToOAState::new())
            }
            _ => panic!("Unsupported conversion direction for streaming"),
        };

        Self {
            source,
            target,
            state,
            buffer: String::new(),
        }
    }

    /// Push a chunk of SSE data and return converted chunks
    pub fn push(&mut self, chunk: &[u8]) -> Result<Vec<Bytes>, StreamError> {
        // Append chunk to buffer
        self.buffer.push_str(std::str::from_utf8(chunk).unwrap_or(""));
        
        let mut output = Vec::new();
        let mut events = Vec::new();
        
        // Split by \n\n to get SSE events
        while let Some(event_end) = self.buffer.find("\n\n") {
            let event = self.buffer[..event_end].to_string();
            self.buffer = self.buffer[event_end + 2..].to_string();
            
            if !event.trim().is_empty() {
                events.push(event);
            }
        }
        
        // Process each event
        for event in events {
            match self.process_event(&event) {
                Ok(Some(output_chunk)) => output.push(output_chunk),
                Ok(None) => {},
                Err(e) => {
                    // Parse failure: pass through with warning
                    tracing::warn!(error = ?e, "SSE parse error, passing through");
                    output.push(Bytes::from(format!("{}\n\n", event)));
                }
            }
        }
        
        Ok(output)
    }

    /// Finalize the stream (called on [DONE] or stream end)
    pub fn finalize(&mut self) -> Result<Vec<Bytes>, StreamError> {
        match &mut self.state {
            StreamState::OpenAIToAnthropic(state) => {
                state.finalize()
            }
            StreamState::AnthropicToOpenAI(state) => {
                state.finalize()
            }
        }
    }

    fn process_event(&mut self, event: &str) -> Result<Option<Bytes>, StreamError> {
        // Parse event lines
        let lines: Vec<&str> = event.lines().collect();
        let mut event_type = None;
        let mut data = None;
        
        for line in lines {
            if let Some(rest) = line.strip_prefix("event:") {
                event_type = Some(rest.trim());
            } else if let Some(rest) = line.strip_prefix("data:") {
                data = Some(rest.trim());
            }
        }
        
        let data = data.ok_or_else(|| StreamError::Parse("No data line in event".to_string()))?;
        
        // Check for [DONE]
        if data == "[DONE]" {
            return Ok(Some(self.finalize()?.into_iter().next().unwrap_or_else(|| Bytes::from("data: [DONE]\n\n"))));
        }
        
        // Parse JSON data
        let data_json: Value = serde_json::from_str(data)
            .map_err(|e| StreamError::Parse(format!("Failed to parse JSON: {}", e)))?;
        
        match &mut self.state {
            StreamState::OpenAIToAnthropic(state) => {
                state.process_openai_event(event_type, &data_json)
            }
            StreamState::AnthropicToOpenAI(state) => {
                state.process_anthropic_event(event_type, &data_json)
            }
        }
    }
}

impl OAToAnState {
    fn new() -> Self {
        Self {
            message_id: None,
            model: None,
            started: false,
            text_block_open: false,
            text_block_index: None,
            next_block_index: 0,
            tool_blocks: HashMap::new(),
            cumulative_input_tokens: None,
            cumulative_output_tokens: 0,
            finished: false,
        }
    }

    fn process_openai_event(&mut self, _event_type: Option<&str>, data: &Value) -> Result<Option<Bytes>, StreamError> {
        // Extract id and model from first chunk
        if !self.started {
            if let Some(id) = data.get("id").and_then(|v| v.as_str()) {
                self.message_id = Some(id.to_string());
            }
            if let Some(model) = data.get("model").and_then(|v| v.as_str()) {
                self.model = Some(model.to_string());
            }
            
            // Send message_start
            self.started = true;
            let output = json!({
                "type": "message_start",
                "message": {
                    "id": self.message_id,
                    "model": self.model,
                    "role": "assistant",
                    "content": [],
                    "usage": {
                        "input_tokens": self.cumulative_input_tokens.unwrap_or(0),
                        "output_tokens": 0
                    }
                }
            });
            return Ok(Some(Bytes::from(format!("event: message_start\ndata: {}\n\n", output))));
        }
        
        // Process choices
        if let Some(choices) = data.get("choices").and_then(|v| v.as_array()) {
            if let Some(choice) = choices.first() {
                if let Some(delta) = choice.get("delta") {
                    // Process content delta
                    if let Some(content) = delta.get("content").and_then(|v| v.as_str()) {
                        if !content.is_empty() {
                            if !self.text_block_open {
                                // Start text block
                                self.text_block_open = true;
                                self.text_block_index = Some(self.next_block_index);
                                self.next_block_index += 1;
                                
                                let start_output = json!({
                                    "type": "content_block_start",
                                    "index": self.text_block_index,
                                    "content_block": {
                                        "type": "text",
                                        "text": ""
                                    }
                                });
                                let delta_output = json!({
                                    "type": "content_block_delta",
                                    "index": self.text_block_index,
                                    "delta": {
                                        "type": "text_delta",
                                        "text": content
                                    }
                                });
                                return Ok(Some(Bytes::from(format!(
                                    "event: content_block_start\ndata: {}\n\nevent: content_block_delta\ndata: {}\n\n",
                                    start_output, delta_output
                                ))));
                            } else {
                                // Continue text block
                                let delta_output = json!({
                                    "type": "content_block_delta",
                                    "index": self.text_block_index,
                                    "delta": {
                                        "type": "text_delta",
                                        "text": content
                                    }
                                });
                                return Ok(Some(Bytes::from(format!("event: content_block_delta\ndata: {}\n\n", delta_output))));
                            }
                        }
                    }
                    
                    // Process tool_calls
                    if let Some(tool_calls) = delta.get("tool_calls").and_then(|v| v.as_array()) {
                        for tool_call in tool_calls {
                            if let Some(index) = tool_call.get("index").and_then(|v| v.as_u64()).map(|v| v as u32) {
                                if let Some(function) = tool_call.get("function") {
                                    let index = index as u32;
                                    
                                    if !self.tool_blocks.contains_key(&index) {
                                        // Close text block if open
                                        if self.text_block_open {
                                            self.text_block_open = false;
                                            let stop_output = json!({
                                                "type": "content_block_stop",
                                                "index": self.text_block_index
                                            });
                                            return Ok(Some(Bytes::from(format!("event: content_block_stop\ndata: {}\n\n", stop_output))));
                                        }
                                        
                                        // Start new tool block
                                        let name = function.get("name").and_then(|v| v.as_str()).unwrap_or("");
                                        let id = tool_call.get("id").and_then(|v| v.as_str());
                                        
                                        self.tool_blocks.insert(index, ToolBlockState {
                                            id: id.map(|s| s.to_string()),
                                            name: Some(name.to_string()),
                                            arguments_buffer: String::new(),
                                            open: true,
                                        });
                                        
                                        let start_output = json!({
                                            "type": "content_block_start",
                                            "index": self.next_block_index,
                                            "content_block": {
                                                "type": "tool_use",
                                                "id": id.unwrap_or(""),
                                                "name": name,
                                                "input": {}
                                            }
                                        });
                                        self.next_block_index += 1;
                                        return Ok(Some(Bytes::from(format!("event: content_block_start\ndata: {}\n\n", start_output))));
                                    } else {
                                        // Continue tool block
                                        if let Some(arguments) = function.get("arguments").and_then(|v| v.as_str()) {
                                            if let Some(block) = self.tool_blocks.get_mut(&index) {
                                                block.arguments_buffer.push_str(arguments);
                                                
                                                let delta_output = json!({
                                                    "type": "content_block_delta",
                                                    "index": index,
                                                    "delta": {
                                                        "type": "input_json_delta",
                                                        "partial_json": arguments
                                                    }
                                                });
                                                return Ok(Some(Bytes::from(format!("event: content_block_delta\ndata: {}\n\n", delta_output))));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                
                // Check for finish_reason
                if let Some(finish_reason) = choice.get("finish_reason").and_then(|v| v.as_str()) {
                    return self.finish_with_reason(finish_reason);
                }
            }
        }
        
        // Update usage if present
        if let Some(usage) = data.get("usage") {
            if let Some(output_tokens) = usage.get("completion_tokens").and_then(|v| v.as_u64()) {
                self.cumulative_output_tokens = output_tokens;
            }
        }
        
        Ok(None)
    }

    fn finish_with_reason(&mut self, reason: &str) -> Result<Option<Bytes>, StreamError> {
        if self.finished {
            return Ok(None);
        }
        
        self.finished = true;
        
        let stop_reason = match reason {
            "stop" => "end_turn",
            "length" => "max_tokens",
            "tool_calls" | "function_call" => "tool_use",
            "content_filter" => "end_turn",
            _ => "end_turn",
        };
        
        let mut output = String::new();
        
        // Close text block if open
        if self.text_block_open {
            self.text_block_open = false;
            let stop_output = json!({
                "type": "content_block_stop",
                "index": self.text_block_index
            });
            output.push_str(&format!("event: content_block_stop\ndata: {}\n\n", stop_output));
        }
        
        // Close all tool blocks
        for (index, block) in &self.tool_blocks {
            if block.open {
                let stop_output = json!({
                    "type": "content_block_stop",
                    "index": index
                });
                output.push_str(&format!("event: content_block_stop\ndata: {}\n\n", stop_output));
            }
        }
        
        // Send message_delta with stop_reason
        let delta_output = json!({
            "type": "message_delta",
            "delta": {
                "stop_reason": stop_reason,
                "stop_sequence": null
            },
            "usage": {
                "output_tokens": self.cumulative_output_tokens
            }
        });
        output.push_str(&format!("event: message_delta\ndata: {}\n\n", delta_output));
        
        // Send message_stop
        output.push_str("event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n");
        
        Ok(Some(Bytes::from(output)))
    }

    fn finalize(&mut self) -> Result<Vec<Bytes>, StreamError> {
        if self.finished {
            return Ok(Vec::new());
        }
        
        if let Some(bytes) = self.finish_with_reason("end_turn")? {
            Ok(vec![bytes])
        } else {
            Ok(Vec::new())
        }
    }
}

impl AnToOAState {
    fn new() -> Self {
        Self {
            message_id: None,
            model: None,
            started: false,
            content_buffer: String::new(),
            tool_call_index: 0,
            tool_calls: HashMap::new(),
            finished: false,
        }
    }

    fn process_anthropic_event(&mut self, event_type: Option<&str>, data: &Value) -> Result<Option<Bytes>, StreamError> {
        let event_type = event_type.ok_or_else(|| StreamError::Parse("No event type".to_string()))?;
        
        match event_type {
            "message_start" => {
                if let Some(message) = data.get("message") {
                    self.message_id = message.get("id").and_then(|v| v.as_str()).map(|s| s.to_string());
                    self.model = message.get("model").and_then(|v| v.as_str()).map(|s| s.to_string());
                    self.started = true;
                }
                Ok(None)
            }
            "content_block_delta" => {
                if let Some(delta) = data.get("delta") {
                    if let Some(text_delta) = delta.get("text").and_then(|v| v.as_str()) {
                        if !text_delta.is_empty() {
                            self.content_buffer.push_str(text_delta);
                            
                            let output = json!({
                                "choices": [{
                                    "index": 0,
                                    "delta": {
                                        "content": text_delta
                                    }
                                }]
                            });
                            return Ok(Some(Bytes::from(format!("data: {}\n\n", output))));
                        }
                    }
                    if let Some(input_json_delta) = delta.get("partial_json").and_then(|v| v.as_str()) {
                        if !input_json_delta.is_empty() {
                            // Accumulate arguments for the current tool call
                            if let Some((_, tool_call)) = self.tool_calls.iter_mut().last() {
                                tool_call.arguments_buffer.push_str(input_json_delta);
                            }
                            
                            let output = json!({
                                "choices": [{
                                    "index": 0,
                                    "delta": {
                                        "tool_calls": [{
                                            "index": self.tool_call_index - 1,
                                            "function": {
                                                "arguments": input_json_delta
                                            }
                                        }]
                                    }
                                }]
                            });
                            return Ok(Some(Bytes::from(format!("data: {}\n\n", output))));
                        }
                    }
                }
                Ok(None)
            }
            "content_block_start" => {
                if let Some(content_block) = data.get("content_block") {
                    if content_block.get("type").and_then(|v| v.as_str()) == Some("tool_use") {
                        let id = content_block.get("id").and_then(|v| v.as_str());
                        let name = content_block.get("name").and_then(|v| v.as_str());
                        
                        self.tool_calls.insert(self.tool_call_index, ToolCallState {
                            id: id.map(|s| s.to_string()),
                            name: name.map(|s| s.to_string()),
                            arguments_buffer: String::new(),
                        });
                        
                        let output = json!({
                            "choices": [{
                                "index": 0,
                                "delta": {
                                    "tool_calls": [{
                                        "index": self.tool_call_index,
                                        "id": id,
                                        "function": {
                                            "name": name,
                                            "arguments": ""
                                        }
                                    }]
                                }
                            }]
                        });
                        self.tool_call_index += 1;
                        return Ok(Some(Bytes::from(format!("data: {}\n\n", output))));
                    }
                }
                Ok(None)
            }
            "message_delta" => {
                if let Some(delta) = data.get("delta") {
                    if let Some(stop_reason) = delta.get("stop_reason").and_then(|v| v.as_str()) {
                        let finish_reason = match stop_reason {
                            "end_turn" => "stop",
                            "max_tokens" => "length",
                            "tool_use" => "tool_calls",
                            _ => "stop",
                        };
                        
                        let output = json!({
                            "choices": [{
                                "index": 0,
                                "delta": {},
                                "finish_reason": finish_reason
                            }]
                        });
                        return Ok(Some(Bytes::from(format!("data: {}\n\n", output))));
                    }
                }
                Ok(None)
            }
            "message_stop" => {
                self.finished = true;
                Ok(Some(Bytes::from("data: [DONE]\n\n")))
            }
            _ => {
                // Unknown event type - pass through with warning
                tracing::warn!(event_type = %event_type, "Unknown Anthropic event type");
                Ok(Some(Bytes::from(format!("event: {}\ndata: {}\n\n", event_type, data))))
            }
        }
    }

    fn finalize(&mut self) -> Result<Vec<Bytes>, StreamError> {
        if self.finished {
            return Ok(Vec::new());
        }
        
        self.finished = true;
        Ok(vec![Bytes::from("data: [DONE]\n\n")])
    }
}

#[derive(Debug)]
pub enum StreamError {
    Parse(String),
    State(String),
}

impl std::fmt::Display for StreamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StreamError::Parse(msg) => write!(f, "Parse error: {}", msg),
            StreamError::State(msg) => write!(f, "State error: {}", msg),
        }
    }
}

impl std::error::Error for StreamError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oa_to_an_simple_text_stream() {
        let mut converter = StreamConverter::new(ApiType::OpenAIChat, ApiType::Anthropic);
        
        // Simulate OpenAI SSE chunks
        let chunk1 = b"data: {\"id\":\"chatcmpl-123\",\"model\":\"gpt-4\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\"}}]}\n\n";
        let chunk2 = b"data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello\"}}]}\n\n";
        let chunk3 = b"data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"completion_tokens\":5}}\n\n";
        let chunk4 = b"data: [DONE]\n\n";
        
        let output1 = converter.push(chunk1).unwrap();
        let output2 = converter.push(chunk2).unwrap();
        let output3 = converter.push(chunk3).unwrap();
        let output4 = converter.push(chunk4).unwrap();
        
        assert!(!output1.is_empty()); // message_start
        assert!(!output2.is_empty()); // content_block_start + content_block_delta
        assert!(!output3.is_empty()); // content_block_stop + message_delta + message_stop
        assert!(!output4.is_empty()); // [DONE] (but already finalized)
    }

    #[test]
    fn test_an_to_oa_simple_text_stream() {
        let mut converter = StreamConverter::new(ApiType::Anthropic, ApiType::OpenAIChat);
        
        let chunk1 = b"event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg-123\",\"model\":\"claude-3\",\"role\":\"assistant\",\"content\":[],\"usage\":{\"input_tokens\":10,\"output_tokens\":0}}}\n\n";
        let chunk2 = b"event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n";
        let chunk3 = b"event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}\n\n";
        let chunk4 = b"event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n";
        let chunk5 = b"event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\",\"stop_sequence\":null},\"usage\":{\"output_tokens\":5}}\n\n";
        let chunk6 = b"event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";
        
        let output1 = converter.push(chunk1).unwrap();
        let output2 = converter.push(chunk2).unwrap();
        let output3 = converter.push(chunk3).unwrap();
        let _output4 = converter.push(chunk4).unwrap();
        let output5 = converter.push(chunk5).unwrap();
        let output6 = converter.push(chunk6).unwrap();
        
        assert!(output1.is_empty()); // message_start is cached
        assert!(output2.is_empty()); // content_block_start is cached
        assert!(!output3.is_empty()); // content_delta produces OpenAI delta
        assert!(!output5.is_empty()); // message_delta produces finish_reason
        assert!(!output6.is_empty()); // message_stop produces [DONE]
    }

    #[test]
    fn test_chunk_splitting() {
        let mut converter = StreamConverter::new(ApiType::OpenAIChat, ApiType::Anthropic);
        
        // Split event across chunks
        let chunk1 = b"data: {\"choices\":[{\"index\":0,\"delta\":{\"content";
        let chunk2 = b":\"Hello\"}}]}\n\n";
        
        let output1 = converter.push(chunk1).unwrap();
        assert!(output1.is_empty()); // Buffering
        
        let output2 = converter.push(chunk2).unwrap();
        assert!(!output2.is_empty()); // Event completed
    }

    #[test]
    fn test_oa_to_an_tool_call_stream() {
        let mut converter = StreamConverter::new(ApiType::OpenAIChat, ApiType::Anthropic);
        
        let chunk1 = b"data: {\"id\":\"chatcmpl-123\",\"model\":\"gpt-4\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\"}}]}\n\n";
        let chunk2 = b"data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_123\",\"function\":{\"name\":\"weather\",\"arguments\":\"{\\\"city\\\"\"}}]}]}}]\n\n";
        let chunk3 = b"data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"\\\"Tokyo\\\"}\"}}]}]}\n\n";
        let chunk4 = b"data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n";
        
        let output1 = converter.push(chunk1).unwrap();
        let output2 = converter.push(chunk2).unwrap();
        let output3 = converter.push(chunk3).unwrap();
        let output4 = converter.push(chunk4).unwrap();
        
        assert!(!output1.is_empty()); // message_start
        assert!(!output2.is_empty()); // content_block_start (tool_use)
        assert!(!output3.is_empty()); // content_block_delta (input_json_delta)
        assert!(!output4.is_empty()); // content_block_stop + message_delta + message_stop
    }

    #[test]
    fn test_finalize_without_finish_reason() {
        let mut converter = StreamConverter::new(ApiType::OpenAIChat, ApiType::Anthropic);
        
        let chunk1 = b"data: {\"id\":\"chatcmpl-123\",\"model\":\"gpt-4\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\"}}]}\n\n";
        let chunk2 = b"data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello\"}}]}\n\n";
        let chunk3 = b"data: [DONE]\n\n";
        
        converter.push(chunk1).unwrap();
        converter.push(chunk2).unwrap();
        let output = converter.push(chunk3).unwrap();
        
        // Should finalize with end_turn
        assert!(!output.is_empty());
        let output_str = std::str::from_utf8(&output[0]).unwrap();
        assert!(output_str.contains("message_stop"));
    }

    #[test]
    fn test_empty_chunks() {
        let mut converter = StreamConverter::new(ApiType::OpenAIChat, ApiType::Anthropic);
        
        let output = converter.push(b"").unwrap();
        assert!(output.is_empty());
    }
}
