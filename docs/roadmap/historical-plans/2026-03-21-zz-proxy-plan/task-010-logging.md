---
status: deferred
horizon: long_term
workflow_stage: archived
next_command: /route-task-by-status
last_reviewed: 2026-03-22
---

# 任务 010：日志模块 - 结构化日志

## 目标

使用 `tracing` 实现可配置日志级别的结构化日志。

## BDD 场景

```gherkin
Scenario: Log request start with method and path
  Given client sends POST /v1/chat/completions
  When request starts processing
  Then log entry contains: level=INFO, method=POST, path=/v1/chat/completions

Scenario: Log provider selection
  Given router selects ali-account-1
  When proxy forwards request
  Then log entry contains: level=DEBUG, provider=ali-account-1, action=selected

Scenario: Log quota error with provider name
  Given ali-account-1 returns 429
  When error is detected
  Then log entry contains: level=WARN, provider=ali-account-1, error=quota_exhausted

Scenario: Log successful response with status
  Given upstream returns 200 OK
  When response is sent to client
  Then log entry contains: level=INFO, status=200, duration_ms=123

Scenario: Respect configured log level
  Given config.log_level = "warn"
  When debug message is logged
  Then debug message is not output
  And warn/error messages are output

Scenario: Structured JSON output
  Given log output format = JSON
  When any log entry is written
  Then output is valid JSON with fields: timestamp, level, message, and context fields
```

## 涉及文件

**创建**：
- `src/logging.rs` - 完整实现

## 历史实施步骤

1. 初始化 tracing subscriber：
   - 使用 `tracing_subscriber::fmt()` + JSON formatter
   - 从配置（或环境变量）读取日志级别
   - 增加 timestamp、target、span 支持

2. 定义结构化日志调用方式：
   - 使用 `tracing::info!`、`tracing::debug!` 等，附带结构化字段
   - 示例：`tracing::info!(method = %req.method(), path = %req.uri().path(), "request started")`

3. 在代码关键路径补日志点：
   - 请求开始 / 结束（含 duration）
   - provider 选择
   - provider 失败与健康状态变化
   - quota 检测
   - admin 端点访问
   - 配置 reload

4. 实现日志配置：
   - 从配置读取 `log_level`
   - 支持环境变量覆盖（如 `RUST_LOG`）
   - 默认级别为 `info`

5. 可选：增加 request ID 以便串联日志
   - 每个请求生成唯一 ID
   - 同一个请求链路上的日志都带上该 ID

## 历史验证方式

运行：

```bash
cargo build
RUST_LOG=debug cargo run 2>&1 | head -20
```

预期：
- 日志为结构化输出
- 日志级别过滤生效
- 关键事件都有日志记录

## 依赖
- 任务 001（`Cargo.toml` 中已有 tracing 依赖）
