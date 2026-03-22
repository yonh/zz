---
status: active
horizon: current
workflow_stage: breakdown
next_command: /sync-active-work-with-code
last_reviewed: 2026-03-23
---

# Token Usage 显示修复 Spec

## 状态

Active

## 目标

在不引入新架构层、不扩展观测系统范围的前提下，只修复控制台 `Logs` 页面中的 token usage 显示缺失问题。

## 当前批次范围

仅包含：
- `FIX-10`：streaming 检测补全（覆盖 body 中 `"stream": true`）
- `FIX-11`：非流式 `output.usage` 路径提取补全

不包含：
- `FIX-12`（timeseries 链路）
- `FIX-13`（`total_errors` 合约扩展）
- `FIX-14`（mock 状态路径清理）

## 整改条目总览（当前批次）

| ID | 级别 | 问题描述 | 涉及文件 |
|---|---|---|---|
| FIX-10 | Bug | streaming 检测只看 `Accept: text/event-stream`，未覆盖 body 中的 `"stream": true`，会导致 `/v1/messages` / `/v1/responses` 等请求被错误归类 | `src/proxy.rs` `src/stream.rs` `docs/active-work/codex-responses/tech-spec.md` |
| FIX-11 | Bug | 非流式 token usage 提取未覆盖 `output.usage` 这类嵌套格式，DashScope 兼容响应可能完全丢失 token 统计 | `src/proxy.rs` `docs/reference/tokens-system/04-technical-details.md` |

## 详细问题说明

### FIX-10 - streaming 检测条件不完整

**当前现实**
- `src/proxy.rs` 在读取完请求头后立刻调用 `is_sse_request(&req)`。
- `src/stream.rs` 当前只检查 `Accept` header 是否包含 `text/event-stream`。
- 当前活跃任务 `docs/active-work/codex-responses/tech-spec.md` 已明确写出这一缺口。

**问题影响**
- 当客户端通过 body 发送 `"stream": true`，但 header 未显式声明 SSE 时，请求可能被错误当作非流式处理。
- 这会影响 `/v1/messages`、`/v1/responses` 等接口的代理路径判断、日志 `streaming` 字段，以及后续 token usage 观测。

**必要变更**
- 在 `src/stream.rs` 增加同时检查 header 与 body 的 streaming helper。
- 在 `src/proxy.rs` 先读取 body，再基于 body + header 综合判断是否 streaming。
- 为该逻辑补充单元测试，覆盖 header-only、body-only、两者都缺失三种路径。

### FIX-11 - 非流式 token usage 提取遗漏嵌套路径

**当前现实**
- `src/proxy.rs` 的 `extract_token_usage()` 只检查：
  - 顶层 `usage`
  - 顶层 `prompt_tokens` / `input_tokens`
- `docs/reference/tokens-system/04-technical-details.md` 已记录 DashScope 非流式响应可能使用 `output.usage` 结构。

**问题影响**
- 对兼容 OpenAI 但把 usage 嵌套在 `output.usage` 的 provider，当前日志中的 `token_usage` 会为空。
- provider 级 token 聚合与日志面板 token 展示都将丢数据。

**必要变更**
- 扩展 `extract_token_usage()`，支持 `output.usage` 嵌套路径。
- 统一覆盖 `prompt_tokens` / `input_tokens` 与 `completion_tokens` / `output_tokens` 组合。
- 增加针对兼容响应样例的测试，避免未来回归。

## 最小可执行批次拆分

### 任务 1：修复 streaming 识别
- 目标：确保 body-only `"stream": true` 请求可被识别为 streaming。
- 涉及文件：`src/stream.rs`、`src/proxy.rs`
- 实现边界：仅补识别逻辑，不改路由策略、不改 failover 机制。
- 验收标准：
  - body-only `"stream": true` 请求记录为 `streaming=true`
  - header-based 检测保持兼容
- 回归检查：`/v1/chat/completions` 既有请求行为无回归。

### 任务 2：补齐 `output.usage` 提取
- 目标：非流式响应中的 token usage 不再因嵌套路径缺失而丢失。
- 涉及文件：`src/proxy.rs`
- 实现边界：仅扩展 `extract_token_usage()` 的路径覆盖，不引入新 parser 模块。
- 验收标准：
  - 顶层 `usage` 样例可提取
  - `output.usage` 样例可提取
- 回归检查：`input_tokens` / `output_tokens` 现有映射保持正确。

## 建议实施顺序

1. 先处理 **FIX-10**，补齐 streaming 检测口径。
2. 再处理 **FIX-11**，补齐非流式 token usage 提取路径。

## 验证建议

### 代码级验证
- 覆盖 `"stream": true` 的 body-only 请求。
- 覆盖 `output.usage` 非流式响应样例。

### 手工验证
- 发送 `POST /v1/messages` 与 `POST /v1/responses` 的 streaming / non-streaming 请求。
- 在 Logs 页面确认 `streaming` 标记与 `token_usage` 展示符合预期。

## 当前批次涉及文件

| 文件 | 关注点 |
|------|--------|
| `src/proxy.rs` | streaming 判断时机、token usage 提取路径 |
| `src/stream.rs` | streaming helper 口径 |

## 明确不做（本批次移除）

- 不处理 timeseries 统计链路。
- 不处理 `total_errors` 合约扩展。
- 不处理 mock WebSocket / mock store 清理。

以上内容全部移出当前批次，后续如需处理应单独建任务。
