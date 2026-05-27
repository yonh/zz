//! # Protocol Conversion Audit Tests
//!
//! Comprehensive tests based on actual API specifications:
//! - OpenAI Chat Completion API
//! - OpenAI Responses API
//! - Anthropic Messages API
//!
//! Each test uses real-world request/response JSON from the official specs.

mod common;

use serde_json::Value;

// ============================================================================
// Helper functions
// ============================================================================

fn convert_responses_to_chat_request(body: &str) -> Result<String, String> {
    use zz::converter::{ApiConverter, ApiType, OpenAIResponsesToChatConverter};
    let converter = OpenAIResponsesToChatConverter;
    let bytes = bytes::Bytes::from(body.to_string());
    converter
        .convert_request(&bytes, ApiType::OpenAIChat)
        .map(|b| String::from_utf8_lossy(&b).to_string())
        .map_err(|e| e.to_string())
}

fn convert_chat_to_responses_response(body: &str) -> Result<String, String> {
    use zz::converter::{ApiConverter, ApiType, OpenAIResponsesToChatConverter};
    let converter = OpenAIResponsesToChatConverter;
    let bytes = bytes::Bytes::from(body.to_string());
    converter
        .convert_response(&bytes, ApiType::OpenAIChat, ApiType::OpenAIResponses, false)
        .map(|b| String::from_utf8_lossy(&b).to_string())
        .map_err(|e| e.to_string())
}

fn convert_anthropic_to_openai_request(body: &str) -> Result<String, String> {
    use zz::converter::{ApiConverter, ApiType, AnthropicToOpenAIConverter};
    let converter = AnthropicToOpenAIConverter;
    let bytes = bytes::Bytes::from(body.to_string());
    converter
        .convert_request(&bytes, ApiType::OpenAIChat)
        .map(|b| String::from_utf8_lossy(&b).to_string())
        .map_err(|e| e.to_string())
}

fn convert_openai_to_anthropic_request(body: &str) -> Result<String, String> {
    use zz::converter::{ApiConverter, ApiType, OpenAIChatToAnthropicConverter};
    let converter = OpenAIChatToAnthropicConverter;
    let bytes = bytes::Bytes::from(body.to_string());
    converter
        .convert_request(&bytes, ApiType::Anthropic)
        .map(|b| String::from_utf8_lossy(&b).to_string())
        .map_err(|e| e.to_string())
}

fn convert_anthropic_to_openai_response(body: &str) -> Result<String, String> {
    use zz::converter::{ApiConverter, ApiType, AnthropicToOpenAIConverter};
    let converter = AnthropicToOpenAIConverter;
    let bytes = bytes::Bytes::from(body.to_string());
    converter
        .convert_response(&bytes, ApiType::Anthropic, ApiType::OpenAIChat, false)
        .map(|b| String::from_utf8_lossy(&b).to_string())
        .map_err(|e| e.to_string())
}

fn convert_openai_to_anthropic_response(body: &str) -> Result<String, String> {
    use zz::converter::{ApiConverter, ApiType, OpenAIChatToAnthropicConverter};
    let converter = OpenAIChatToAnthropicConverter;
    let bytes = bytes::Bytes::from(body.to_string());
    converter
        .convert_response(&bytes, ApiType::OpenAIChat, ApiType::Anthropic, false)
        .map(|b| String::from_utf8_lossy(&b).to_string())
        .map_err(|e| e.to_string())
}

// ============================================================================
// Responses API → Chat API (Request)
// Based on OpenAI Responses API spec
// ============================================================================

#[cfg(test)]
mod responses_to_chat_request {
    use super::*;

    /// Responses API: simple string input → Chat user message
    #[test]
    fn simple_string_input() {
        let req = r#"{"model":"gpt-4o","input":"What is the capital of France?"}"#;
        let result = convert_responses_to_chat_request(req).unwrap();
        let chat: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(chat["messages"][0]["role"], "user");
        assert_eq!(chat["messages"][0]["content"], "What is the capital of France?");
        assert_eq!(chat["model"], "gpt-4o");
    }

    /// Responses API: instructions → system message (inserted first)
    #[test]
    fn instructions_becomes_system() {
        let req = r#"{"model":"gpt-4o","input":"Hello","instructions":"You are a helpful assistant."}"#;
        let result = convert_responses_to_chat_request(req).unwrap();
        let chat: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(chat["messages"][0]["role"], "system");
        assert_eq!(chat["messages"][0]["content"], "You are a helpful assistant.");
        assert_eq!(chat["messages"][1]["role"], "user");
        assert_eq!(chat["messages"][1]["content"], "Hello");
    }

    /// Responses API: developer role → system role in Chat
    #[test]
    fn developer_role_becomes_system() {
        let req = r#"{"model":"gpt-4o","input":[{"type":"message","role":"developer","content":"Be concise"},{"type":"message","role":"user","content":"Hello"}]}"#;
        let result = convert_responses_to_chat_request(req).unwrap();
        let chat: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(chat["messages"][0]["role"], "system");
        assert_eq!(chat["messages"][0]["content"], "Be concise");
        assert_eq!(chat["messages"][1]["role"], "user");
    }

    /// Responses API: max_output_tokens → max_tokens
    #[test]
    fn max_output_tokens_mapping() {
        let req = r#"{"model":"gpt-4o","input":"Hello","max_output_tokens":1024}"#;
        let result = convert_responses_to_chat_request(req).unwrap();
        let chat: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(chat["max_tokens"], 1024);
    }

    /// Responses API: tools format → Chat tools format
    /// Responses: {type:"function", name, description, parameters}
    /// Chat: {type:"function", function:{name, description, parameters}}
    #[test]
    fn tools_conversion_format() {
        let req = r#"{
            "model":"gpt-4o",
            "input":"Get weather",
            "tools":[{
                "type":"function",
                "name":"get_weather",
                "description":"Get weather for a location",
                "parameters":{
                    "type":"object",
                    "properties":{"location":{"type":"string"}},
                    "required":["location"]
                }
            }]
        }"#;
        let result = convert_responses_to_chat_request(req).unwrap();
        let chat: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(chat["tools"][0]["type"], "function");
        assert_eq!(chat["tools"][0]["function"]["name"], "get_weather");
        assert_eq!(chat["tools"][0]["function"]["description"], "Get weather for a location");
        assert_eq!(chat["tools"][0]["function"]["parameters"]["type"], "object");
    }

    /// Responses API: tool_choice mapping
    /// Responses: "auto"|"required"|"none"|{type:"function",name:"..."}
    /// Chat: "auto"|"required"|"none"|{type:"function",function:{name:"..."}}
    #[test]
    fn tool_choice_auto() {
        let req = r#"{"model":"gpt-4o","input":"Hello","tool_choice":"auto"}"#;
        let result = convert_responses_to_chat_request(req).unwrap();
        let chat: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(chat["tool_choice"], "auto");
    }

    #[test]
    fn tool_choice_required() {
        let req = r#"{"model":"gpt-4o","input":"Hello","tool_choice":"required"}"#;
        let result = convert_responses_to_chat_request(req).unwrap();
        let chat: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(chat["tool_choice"], "required");
    }

    /// Responses API: non-function tools should be filtered out
    /// (web_search, file_search, code_interpreter, computer_use, mcp)
    #[test]
    fn filters_non_function_tools() {
        let req = r#"{
            "model":"gpt-4o",
            "input":"Hello",
            "tools":[
                {"type":"function","name":"do_thing","description":"Does a thing","parameters":{"type":"object"}},
                {"type":"web_search","external_web_access":false},
                {"type":"file_search","vector_store_ids":["vs_123"]},
                {"type":"code_interpreter","container":{"type":"auto"}}
            ]
        }"#;
        let result = convert_responses_to_chat_request(req).unwrap();
        let chat: Value = serde_json::from_str(&result).unwrap();
        let tools = chat["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 1, "Only function tools should be kept");
        assert_eq!(tools[0]["function"]["name"], "do_thing");
    }

    /// Responses API: stream field should be preserved
    #[test]
    fn preserves_stream() {
        let req = r#"{"model":"gpt-4o","input":"Hello","stream":true}"#;
        let result = convert_responses_to_chat_request(req).unwrap();
        let chat: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(chat["stream"], true);
    }

    /// Responses API: store and previous_response_id should be dropped
    #[test]
    fn drops_stateful_fields() {
        let req = r#"{"model":"gpt-4o","input":"Hello","store":true,"previous_response_id":"resp_abc"}"#;
        let result = convert_responses_to_chat_request(req).unwrap();
        let chat: Value = serde_json::from_str(&result).unwrap();
        assert!(chat.get("store").is_none());
        assert!(chat.get("previous_response_id").is_none());
    }

    /// Responses API: temperature, top_p, stop should pass through
    #[test]
    fn passthrough_parameters() {
        let req = r#"{"model":"gpt-4o","input":"Hello","temperature":0.7,"top_p":0.9,"stop":["END"],"metadata":{"user_id":"u1"}}"#;
        let result = convert_responses_to_chat_request(req).unwrap();
        let chat: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(chat["temperature"], 0.7);
        assert_eq!(chat["top_p"], 0.9);
        assert_eq!(chat["stop"][0], "END");
        assert_eq!(chat["metadata"]["user_id"], "u1");
    }

    /// Responses API: input array with multiple messages
    #[test]
    fn input_array_multiple_messages() {
        let req = r#"{
            "model":"gpt-4o",
            "input":[
                {"type":"message","role":"user","content":"Hi"},
                {"type":"message","role":"assistant","content":"Hello!"},
                {"type":"message","role":"user","content":"How are you?"}
            ]
        }"#;
        let result = convert_responses_to_chat_request(req).unwrap();
        let chat: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(chat["messages"].as_array().unwrap().len(), 3);
        assert_eq!(chat["messages"][0]["role"], "user");
        assert_eq!(chat["messages"][0]["content"], "Hi");
        assert_eq!(chat["messages"][1]["role"], "assistant");
        assert_eq!(chat["messages"][2]["content"], "How are you?");
    }

    /// Responses API: tool_choice with specific function
    /// Responses: {type:"function", name:"get_weather"}
    /// Chat: {type:"function", function:{name:"get_weather"}}
    #[test]
    fn tool_choice_specific_function() {
        let req = r#"{"model":"gpt-4o","input":"Weather?","tool_choice":{"type":"function","name":"get_weather"}}"#;
        let result = convert_responses_to_chat_request(req).unwrap();
        let chat: Value = serde_json::from_str(&result).unwrap();
        // Responses API function-specific tool_choice format differs from Chat
        // Responses: {type:"function", name:"..."}
        // Chat: {type:"function", function:{name:"..."}}
        assert_eq!(chat["tool_choice"]["type"], "function");
    }
}

// ============================================================================
// Chat API → Responses API (Response)
// Based on OpenAI Chat Completion API spec
// ============================================================================

#[cfg(test)]
mod chat_to_responses_response {
    use super::*;

    /// Chat: simple text response → Responses format
    /// Must have: id (resp_ prefix), object:"response", output array, usage with total_tokens
    #[test]
    fn simple_text_response() {
        let chat_resp = r#"{
            "id":"chatcmpl-abc123",
            "object":"chat.completion",
            "created":1748332800,
            "model":"gpt-4o",
            "choices":[{
                "index":0,
                "message":{"role":"assistant","content":"The capital of France is Paris."},
                "finish_reason":"stop"
            }],
            "usage":{"prompt_tokens":25,"completion_tokens":10,"total_tokens":35}
        }"#;
        let result = convert_chat_to_responses_response(chat_resp).unwrap();
        let resp: Value = serde_json::from_str(&result).unwrap();

        // Responses format requirements
        assert!(resp["id"].as_str().unwrap().starts_with("resp_"), "id must start with resp_");
        assert_eq!(resp["object"], "response");
        assert_eq!(resp["model"], "gpt-4o");

        // Output structure
        let output = resp["output"].as_array().unwrap();
        assert_eq!(output.len(), 1);
        assert_eq!(output[0]["type"], "message");
        assert_eq!(output[0]["role"], "assistant");
        assert_eq!(output[0]["content"][0]["type"], "output_text");
        assert_eq!(output[0]["content"][0]["text"], "The capital of France is Paris.");
        assert!(output[0]["content"][0]["annotations"].is_array());

        // Usage must have total_tokens
        assert_eq!(resp["usage"]["input_tokens"], 25);
        assert_eq!(resp["usage"]["output_tokens"], 10);
        assert_eq!(resp["usage"]["total_tokens"], 35);
    }

    /// Chat: tool_calls response → Responses function_call output
    #[test]
    fn tool_calls_response() {
        let chat_resp = r#"{
            "id":"chatcmpl-abc123",
            "object":"chat.completion",
            "created":1748332800,
            "model":"gpt-4o",
            "choices":[{
                "index":0,
                "message":{
                    "role":"assistant",
                    "content":null,
                    "tool_calls":[{
                        "id":"call_abc123",
                        "type":"function",
                        "function":{"name":"get_weather","arguments":"{\"location\":\"Paris\"}"}
                    }]
                },
                "finish_reason":"tool_calls"
            }],
            "usage":{"prompt_tokens":50,"completion_tokens":10,"total_tokens":60}
        }"#;
        let result = convert_chat_to_responses_response(chat_resp).unwrap();
        let resp: Value = serde_json::from_str(&result).unwrap();

        let output = resp["output"].as_array().unwrap();
        // Should have function_call item
        let fc = output.iter().find(|o| o["type"] == "function_call").unwrap();
        assert_eq!(fc["id"], "call_abc123");
        assert_eq!(fc["call_id"], "call_abc123");
        assert_eq!(fc["name"], "get_weather");
        assert_eq!(fc["arguments"], "{\"location\":\"Paris\"}");

        // stop_reason should be "tool_use"
        let msg = output.iter().find(|o| o["type"] == "message").unwrap();
        assert_eq!(msg["stop_reason"], "tool_use");
    }

    /// Chat: finish_reason mapping
    /// stop → end_turn, length → max_tokens, tool_calls → tool_use, content_filter → content_filter
    #[test]
    fn finish_reason_stop() {
        let chat_resp = r#"{"id":"chatcmpl-1","model":"gpt-4o","choices":[{"index":0,"message":{"role":"assistant","content":"OK"},"finish_reason":"stop"}],"usage":{"prompt_tokens":5,"completion_tokens":2,"total_tokens":7}}"#;
        let resp: Value = serde_json::from_str(&convert_chat_to_responses_response(chat_resp).unwrap()).unwrap();
        assert_eq!(resp["output"][0]["stop_reason"], "end_turn");
    }

    #[test]
    fn finish_reason_length() {
        let chat_resp = r#"{"id":"chatcmpl-1","model":"gpt-4o","choices":[{"index":0,"message":{"role":"assistant","content":"part"},"finish_reason":"length"}],"usage":{"prompt_tokens":5,"completion_tokens":100,"total_tokens":105}}"#;
        let resp: Value = serde_json::from_str(&convert_chat_to_responses_response(chat_resp).unwrap()).unwrap();
        assert_eq!(resp["output"][0]["stop_reason"], "max_tokens");
    }

    #[test]
    fn finish_reason_tool_calls() {
        let chat_resp = r#"{"id":"chatcmpl-1","model":"gpt-4o","choices":[{"index":0,"message":{"role":"assistant","content":null,"tool_calls":[{"id":"call_1","type":"function","function":{"name":"fn","arguments":"{}"}}]},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":5,"completion_tokens":10,"total_tokens":15}}"#;
        let resp: Value = serde_json::from_str(&convert_chat_to_responses_response(chat_resp).unwrap()).unwrap();
        // function_call items come before the message item
        let msg = resp["output"].as_array().unwrap().iter().find(|o| o["type"] == "message").unwrap();
        assert_eq!(msg["stop_reason"], "tool_use");
    }

    #[test]
    fn finish_reason_content_filter() {
        let chat_resp = r#"{"id":"chatcmpl-1","model":"gpt-4o","choices":[{"index":0,"message":{"role":"assistant","content":"Text"},"finish_reason":"content_filter"}],"usage":{"prompt_tokens":5,"completion_tokens":2,"total_tokens":7}}"#;
        let resp: Value = serde_json::from_str(&convert_chat_to_responses_response(chat_resp).unwrap()).unwrap();
        assert_eq!(resp["output"][0]["stop_reason"], "content_filter");
    }

    /// Chat: usage with total_tokens (Codex requires this)
    #[test]
    fn usage_has_total_tokens() {
        let chat_resp = r#"{"id":"chatcmpl-1","model":"gpt-4o","choices":[{"index":0,"message":{"role":"assistant","content":"OK"},"finish_reason":"stop"}],"usage":{"prompt_tokens":100,"completion_tokens":50,"total_tokens":150}}"#;
        let resp: Value = serde_json::from_str(&convert_chat_to_responses_response(chat_resp).unwrap()).unwrap();
        assert_eq!(resp["usage"]["total_tokens"], 150);
    }

    /// Chat: usage without total_tokens (should be computed)
    #[test]
    fn usage_computes_total_tokens() {
        let chat_resp = r#"{"id":"chatcmpl-1","model":"gpt-4o","choices":[{"index":0,"message":{"role":"assistant","content":"OK"},"finish_reason":"stop"}],"usage":{"prompt_tokens":100,"completion_tokens":50}}"#;
        let resp: Value = serde_json::from_str(&convert_chat_to_responses_response(chat_resp).unwrap()).unwrap();
        assert_eq!(resp["usage"]["total_tokens"], 150);
    }

    /// Chat: error response → Responses error format
    #[test]
    fn error_response() {
        let chat_resp = r#"{"error":{"message":"Invalid API key","type":"authentication_error","code":"invalid_api_key"}}"#;
        let result = convert_chat_to_responses_response(chat_resp).unwrap();
        let resp: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(resp["type"], "error");
        assert_eq!(resp["error"]["type"], "authentication_error");
        assert_eq!(resp["error"]["message"], "Invalid API key");
    }

    /// Chat: model and created should be preserved
    #[test]
    fn metadata_passthrough() {
        let chat_resp = r#"{"id":"chatcmpl-1","object":"chat.completion","created":1700000000,"model":"gpt-4-turbo","choices":[{"index":0,"message":{"role":"assistant","content":"OK"},"finish_reason":"stop"}],"usage":{"prompt_tokens":5,"completion_tokens":2,"total_tokens":7}}"#;
        let resp: Value = serde_json::from_str(&convert_chat_to_responses_response(chat_resp).unwrap()).unwrap();
        assert_eq!(resp["created"], 1700000000);
        assert_eq!(resp["model"], "gpt-4-turbo");
    }
}

// ============================================================================
// Anthropic → OpenAI Chat (Request)
// Based on Anthropic Messages API spec
// ============================================================================

#[cfg(test)]
mod anthropic_to_openai_request {
    use super::*;

    /// Anthropic: simple text request
    #[test]
    fn simple_text_request() {
        let req = r#"{
            "model":"claude-sonnet-4-6",
            "max_tokens":1024,
            "messages":[{"role":"user","content":"Hello, world!"}]
        }"#;
        let result = convert_anthropic_to_openai_request(req).unwrap();
        let chat: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(chat["model"], "claude-sonnet-4-6");
        assert_eq!(chat["max_tokens"], 1024);
        assert_eq!(chat["messages"][0]["role"], "user");
        assert_eq!(chat["messages"][0]["content"], "Hello, world!");
    }

    /// Anthropic: system string → OpenAI system message
    #[test]
    fn system_string_to_system_message() {
        let req = r#"{
            "model":"claude-sonnet-4-6",
            "max_tokens":1024,
            "system":"You are a helpful assistant.",
            "messages":[{"role":"user","content":"Hello!"}]
        }"#;
        let result = convert_anthropic_to_openai_request(req).unwrap();
        let chat: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(chat["messages"][0]["role"], "system");
        assert_eq!(chat["messages"][0]["content"], "You are a helpful assistant.");
        assert_eq!(chat["messages"][1]["role"], "user");
    }

    /// Anthropic: system array → concatenated system message
    #[test]
    fn system_array_concatenated() {
        let req = r#"{
            "model":"claude-sonnet-4-6",
            "max_tokens":1024,
            "system":[
                {"type":"text","text":"First instruction."},
                {"type":"text","text":"Second instruction."}
            ],
            "messages":[{"role":"user","content":"Hello!"}]
        }"#;
        let result = convert_anthropic_to_openai_request(req).unwrap();
        let chat: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(chat["messages"][0]["role"], "system");
        assert_eq!(chat["messages"][0]["content"], "First instruction.\n\nSecond instruction.");
    }

    /// Anthropic: tools format → OpenAI tools format
    /// Anthropic: {name, description, input_schema}
    /// OpenAI: {type:"function", function:{name, description, parameters}}
    #[test]
    fn tools_conversion() {
        let req = r#"{
            "model":"claude-sonnet-4-6",
            "max_tokens":1024,
            "messages":[{"role":"user","content":"Get weather"}],
            "tools":[{
                "name":"get_weather",
                "description":"Get current weather",
                "input_schema":{
                    "type":"object",
                    "properties":{"location":{"type":"string"}},
                    "required":["location"]
                }
            }]
        }"#;
        let result = convert_anthropic_to_openai_request(req).unwrap();
        let chat: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(chat["tools"][0]["type"], "function");
        assert_eq!(chat["tools"][0]["function"]["name"], "get_weather");
        assert_eq!(chat["tools"][0]["function"]["description"], "Get current weather");
        assert_eq!(chat["tools"][0]["function"]["parameters"]["properties"]["location"]["type"], "string");
    }

    /// Anthropic: tool_choice mapping
    /// auto → auto, any → required, tool → {type:"function",function:{name}}
    #[test]
    fn tool_choice_auto() {
        let req = r#"{"model":"claude-sonnet-4-6","max_tokens":100,"messages":[{"role":"user","content":"Hi"}],"tool_choice":{"type":"auto"}}"#;
        let result = convert_anthropic_to_openai_request(req).unwrap();
        let chat: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(chat["tool_choice"], "auto");
    }

    #[test]
    fn tool_choice_any_to_required() {
        let req = r#"{"model":"claude-sonnet-4-6","max_tokens":100,"messages":[{"role":"user","content":"Hi"}],"tool_choice":{"type":"any"}}"#;
        let result = convert_anthropic_to_openai_request(req).unwrap();
        let chat: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(chat["tool_choice"], "required");
    }

    #[test]
    fn tool_choice_specific_tool() {
        let req = r#"{"model":"claude-sonnet-4-6","max_tokens":100,"messages":[{"role":"user","content":"Hi"}],"tool_choice":{"type":"tool","name":"get_weather"}}"#;
        let result = convert_anthropic_to_openai_request(req).unwrap();
        let chat: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(chat["tool_choice"]["type"], "function");
        assert_eq!(chat["tool_choice"]["function"]["name"], "get_weather");
    }

    /// Anthropic: stop_sequences → stop
    #[test]
    fn stop_sequences_mapping() {
        let req = r#"{"model":"claude-sonnet-4-6","max_tokens":100,"messages":[{"role":"user","content":"Hi"}],"stop_sequences":["\n\nHuman:","\n\nAssistant:"]}"#;
        let result = convert_anthropic_to_openai_request(req).unwrap();
        let chat: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(chat["stop"][0], "\n\nHuman:");
        assert_eq!(chat["stop"][1], "\n\nAssistant:");
    }

    /// Anthropic: metadata.user_id → user
    #[test]
    fn metadata_user_id_mapping() {
        let req = r#"{"model":"claude-sonnet-4-6","max_tokens":100,"messages":[{"role":"user","content":"Hi"}],"metadata":{"user_id":"user123"}}"#;
        let result = convert_anthropic_to_openai_request(req).unwrap();
        let chat: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(chat["user"], "user123");
    }

    /// Anthropic: top_k should be skipped (not supported in OpenAI)
    #[test]
    fn top_k_skipped() {
        let req = r#"{"model":"claude-sonnet-4-6","max_tokens":100,"messages":[{"role":"user","content":"Hi"}],"top_k":40}"#;
        let result = convert_anthropic_to_openai_request(req).unwrap();
        let chat: Value = serde_json::from_str(&result).unwrap();
        assert!(chat.get("top_k").is_none());
    }

    /// Anthropic: tool_result in user message → OpenAI tool message
    #[test]
    fn tool_result_conversion() {
        let req = r#"{
            "model":"claude-sonnet-4-6",
            "max_tokens":100,
            "messages":[
                {"role":"user","content":[
                    {"type":"tool_result","tool_use_id":"toolu_123","content":"The weather is sunny"}
                ]}
            ]
        }"#;
        let result = convert_anthropic_to_openai_request(req).unwrap();
        let chat: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(chat["messages"][0]["role"], "tool");
        assert_eq!(chat["messages"][0]["tool_call_id"], "toolu_123");
    }

    /// Anthropic: tool_use in assistant message → OpenAI tool_calls
    #[test]
    fn tool_use_in_assistant_message() {
        let req = r#"{
            "model":"claude-sonnet-4-6",
            "max_tokens":100,
            "messages":[
                {"role":"assistant","content":[
                    {"type":"text","text":"Let me check."},
                    {"type":"tool_use","id":"toolu_123","name":"get_weather","input":{"location":"SF"}}
                ]}
            ]
        }"#;
        let result = convert_anthropic_to_openai_request(req).unwrap();
        let chat: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(chat["messages"][0]["role"], "assistant");
        assert_eq!(chat["messages"][0]["tool_calls"][0]["id"], "toolu_123");
        assert_eq!(chat["messages"][0]["tool_calls"][0]["function"]["name"], "get_weather");
        assert_eq!(chat["messages"][0]["tool_calls"][0]["function"]["arguments"], "{\"location\":\"SF\"}");
    }
}

// ============================================================================
// OpenAI Chat → Anthropic (Request)
// Based on OpenAI Chat Completion API spec
// ============================================================================

#[cfg(test)]
mod openai_to_anthropic_request {
    use super::*;

    /// Chat: simple text request
    #[test]
    fn simple_text_request() {
        let req = r#"{
            "model":"gpt-4o",
            "messages":[{"role":"user","content":"Hello, world!"}]
        }"#;
        let result = convert_openai_to_anthropic_request(req).unwrap();
        let anthropic: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(anthropic["model"], "gpt-4o");
        assert_eq!(anthropic["messages"][0]["role"], "user");
        assert_eq!(anthropic["messages"][0]["content"], "Hello, world!");
    }

    /// Chat: system message → Anthropic system field
    #[test]
    fn system_message_to_system_field() {
        let req = r#"{
            "model":"gpt-4o",
            "messages":[
                {"role":"system","content":"You are a helpful assistant."},
                {"role":"user","content":"Hello!"}
            ]
        }"#;
        let result = convert_openai_to_anthropic_request(req).unwrap();
        let anthropic: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(anthropic["system"], "You are a helpful assistant.");
        assert_eq!(anthropic["messages"][0]["role"], "user");
    }

    /// Chat: max_tokens → max_tokens
    #[test]
    fn max_tokens_passthrough() {
        let req = r#"{"model":"gpt-4o","messages":[{"role":"user","content":"Hi"}],"max_tokens":1024}"#;
        let result = convert_openai_to_anthropic_request(req).unwrap();
        let anthropic: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(anthropic["max_tokens"], 1024);
    }

    /// Chat: max_completion_tokens → max_tokens
    #[test]
    fn max_completion_tokens_to_max_tokens() {
        let req = r#"{"model":"gpt-4o","messages":[{"role":"user","content":"Hi"}],"max_completion_tokens":2048}"#;
        let result = convert_openai_to_anthropic_request(req).unwrap();
        let anthropic: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(anthropic["max_tokens"], 2048);
    }

    /// Chat: tools format → Anthropic tools format
    /// Chat: {type:"function", function:{name, description, parameters}}
    /// Anthropic: {name, description, input_schema}
    #[test]
    fn tools_conversion() {
        let req = r#"{
            "model":"gpt-4o",
            "messages":[{"role":"user","content":"Get weather"}],
            "tools":[{
                "type":"function",
                "function":{
                    "name":"get_weather",
                    "description":"Get current weather",
                    "parameters":{"type":"object","properties":{"location":{"type":"string"}}}
                }
            }]
        }"#;
        let result = convert_openai_to_anthropic_request(req).unwrap();
        let anthropic: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(anthropic["tools"][0]["name"], "get_weather");
        assert_eq!(anthropic["tools"][0]["description"], "Get current weather");
        assert_eq!(anthropic["tools"][0]["input_schema"]["type"], "object");
    }

    /// Chat: tool_choice mapping
    /// auto → {type:"auto"}, required → {type:"any"}, none → {type:"none"}
    /// {type:"function",function:{name}} → {type:"tool",name}
    #[test]
    fn tool_choice_auto() {
        let req = r#"{"model":"gpt-4o","messages":[{"role":"user","content":"Hi"}],"tool_choice":"auto"}"#;
        let result = convert_openai_to_anthropic_request(req).unwrap();
        let anthropic: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(anthropic["tool_choice"]["type"], "auto");
    }

    #[test]
    fn tool_choice_required_to_any() {
        let req = r#"{"model":"gpt-4o","messages":[{"role":"user","content":"Hi"}],"tool_choice":"required"}"#;
        let result = convert_openai_to_anthropic_request(req).unwrap();
        let anthropic: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(anthropic["tool_choice"]["type"], "any");
    }

    #[test]
    fn tool_choice_specific_function() {
        let req = r#"{"model":"gpt-4o","messages":[{"role":"user","content":"Hi"}],"tool_choice":{"type":"function","function":{"name":"get_weather"}}}"#;
        let result = convert_openai_to_anthropic_request(req).unwrap();
        let anthropic: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(anthropic["tool_choice"]["type"], "tool");
        assert_eq!(anthropic["tool_choice"]["name"], "get_weather");
    }

    /// Chat: stop → stop_sequences
    #[test]
    fn stop_string_to_array() {
        let req = r#"{"model":"gpt-4o","messages":[{"role":"user","content":"Hi"}],"stop":"END"}"#;
        let result = convert_openai_to_anthropic_request(req).unwrap();
        let anthropic: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(anthropic["stop_sequences"][0], "END");
    }

    #[test]
    fn stop_array_passthrough() {
        let req = r#"{"model":"gpt-4o","messages":[{"role":"user","content":"Hi"}],"stop":["END","STOP"]}"#;
        let result = convert_openai_to_anthropic_request(req).unwrap();
        let anthropic: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(anthropic["stop_sequences"][0], "END");
        assert_eq!(anthropic["stop_sequences"][1], "STOP");
    }

    /// Chat: user → metadata.user_id
    #[test]
    fn user_to_metadata() {
        let req = r#"{"model":"gpt-4o","messages":[{"role":"user","content":"Hi"}],"user":"user123"}"#;
        let result = convert_openai_to_anthropic_request(req).unwrap();
        let anthropic: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(anthropic["metadata"]["user_id"], "user123");
    }

    /// Chat: parallel_tool_calls=false → tool_choice.disable_parallel_tool_use
    #[test]
    fn parallel_tool_calls_false() {
        let req = r#"{"model":"gpt-4o","messages":[{"role":"user","content":"Hi"}],"parallel_tool_calls":false}"#;
        let result = convert_openai_to_anthropic_request(req).unwrap();
        let anthropic: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(anthropic["tool_choice"]["disable_parallel_tool_use"], true);
        assert_eq!(anthropic["tool_choice"]["type"], "auto");
    }

    /// Chat: tool message → Anthropic tool_result in user message
    #[test]
    fn tool_message_to_tool_result() {
        let req = r#"{
            "model":"gpt-4o",
            "messages":[
                {"role":"user","content":"Get weather"},
                {"role":"assistant","content":null,"tool_calls":[{"id":"call_1","type":"function","function":{"name":"get_weather","arguments":"{\"location\":\"SF\"}"}}]},
                {"role":"tool","tool_call_id":"call_1","content":"{\"temp\":72}"}
            ]
        }"#;
        let result = convert_openai_to_anthropic_request(req).unwrap();
        let anthropic: Value = serde_json::from_str(&result).unwrap();
        // tool message should become user message with tool_result
        let tool_msg = anthropic["messages"].as_array().unwrap().iter()
            .find(|m| m["role"] == "user" && m["content"].is_array())
            .unwrap();
        assert_eq!(tool_msg["content"][0]["type"], "tool_result");
        assert_eq!(tool_msg["content"][0]["tool_use_id"], "call_1");
    }

    /// Chat: assistant with tool_calls → Anthropic assistant with tool_use blocks
    #[test]
    fn assistant_tool_calls_to_tool_use() {
        let req = r#"{
            "model":"gpt-4o",
            "messages":[
                {"role":"user","content":"Get weather"},
                {"role":"assistant","content":null,"tool_calls":[{"id":"call_1","type":"function","function":{"name":"get_weather","arguments":"{\"location\":\"SF\"}"}}]}
            ]
        }"#;
        let result = convert_openai_to_anthropic_request(req).unwrap();
        let anthropic: Value = serde_json::from_str(&result).unwrap();
        let assistant_msg = &anthropic["messages"][1];
        assert_eq!(assistant_msg["role"], "assistant");
        assert_eq!(assistant_msg["content"][0]["type"], "tool_use");
        assert_eq!(assistant_msg["content"][0]["id"], "call_1");
        assert_eq!(assistant_msg["content"][0]["name"], "get_weather");
        assert_eq!(assistant_msg["content"][0]["input"]["location"], "SF");
    }
}

// ============================================================================
// Anthropic → OpenAI Chat (Response)
// Based on Anthropic Messages API spec
// ============================================================================

#[cfg(test)]
mod anthropic_to_openai_response {
    use super::*;

    /// Anthropic: simple text response → Chat format
    #[test]
    fn simple_text_response() {
        let resp = r#"{
            "type":"message",
            "id":"msg_013Zva2CMHLNnXjNJJKqJ2EF",
            "role":"assistant",
            "model":"claude-sonnet-4-6",
            "content":[{"type":"text","text":"Hi! My name is Claude."}],
            "stop_reason":"end_turn",
            "stop_sequence":null,
            "usage":{"input_tokens":2095,"output_tokens":503}
        }"#;
        let result = convert_anthropic_to_openai_response(resp).unwrap();
        let chat: Value = serde_json::from_str(&result).unwrap();

        assert_eq!(chat["id"], "msg_013Zva2CMHLNnXjNJJKqJ2EF");
        assert_eq!(chat["object"], "chat.completion");
        assert_eq!(chat["model"], "claude-sonnet-4-6");
        assert_eq!(chat["choices"][0]["message"]["role"], "assistant");
        assert_eq!(chat["choices"][0]["message"]["content"], "Hi! My name is Claude.");
        assert_eq!(chat["choices"][0]["finish_reason"], "stop");
        assert_eq!(chat["usage"]["prompt_tokens"], 2095);
        assert_eq!(chat["usage"]["completion_tokens"], 503);
    }

    /// Anthropic: tool_use response → Chat tool_calls
    #[test]
    fn tool_use_response() {
        let resp = r#"{
            "type":"message",
            "id":"msg_123",
            "role":"assistant",
            "model":"claude-sonnet-4-6",
            "content":[
                {"type":"text","text":"Let me check the weather."},
                {"type":"tool_use","id":"toolu_01D7FLrfh4GYq7yT1ULFeyMV","name":"get_weather","input":{"location":"San Francisco"}}
            ],
            "stop_reason":"tool_use",
            "usage":{"input_tokens":100,"output_tokens":50}
        }"#;
        let result = convert_anthropic_to_openai_response(resp).unwrap();
        let chat: Value = serde_json::from_str(&result).unwrap();

        assert_eq!(chat["choices"][0]["message"]["content"], "Let me check the weather.");
        assert_eq!(chat["choices"][0]["message"]["tool_calls"][0]["id"], "toolu_01D7FLrfh4GYq7yT1ULFeyMV");
        assert_eq!(chat["choices"][0]["message"]["tool_calls"][0]["type"], "function");
        assert_eq!(chat["choices"][0]["message"]["tool_calls"][0]["function"]["name"], "get_weather");
        assert_eq!(chat["choices"][0]["message"]["tool_calls"][0]["function"]["arguments"], "{\"location\":\"San Francisco\"}");
        assert_eq!(chat["choices"][0]["finish_reason"], "tool_calls");
    }

    /// Anthropic: stop_reason → finish_reason mapping
    /// end_turn → stop, max_tokens → length, tool_use → tool_calls, stop_sequence → stop
    #[test]
    fn stop_reason_end_turn() {
        let resp = r#"{"type":"message","id":"msg_1","role":"assistant","model":"m","content":[{"type":"text","text":"OK"}],"stop_reason":"end_turn","usage":{"input_tokens":1,"output_tokens":1}}"#;
        let chat: Value = serde_json::from_str(&convert_anthropic_to_openai_response(resp).unwrap()).unwrap();
        assert_eq!(chat["choices"][0]["finish_reason"], "stop");
    }

    #[test]
    fn stop_reason_max_tokens() {
        let resp = r#"{"type":"message","id":"msg_1","role":"assistant","model":"m","content":[{"type":"text","text":"part"}],"stop_reason":"max_tokens","usage":{"input_tokens":1,"output_tokens":100}}"#;
        let chat: Value = serde_json::from_str(&convert_anthropic_to_openai_response(resp).unwrap()).unwrap();
        assert_eq!(chat["choices"][0]["finish_reason"], "length");
    }

    #[test]
    fn stop_reason_tool_use() {
        let resp = r#"{"type":"message","id":"msg_1","role":"assistant","model":"m","content":[{"type":"tool_use","id":"t1","name":"fn","input":{}}],"stop_reason":"tool_use","usage":{"input_tokens":1,"output_tokens":10}}"#;
        let chat: Value = serde_json::from_str(&convert_anthropic_to_openai_response(resp).unwrap()).unwrap();
        assert_eq!(chat["choices"][0]["finish_reason"], "tool_calls");
    }

    #[test]
    fn stop_reason_stop_sequence() {
        let resp = r#"{"type":"message","id":"msg_1","role":"assistant","model":"m","content":[{"type":"text","text":"text"}],"stop_reason":"stop_sequence","usage":{"input_tokens":1,"output_tokens":5}}"#;
        let chat: Value = serde_json::from_str(&convert_anthropic_to_openai_response(resp).unwrap()).unwrap();
        assert_eq!(chat["choices"][0]["finish_reason"], "stop");
    }

    /// Anthropic: usage with cache_read_input_tokens → prompt_tokens_details.cached_tokens
    #[test]
    fn cached_tokens_mapping() {
        let resp = r#"{
            "type":"message","id":"msg_1","role":"assistant","model":"m",
            "content":[{"type":"text","text":"OK"}],
            "stop_reason":"end_turn",
            "usage":{"input_tokens":100,"output_tokens":50,"cache_read_input_tokens":30}
        }"#;
        let chat: Value = serde_json::from_str(&convert_anthropic_to_openai_response(resp).unwrap()).unwrap();
        assert_eq!(chat["usage"]["prompt_tokens"], 100);
        assert_eq!(chat["usage"]["completion_tokens"], 50);
        assert_eq!(chat["usage"]["prompt_tokens_details"]["cached_tokens"], 30);
    }

    /// Anthropic: error response → Chat error format
    #[test]
    fn error_response() {
        let resp = r#"{"type":"error","error":{"type":"invalid_request_error","message":"Invalid request"}}"#;
        let chat: Value = serde_json::from_str(&convert_anthropic_to_openai_response(resp).unwrap()).unwrap();
        assert_eq!(chat["error"]["type"], "invalid_request_error");
        assert_eq!(chat["error"]["message"], "Invalid request");
    }

    /// Anthropic: rate_limit_error → rate_limit_exceeded
    #[test]
    fn rate_limit_error_mapping() {
        let resp = r#"{"type":"error","error":{"type":"rate_limit_error","message":"Too many requests"}}"#;
        let chat: Value = serde_json::from_str(&convert_anthropic_to_openai_response(resp).unwrap()).unwrap();
        assert_eq!(chat["error"]["type"], "rate_limit_exceeded");
    }

    /// Anthropic: pause_turn stop_reason (new in spec)
    #[test]
    fn stop_reason_pause_turn() {
        let resp = r#"{"type":"message","id":"msg_1","role":"assistant","model":"m","content":[{"type":"text","text":"part"}],"stop_reason":"pause_turn","usage":{"input_tokens":1,"output_tokens":5}}"#;
        let chat: Value = serde_json::from_str(&convert_anthropic_to_openai_response(resp).unwrap()).unwrap();
        // pause_turn should map to "stop" (closest equivalent)
        assert_eq!(chat["choices"][0]["finish_reason"], "stop");
    }

    /// Anthropic: refusal stop_reason (new in spec)
    #[test]
    fn stop_reason_refusal() {
        let resp = r#"{"type":"message","id":"msg_1","role":"assistant","model":"m","content":[{"type":"text","text":"I can't do that"}],"stop_reason":"refusal","usage":{"input_tokens":1,"output_tokens":5}}"#;
        let chat: Value = serde_json::from_str(&convert_anthropic_to_openai_response(resp).unwrap()).unwrap();
        // refusal should map to "stop" (closest equivalent)
        assert_eq!(chat["choices"][0]["finish_reason"], "stop");
    }
}

// ============================================================================
// OpenAI Chat → Anthropic (Response)
// Based on OpenAI Chat Completion API spec
// ============================================================================

#[cfg(test)]
mod openai_to_anthropic_response {
    use super::*;

    /// Chat: simple text response → Anthropic format
    #[test]
    fn simple_text_response() {
        let resp = r#"{
            "id":"chatcmpl-abc123",
            "object":"chat.completion",
            "created":1677858242,
            "model":"gpt-4o",
            "choices":[{
                "index":0,
                "message":{"role":"assistant","content":"The weather is 72F."},
                "finish_reason":"stop"
            }],
            "usage":{"prompt_tokens":20,"completion_tokens":15,"total_tokens":35}
        }"#;
        let result = convert_openai_to_anthropic_response(resp).unwrap();
        let anthropic: Value = serde_json::from_str(&result).unwrap();

        assert_eq!(anthropic["type"], "message");
        assert_eq!(anthropic["role"], "assistant");
        assert_eq!(anthropic["model"], "gpt-4o");
        assert_eq!(anthropic["content"][0]["type"], "text");
        assert_eq!(anthropic["content"][0]["text"], "The weather is 72F.");
        assert_eq!(anthropic["stop_reason"], "end_turn");
        assert_eq!(anthropic["stop_sequence"], serde_json::Value::Null);
        assert_eq!(anthropic["usage"]["input_tokens"], 20);
        assert_eq!(anthropic["usage"]["output_tokens"], 15);
    }

    /// Chat: tool_calls response → Anthropic tool_use blocks
    #[test]
    fn tool_calls_response() {
        let resp = r#"{
            "id":"chatcmpl-abc123",
            "model":"gpt-4o",
            "choices":[{
                "index":0,
                "message":{
                    "role":"assistant",
                    "content":null,
                    "tool_calls":[{
                        "id":"call_abc123",
                        "type":"function",
                        "function":{"name":"get_weather","arguments":"{\"city\":\"NYC\"}"}
                    }]
                },
                "finish_reason":"tool_calls"
            }],
            "usage":{"prompt_tokens":20,"completion_tokens":15,"total_tokens":35}
        }"#;
        let result = convert_openai_to_anthropic_response(resp).unwrap();
        let anthropic: Value = serde_json::from_str(&result).unwrap();

        assert_eq!(anthropic["content"][0]["type"], "tool_use");
        assert_eq!(anthropic["content"][0]["id"], "call_abc123");
        assert_eq!(anthropic["content"][0]["name"], "get_weather");
        assert_eq!(anthropic["content"][0]["input"]["city"], "NYC");
        assert_eq!(anthropic["stop_reason"], "tool_use");
    }

    /// Chat: finish_reason → stop_reason mapping
    /// stop → end_turn, length → max_tokens, tool_calls → tool_use
    #[test]
    fn finish_reason_stop() {
        let resp = r#"{"id":"chatcmpl-1","model":"gpt-4o","choices":[{"index":0,"message":{"role":"assistant","content":"OK"},"finish_reason":"stop"}],"usage":{"prompt_tokens":5,"completion_tokens":2}}"#;
        let anthropic: Value = serde_json::from_str(&convert_openai_to_anthropic_response(resp).unwrap()).unwrap();
        assert_eq!(anthropic["stop_reason"], "end_turn");
    }

    #[test]
    fn finish_reason_length() {
        let resp = r#"{"id":"chatcmpl-1","model":"gpt-4o","choices":[{"index":0,"message":{"role":"assistant","content":"part"},"finish_reason":"length"}],"usage":{"prompt_tokens":5,"completion_tokens":100}}"#;
        let anthropic: Value = serde_json::from_str(&convert_openai_to_anthropic_response(resp).unwrap()).unwrap();
        assert_eq!(anthropic["stop_reason"], "max_tokens");
    }

    #[test]
    fn finish_reason_tool_calls() {
        let resp = r#"{"id":"chatcmpl-1","model":"gpt-4o","choices":[{"index":0,"message":{"role":"assistant","content":null,"tool_calls":[{"id":"call_1","type":"function","function":{"name":"fn","arguments":"{}"}}]},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":5,"completion_tokens":10}}"#;
        let anthropic: Value = serde_json::from_str(&convert_openai_to_anthropic_response(resp).unwrap()).unwrap();
        assert_eq!(anthropic["stop_reason"], "tool_use");
    }

    /// Chat: content=null with tool_calls → Anthropic content with tool_use only
    #[test]
    fn null_content_with_tool_calls() {
        let resp = r#"{
            "id":"chatcmpl-1","model":"gpt-4o",
            "choices":[{"index":0,"message":{"role":"assistant","content":null,"tool_calls":[{"id":"call_1","type":"function","function":{"name":"fn","arguments":"{}"}}]},"finish_reason":"tool_calls"}],
            "usage":{"prompt_tokens":5,"completion_tokens":10}
        }"#;
        let anthropic: Value = serde_json::from_str(&convert_openai_to_anthropic_response(resp).unwrap()).unwrap();
        // Anthropic requires content to be an array (even if empty for text blocks)
        // When there are tool_use blocks, they should be in the content array
        assert!(anthropic["content"].is_array());
        let has_tool_use = anthropic["content"].as_array().unwrap().iter()
            .any(|b| b["type"] == "tool_use");
        assert!(has_tool_use, "Should have tool_use block in content");
    }

    /// Chat: error response → Anthropic error format
    #[test]
    fn error_response() {
        let resp = r#"{"error":{"message":"Invalid API key","type":"authentication_error","code":"invalid_api_key"}}"#;
        let anthropic: Value = serde_json::from_str(&convert_openai_to_anthropic_response(resp).unwrap()).unwrap();
        assert_eq!(anthropic["type"], "error");
        assert_eq!(anthropic["error"]["type"], "authentication_error");
        assert_eq!(anthropic["error"]["message"], "Invalid API key");
    }

    /// Chat: usage with cached_tokens → cache_read_input_tokens
    #[test]
    fn cached_tokens_mapping() {
        let resp = r#"{
            "id":"chatcmpl-1","model":"gpt-4o",
            "choices":[{"index":0,"message":{"role":"assistant","content":"OK"},"finish_reason":"stop"}],
            "usage":{"prompt_tokens":100,"completion_tokens":50,"prompt_tokens_details":{"cached_tokens":30}}
        }"#;
        let anthropic: Value = serde_json::from_str(&convert_openai_to_anthropic_response(resp).unwrap()).unwrap();
        assert_eq!(anthropic["usage"]["input_tokens"], 100);
        assert_eq!(anthropic["usage"]["output_tokens"], 50);
        assert_eq!(anthropic["usage"]["cache_read_input_tokens"], 30);
    }

    /// Chat: content="" with no tool_calls → empty content array
    #[test]
    fn empty_content_no_tool_calls() {
        let resp = r#"{"id":"chatcmpl-1","model":"gpt-4o","choices":[{"index":0,"message":{"role":"assistant","content":""},"finish_reason":"stop"}],"usage":{"prompt_tokens":5,"completion_tokens":2}}"#;
        let anthropic: Value = serde_json::from_str(&convert_openai_to_anthropic_response(resp).unwrap()).unwrap();
        assert_eq!(anthropic["content"], serde_json::json!([]));
    }
}
