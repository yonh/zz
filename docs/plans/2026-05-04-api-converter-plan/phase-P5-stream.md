# Phase P5 — 流式 SSE 双向转换

**Depends on:** P4
**Type:** impl
**Goal:** 在 `conversion_proxy_handler` 检测到 `stream:true` / `text/event-stream` 时启用增量 SSE 转换。

---

## Scope（严格按 `field-mapping.md` §4 执行）

### 1. 数据结构

```rust
pub struct StreamConverter {
    source: ApiType,
    target: ApiType,
    state: StreamState,
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
    tool_blocks: HashMap<u32 /*oa index*/, ToolBlockState>,
    cumulative_input_tokens: Option<u64>,
    cumulative_output_tokens: u64,
    finished: bool,
}
```

### 2. SSE 解析

- 按字节读取 chunk → push 到内部 buffer → 按 `\n\n` 切事件 → 每事件解析 `event:` / `data:` 行。
- 跨 chunk 半包：未遇到 `\n\n` 之前保留缓冲。
- `data: [DONE]` 终止：触发 finalize。
- 非 JSON `data` 行 + 未识别事件：透传 + warn（短码 `sse_parse`，非致命）。

### 3. OpenAI → Anthropic 状态机

- 首个 chunk 含 `id`/`model` → 发 `message_start`。
- 首次 `delta.content` 非空 → 发 `content_block_start(index=0,type=text)` + `content_block_delta(text_delta)`，置 `text_block_open=true`。
- 后续 `delta.content` → `content_block_delta(text_delta)`。
- `delta.tool_calls[i]`：
  - 第一次见某个 `i`：若 text_block_open 先发 `content_block_stop`（关闭 text）；分配新 block_index，发 `content_block_start(type=tool_use,id,name,input:{})`；用 `function.arguments` 累加 `input_json_delta`。
  - 后续：继续发 `input_json_delta`。
- 收到 `finish_reason`：关闭所有未关闭 block；发 `message_delta(stop_reason, usage.output_tokens=cumulative)` + `message_stop`；置 `finished=true`。
- `[DONE]` 兜底：若未 finished，按 `end_turn` finalize。

### 4. Anthropic → OpenAI 状态机（反向）

镜像规则：
- `message_start` → 缓存 id/model；首个 OpenAI chunk delta 中带 `role:"assistant"`。
- `content_block_start(text)` → 不直接发出，等 delta。
- `content_block_delta(text_delta)` → OpenAI `delta.content`。
- `content_block_start(tool_use,id,name)` + 累计 `input_json_delta` → OpenAI `delta.tool_calls[i]`，`function.arguments` 拼接。
- `message_delta.stop_reason` → 反向映射为 `finish_reason`，附在最后一个 chunk 上。
- `message_stop` → 发 `data: [DONE]`。

### 5. 错误与降级

- 状态机异常（如 `content_block_delta` 前未 start）：
  - 若已发出 `message_start`：尝试发送 `message_delta(stop_reason=end_turn)` + `message_stop` 优雅收尾，并日志 `error_code=sse_state`。
  - 若一字节都未发出：返回上层走 P6 降级（响应侧降级 → 透传上游原始流）。
- 上游断流：发送 `message_stop`（已开始）或透传（未开始）。
- 单事件解析失败：透传该事件原文，warn `sse_parse`，继续。

## Files Touched

- `src/converter/stream.rs`（新增）
- `src/converter/mod.rs`（导出）
- `src/proxy.rs`（流式分支接入：包装 upstream body stream，调用 `StreamConverter::push(chunk)` 输出 `Vec<Bytes>`）
- `tests/converter_stream_oa_to_an.rs`
- `tests/converter_stream_an_to_oa.rs`
- `tests/integration_stream_a2o.rs`

## Acceptance Criteria

- 单测 ≥6：纯文本流、单 tool_call、多 tool_calls、文本+tool_call 混合、半包跨 chunk、上游中途断流。
- 录制真实样本（脱敏）放 `tests/fixtures/sse/`，用于回放对比。
- 集成测试：`/a2o/v1/messages` `stream:true` → 客户端逐行收到合法 Anthropic SSE 事件序列（顺序与字段断言）。
- finalize 后必须发 `message_stop` 且 SSE 流以 `\n\n` 结束。
- `cargo test` 全绿。

## Non-Goals

- Audio/video 流。
- OpenAI 流式 `usage` 缺失场景下的精确 token 推断（首版 output_tokens 用累计字符近似或填 0 + warn）。
