//! # Test-Driven Guide: Responses ↔ Chat Completion 协议转换
//!
//! 测试分级 (TDD 实现路径):
//!   Level 0: Converter 单元测试 - 纯函数，无需启动服务
//!   Level 1: 完整 http 往返测试 - 启动 zz + mock upstream
//!   Level 2: 流式转换测试 - SSE chunk 级别
//!
//! "先实现能用，在实现细节" — 每个 Level 从最简单的 case 开始
//! 实现策略: 先让 Level-0-* 全部变绿 → Level-1-* → Level-2-*
//! ====================================================================

mod common;

use serde_json::Value;

// ============================================================================
// Level 0-A: Responses → Chat 请求转换
// 目标: 验证 Responses API 请求体能正确转为 Chat API 请求体
// ============================================================================

/// Test 0-A-1: 最简单的 string input
#[test]
fn test_r2c_simple_string_input() {
    let responses_req = r#"{"input":"Hello","model":"gpt-4o"}"#;
    let result = convert_responses_to_chat_request(responses_req);
    assert!(result.is_ok(), "Simple string input should convert: {:?}", result.err());
    let chat: Value = serde_json::from_str(&result.unwrap()).unwrap();
    assert_eq!(chat["messages"][0]["role"], "user");
    assert_eq!(chat["messages"][0]["content"], "Hello");
    assert_eq!(chat["model"], "gpt-4o");
}

/// Test 0-A-2: instructions → system message (插入最前面)
#[test]
fn test_r2c_instructions_becomes_system() {
    let responses_req = r#"{"input":"Hi","model":"gpt-4o","instructions":"Be concise."}"#;
    let result = convert_responses_to_chat_request(responses_req);
    assert!(result.is_ok());
    let chat: Value = serde_json::from_str(&result.unwrap()).unwrap();
    assert_eq!(chat["messages"][0]["role"], "system");
    assert_eq!(chat["messages"][0]["content"], "Be concise.");
    assert_eq!(chat["messages"][1]["role"], "user");
}

/// Test 0-A-3: input 数组 (含 developer role)
#[test]
fn test_r2c_input_array_with_developer() {
    let responses_req = include_str!("fixtures/responses_with_history_request.json");
    let result = convert_responses_to_chat_request(responses_req);
    assert!(result.is_ok());
    let chat: Value = serde_json::from_str(&result.unwrap()).unwrap();
    assert_eq!(chat["messages"][0]["role"], "system");       // from instructions
    assert_eq!(chat["messages"][1]["role"], "system");       // developer → system
    assert_eq!(chat["messages"][2]["role"], "user");
}

/// Test 0-A-4: max_output_tokens → max_tokens
#[test]
fn test_r2c_max_output_tokens_mapping() {
    let responses_req = r#"{"input":"Test","model":"gpt-4o","max_output_tokens":200}"#;
    let result = convert_responses_to_chat_request(responses_req);
    assert!(result.is_ok());
    let chat: Value = serde_json::from_str(&result.unwrap()).unwrap();
    assert_eq!(chat["max_tokens"], 200);
}

/// Test 0-A-5: tools + tool_choice 映射
#[test]
fn test_r2c_tools_mapping() {
    let responses_req = r#"{
        "input":"Weather?","model":"gpt-4o",
        "tools":[{"type":"function","name":"get_weather","description":"Get weather","parameters":{"type":"object"}}],
        "tool_choice":"auto"
    }"#;
    let result = convert_responses_to_chat_request(responses_req);
    assert!(result.is_ok());
    let chat: Value = serde_json::from_str(&result.unwrap()).unwrap();
    assert!(chat["tools"][0]["function"]["name"] == "get_weather"
        || chat["tools"][0]["name"] == "get_weather");
    assert_eq!(chat["tool_choice"], "auto");
}

/// Test 0-A-6: stream 字段保留
#[test]
fn test_r2c_preserves_stream() {
    let responses_req = r#"{"input":"Hi","model":"gpt-4o","stream":true}"#;
    let result = convert_responses_to_chat_request(responses_req);
    assert!(result.is_ok());
    let chat: Value = serde_json::from_str(&result.unwrap()).unwrap();
    assert_eq!(chat["stream"], true);
}

/// Test 0-A-7: previous_response_id 丢弃
#[test]
fn test_r2c_previous_response_id_dropped() {
    let responses_req = r#"{"input":"Hi","model":"gpt-4o","previous_response_id":"resp_abc123"}"#;
    let result = convert_responses_to_chat_request(responses_req);
    assert!(result.is_ok());
    let chat: Value = serde_json::from_str(&result.unwrap()).unwrap();
    assert!(chat.get("previous_response_id").is_none());
}

/// Test 0-A-8: temperature/top_p 透传
#[test]
fn test_r2c_passthrough_params() {
    let responses_req = r#"{"input":"Hi","model":"gpt-4o","temperature":0.5,"top_p":0.9}"#;
    let result = convert_responses_to_chat_request(responses_req);
    assert!(result.is_ok());
    let chat: Value = serde_json::from_str(&result.unwrap()).unwrap();
    assert_eq!(chat["temperature"], 0.5);
    assert_eq!(chat["top_p"], 0.9);
}

/// Test 0-A-9: metadata 透传
#[test]
fn test_r2c_metadata_passthrough() {
    let responses_req = r#"{"input":"Hi","model":"gpt-4o","metadata":{"user_id":"abc"}}"#;
    let result = convert_responses_to_chat_request(responses_req);
    assert!(result.is_ok());
    let chat: Value = serde_json::from_str(&result.unwrap()).unwrap();
    assert_eq!(chat["metadata"]["user_id"], "abc");
}

/// Test 0-A-10: store 字段丢弃
#[test]
fn test_r2c_store_dropped() {
    let responses_req = r#"{"input":"Hi","model":"gpt-4o","store":true}"#;
    let result = convert_responses_to_chat_request(responses_req);
    assert!(result.is_ok());
    let chat: Value = serde_json::from_str(&result.unwrap()).unwrap();
    assert!(chat.get("store").is_none());
}

/// Test 0-A-11: stop sequences 透传
#[test]
fn test_r2c_stop_sequences() {
    let responses_req = r#"{"input":"Hi","model":"gpt-4o","stop":["END"]}"#;
    let result = convert_responses_to_chat_request(responses_req);
    assert!(result.is_ok());
    let chat: Value = serde_json::from_str(&result.unwrap()).unwrap();
    assert_eq!(chat["stop"][0], "END");
}


// ============================================================================
// Level 0-B: Chat → Responses 响应转换
// 目标: 验证上游 Chat API 响应能被转回 Responses API 格式
// ============================================================================

/// Test 0-B-1: 简单文本响应
#[test]
fn test_c2r_simple_text_response() {
    let chat_resp = r#"{"id":"chatcmpl-abc","object":"chat.completion","created":1748332800,"model":"gpt-4o","choices":[{"index":0,"message":{"role":"assistant","content":"Hi!"},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":2}}"#;
    let result = convert_chat_to_responses_response(chat_resp);
    assert!(result.is_ok(), "Simple text response should convert: {:?}", result.err());
    let resp: Value = serde_json::from_str(&result.unwrap()).unwrap();
    assert_eq!(resp["object"], "response");
    assert!(resp["id"].as_str().unwrap_or("").starts_with("resp_"),
        "id should start with resp_, got: {:?}", resp["id"]);
    assert_eq!(resp["output"][0]["type"], "message");
    assert_eq!(resp["output"][0]["content"][0]["type"], "output_text");
    assert_eq!(resp["output"][0]["content"][0]["text"], "Hi!");
    assert_eq!(resp["usage"]["input_tokens"], 10);
    assert_eq!(resp["usage"]["output_tokens"], 2);
}

/// Test 0-B-2: tool_calls → function_call output
#[test]
fn test_c2r_tool_call_response() {
    let chat_resp = r#"{"id":"chatcmpl-abc","object":"chat.completion","created":1748332800,"model":"gpt-4o","choices":[{"index":0,"message":{"role":"assistant","content":null,"tool_calls":[{"id":"call_1","type":"function","function":{"name":"get_weather","arguments":"{\"location\":\"Tokyo\"}"}}]},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":50,"completion_tokens":10}}"#;
    let result = convert_chat_to_responses_response(chat_resp);
    assert!(result.is_ok());
    let resp: Value = serde_json::from_str(&result.unwrap()).unwrap();
    let has_fc = resp["output"].as_array().unwrap().iter().any(|o| o["type"] == "function_call");
    assert!(has_fc, "Should contain function_call output item");
}

/// Test 0-B-3a: finish_reason: stop → end_turn
#[test]
fn test_c2r_finish_reason_stop() {
    let chat_resp = r#"{"id":"chatcmpl-abc","object":"chat.completion","created":1748332800,"model":"gpt-4o","choices":[{"index":0,"message":{"role":"assistant","content":"OK"},"finish_reason":"stop"}],"usage":{"prompt_tokens":5,"completion_tokens":2}}"#;
    let resp: Value = serde_json::from_str(&convert_chat_to_responses_response(chat_resp).unwrap()).unwrap();
    assert_eq!(resp["output"][0]["stop_reason"], "end_turn");
}

/// Test 0-B-3b: finish_reason: length → max_tokens
#[test]
fn test_c2r_finish_reason_length() {
    let chat_resp = r#"{"id":"chatcmpl-abc","object":"chat.completion","created":1748332800,"model":"gpt-4o","choices":[{"index":0,"message":{"role":"assistant","content":"part"},"finish_reason":"length"}],"usage":{"prompt_tokens":5,"completion_tokens":100}}"#;
    let resp: Value = serde_json::from_str(&convert_chat_to_responses_response(chat_resp).unwrap()).unwrap();
    assert_eq!(resp["output"][0]["stop_reason"], "max_tokens");
}

/// Test 0-B-4: model + created 透传
#[test]
fn test_c2r_metadata_passthrough() {
    let chat_resp = r#"{"id":"chatcmpl-abc","object":"chat.completion","created":1700000000,"model":"gpt-4-turbo","choices":[{"index":0,"message":{"role":"assistant","content":"OK"},"finish_reason":"stop"}],"usage":{"prompt_tokens":5,"completion_tokens":2}}"#;
    let resp: Value = serde_json::from_str(&convert_chat_to_responses_response(chat_resp).unwrap()).unwrap();
    assert_eq!(resp["created"], 1700000000);
    assert_eq!(resp["model"], "gpt-4-turbo");
}

/// Test 0-B-5: usage 映射 (prompt_tokens→input_tokens, completion_tokens→output_tokens)
#[test]
fn test_c2r_usage_mapping() {
    let chat_resp = r#"{"id":"chatcmpl-abc","object":"chat.completion","created":1748332800,"model":"gpt-4o","choices":[{"index":0,"message":{"role":"assistant","content":"OK"},"finish_reason":"stop"}],"usage":{"prompt_tokens":100,"completion_tokens":50}}"#;
    let resp: Value = serde_json::from_str(&convert_chat_to_responses_response(chat_resp).unwrap()).unwrap();
    assert_eq!(resp["usage"]["input_tokens"], 100);
    assert_eq!(resp["usage"]["output_tokens"], 50);
}


// ============================================================================
// Level 0-C: Chat → Responses 请求转换 (对称方向，Phase 3)
// ============================================================================

/// Test 0-C-1: Chat messages → Responses input (system → developer)
#[test]
#[ignore = "Phase 3"]
fn test_c2r_request_messages_to_input() {
    let chat_req = r#"{"model":"gpt-4o","messages":[{"role":"system","content":"You are helpful"},{"role":"user","content":"Hello"}]}"#;
    let result = convert_chat_to_responses_request(chat_req);
    assert!(result.is_ok());
    let resp: Value = serde_json::from_str(&result.unwrap()).unwrap();
    assert!(resp["input"].is_array());
    assert_eq!(resp["input"][0]["type"], "message");
    assert_eq!(resp["input"][0]["role"], "developer");
}

/// Test 0-C-2: max_tokens → max_output_tokens
#[test]
#[ignore = "Phase 3"]
fn test_c2r_request_max_tokens() {
    let chat_req = r#"{"model":"gpt-4o","messages":[{"role":"user","content":"Hi"}],"max_tokens":500}"#;
    let result = convert_chat_to_responses_request(chat_req);
    assert!(result.is_ok());
    let resp: Value = serde_json::from_str(&result.unwrap()).unwrap();
    assert_eq!(resp["max_output_tokens"], 500);
}


// ============================================================================
// Level 0-D: Responses → Chat 响应转换 (对称方向，Phase 3)
// ============================================================================

/// Test 0-D-1: Responses output → Chat choices
#[test]
#[ignore = "Phase 3"]
fn test_r2c_response_output_to_choices() {
    let resp_body = r#"{"id":"resp_abc","object":"response","created":1748332800,"model":"gpt-4o","output":[{"type":"message","id":"msg_1","role":"assistant","content":[{"type":"output_text","text":"Hello!","annotations":[]}]}],"usage":{"input_tokens":10,"output_tokens":2}}"#;
    let result = convert_responses_to_chat_response(resp_body);
    assert!(result.is_ok());
    let chat: Value = serde_json::from_str(&result.unwrap()).unwrap();
    assert_eq!(chat["choices"][0]["message"]["content"], "Hello!");
    assert_eq!(chat["choices"][0]["finish_reason"], "stop");
    assert_eq!(chat["usage"]["prompt_tokens"], 10);
    assert_eq!(chat["usage"]["completion_tokens"], 2);
}


// ============================================================================
// Level 1: 完整 HTTP 往返测试 (需要 zz 进程 + mock upstream)
// #[ignore] 直到 Phase 2 + Phase 5
// ============================================================================

/// Test 1-A: 最简单往返 — Responses → zz → mock(Chat) → zz → Responses
#[tokio::test]
#[ignore = "Phase 2 + Phase 5"]
async fn test_r2c_full_roundtrip_simple() {
    let _url = format!("test placeholder");
    assert!(true, "Implement when Phase 2 + Phase 5 complete");
}

/// Test 1-B: 工具调用往返
#[tokio::test]
#[ignore = "Phase 2 + Phase 5"]
async fn test_r2c_roundtrip_tool_call() {
    assert!(true, "Implement when Phase 2 + Phase 5 complete");
}

/// Test 1-C: 上游错误传播
#[tokio::test]
#[ignore = "Phase 2 + Phase 5"]
async fn test_r2c_upstream_error() {
    assert!(true, "Implement when Phase 2 + Phase 5 complete");
}

/// Test 1-D: GET /r2c/models
#[tokio::test]
#[ignore = "models endpoint"]
async fn test_r2c_models_endpoint() {
    assert!(true, "Implement when models endpoint done");
}


// ============================================================================
// Level 2: 流式 SSE 转换 (Phase 4)
// ============================================================================

/// Test 2-A: Responses SSE event → Chat SSE data
#[tokio::test]
#[ignore = "Phase 4"]
async fn test_streaming_responses_to_chat() {
    assert!(true, "Replace with StreamConverter test when Phase 4 implemented");
}

/// Test 2-B: Chat SSE data → Responses SSE event
#[tokio::test]
#[ignore = "Phase 4"]
async fn test_streaming_chat_to_responses() {
    assert!(true, "Replace with StreamConverter test when Phase 4 implemented");
}

/// Test 2-C: 完整流式事件序列
#[tokio::test]
#[ignore = "Phase 4"]
async fn test_streaming_full_sequence() {
    assert!(true, "Replace with StreamConverter test when Phase 4 implemented");
}

/// Test 2-D: 跨 chunk 边界缓冲
#[tokio::test]
#[ignore = "Phase 4"]
async fn test_streaming_partial_chunks() {
    assert!(true, "Replace with StreamConverter test when Phase 4 implemented");
}


// ============================================================================
// 内部辅助函数 (Test Stubs)
// 实现 Phase 2/3 时替换为真实 zz::converter 调用
// ============================================================================

fn convert_responses_to_chat_request(body: &str) -> Result<String, String> {
    use zz::converter::{ApiConverter, ApiType, OpenAIResponsesToChatConverter};
    let converter = OpenAIResponsesToChatConverter;
    let bytes = bytes::Bytes::from(body.to_string());
    converter.convert_request(&bytes, ApiType::OpenAIChat)
        .map(|b| String::from_utf8_lossy(&b).to_string())
        .map_err(|e| e.to_string())
}

fn convert_chat_to_responses_response(body: &str) -> Result<String, String> {
    use zz::converter::{ApiConverter, ApiType, OpenAIResponsesToChatConverter};
    let converter = OpenAIResponsesToChatConverter;
    let bytes = bytes::Bytes::from(body.to_string());
    converter.convert_response(&bytes, ApiType::OpenAIChat, ApiType::OpenAIResponses, false)
        .map(|b| String::from_utf8_lossy(&b).to_string())
        .map_err(|e| e.to_string())
}

fn convert_chat_to_responses_request(body: &str) -> Result<String, String> {
    // TODO Phase 3: 替换为 zz::converter::OpenAIChatToResponsesConverter::convert_request
    Err("NOT_IMPLEMENTED [Phase3]".to_string())
}

fn convert_responses_to_chat_response(body: &str) -> Result<String, String> {
    // TODO Phase 3: 替换为 zz::converter::OpenAIChatToResponsesConverter::convert_response
    Err("NOT_IMPLEMENTED [Phase3]".to_string())
}

async fn start_zz(upstream_url: &str) -> (ZZShutdown, String) {
    let config_toml = common::zz_config(upstream_url);
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    std::fs::write(&config_path, &config_toml).unwrap();
    // TODO Phase 5: 启动 zz 进程
    (ZZShutdown { _dir: dir }, "http://127.0.0.1:9091".to_string())
}

struct ZZShutdown {
    _dir: tempfile::TempDir,
}

impl ZZShutdown {
    async fn shutdown(self) {
        // TODO Phase 5: 停止 zz
    }
}
