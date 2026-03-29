---
status: active
horizon: current
workflow_stage: breakdown
feature: llm-request-journal-backend
last_reviewed: 2026-03-26
---

# Spec 01: 后端全量捕获 LLM 请求日志

## 1. 问题陈述

当前 ZZ 已有 `LogEntry` 和 `/zz/api/logs`，但它只保存请求元数据：`path`、`provider`、`status`、`model`、`request_bytes` 等。它**看不到真实请求内容**，因此无法回答以下关键排障问题：

- Claude / Codex / Cursor 实际发来的 JSON body 是什么？
- 请求里是否真的包含 `thinking_budget`、`reasoning` 或其他敏感参数？
- 不同编辑器发来的 headers / user-agent / body 结构是否不同？
- ZZ 转发到哪个 provider、哪个 upstream URL，和失败状态是否一致？

用户明确要求查看“**所有日志**”。对这个需求来说，仅有内存 ring buffer 的 metadata log 不足够，必须增加**可持久化的原始请求日志**能力。

## 2. 目标

- 捕获所有经过 ZZ 代理到上游 LLM 的请求（不包含 `/zz/api/*` 管理接口）
- 对每个请求记录：客户端标识、headers、request body、选中的 provider、upstream URL、返回状态
- 日志落盘持久化，避免仅保留内存中的最近 10000 条
- 默认保留完整 request body，用于精确排查参数问题
- 对敏感 headers 做脱敏，避免泄露 API Key / Cookie / Authorization

## 3. 当前代码现实

| 组件 | 现状 | 缺口 |
|------|------|------|
| `src/proxy.rs` | 已在入口收集 `body_bytes`，且 request body 会被完整缓存用于重试 | ✓ 已有最佳捕获点 |
| `src/stats.rs::LogEntry` | 仅保存 metadata，不含 headers / body | ✗ 不足以排查请求参数问题 |
| `RequestLogBuffer` | 内存 ring buffer，容量 10000，重启即丢失 | ✗ 不满足“所有日志” |
| `src/logging.rs` | 只有 tracing 输出到 stderr | ✗ 不是结构化请求日志 |

## 4. 设计

### 4.1 新增配置

在 `src/config.rs` 中新增一组调试/观测配置，例如：

```toml
[observability.request_journal]
enabled = false
storage_dir = "logs/request-journal"
retention_days = 7
redact_headers = ["authorization", "x-api-key", "cookie", "set-cookie"]
```

说明：
- 默认 `enabled = false`，因为该能力会保存 prompt 与上下文，属于高敏感日志
- 一旦开启，应记录**所有** proxied LLM requests，而不是抽样
- `storage_dir` 为本地持久化目录

### 4.2 新增数据结构

新增独立于 `stats::LogEntry` 的结构，例如 `RequestJournalEntry`：

- `id`
- `timestamp`
- `client_name`（推断值：claude / codex / cursor / unknown）
- `user_agent`
- `method`
- `path`
- `provider`
- `upstream_url`
- `model`
- `streaming`
- `status`
- `request_headers`
- `request_content_type`
- `request_body_text`（UTF-8 / JSON 请求直接保存）
- `request_body_base64`（非 UTF-8 时回退）
- `request_bytes`
- `response_bytes`
- `failover_chain`
- `error`

`client_name` 基于 headers 做轻量推断，优先使用：
- `user-agent`
- Anthropic / OpenAI / editor 特征 header
- 无法识别时为 `unknown`

### 4.3 捕获时机

在 `src/proxy.rs` 中，复用当前 `body_bytes` 捕获点：

1. 收到请求并完成 `req.collect()` 后，立即生成“原始入站请求快照”
2. 选定 provider、重写 URL 后，补充 `provider` 与 `upstream_url`
3. 请求完成后补充 `status`、`response_bytes`、`failover_chain`
4. 即使请求失败（如 provider unavailable / upstream error），也必须落一条失败日志

### 4.4 持久化方式

不要把这类日志仅存在 `RequestLogBuffer` 中；需要单独落盘。

推荐目录结构：

```text
logs/request-journal/
  2026-03-26/
    req_xxx.json
    req_yyy.json
```

每个请求一份 JSON 文件，便于：
- 保留完整 body
- 后续按 `id` 精确读取
- 导出时直接拼接

本批次优先保证正确性与可读性，不强制要求一开始就做高吞吐 JSONL 索引优化。

### 4.5 脱敏规则

虽然用户要求“所有日志”，但 secret headers 仍需脱敏：

- `authorization`
- `x-api-key`
- `cookie`
- `set-cookie`

脱敏方式建议统一为：保留 header 名，值替换为 `"[REDACTED]"`。

注意：
- **不要**对 request body 做自动删改，否则会破坏“我到底发送了什么”的诊断价值
- ZZ 作为透明代理，本功能的核心就是保留 body 原貌

## 5. 不做

- 不在本批次记录完整 SSE 响应流内容
- 不做请求体字段级 redaction
- 不修改现有 `/zz/api/logs` 的 metadata 结构含义
- 不把日志持久化到数据库（文件系统优先）

## 6. 验收标准

- [ ] 开启 `observability.request_journal.enabled = true` 后，所有 proxied LLM requests 都会写入持久化日志
- [ ] 日志中可查看到 Claude / Codex / Cursor 等客户端的 `user-agent` 或可推断客户端标识
- [ ] 日志中可查看完整 request body，从而确认是否包含 `thinking_budget`
- [ ] 日志中可查看选中的 provider 与 upstream URL
- [ ] 上游失败 / 503 / failover 请求同样会落日志
- [ ] `authorization` 等敏感 headers 不以明文写入文件
- [ ] `cargo test` 通过
- [ ] `cargo clippy` 通过

## 7. 涉及文件

| 文件 | 变更类型 |
|------|----------|
| `src/config.rs` | 新增 request journal 配置 |
| `src/proxy.rs` | 在请求入口与完成点写入 journal |
| `src/logging.rs` 或新文件 | 新增 journal writer / redaction 辅助逻辑 |
| `src/main.rs` | 初始化 journal 组件并注入 `AppState` |

## 8. 预计工时

| 任务 | 估时 |
|------|------|
| 配置结构与初始化 | 20-30 分钟 |
| journal 数据结构与 writer | 45-60 分钟 |
| proxy.rs 接入与失败路径补全 | 45-60 分钟 |
| 测试验证 | 30-45 分钟 |
| **合计** | **约 2.5-3.5 小时** |