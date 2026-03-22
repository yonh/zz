---
status: active
horizon: current
workflow_stage: breakdown
next_command: /sync-active-work-with-code
last_reviewed: 2026-03-22
---

# ZZ - OpenAI Responses API 实施计划

## 版本：3.1.0
## 状态：Active
## 创建时间：2026-03-22
## 最后更新：2026-03-22

---

## 1. 交付目标

交付当前最小变更集，使 Codex 能稳定通过 ZZ 使用 OpenAI Responses API。

## 2. 范围

本实施计划**只覆盖当前 active 批次**。

包含：
- streaming 检测修复
- `api_type` 日志字段
- Responses 非流式 token usage 验证
- OpenAI 端到端验证

不包含：
- UI 观测增强
- path-based routing
- 协议转换层

## 3. 工作拆分

### 任务 1：修复 streaming 检测
**文件**：`src/stream.rs`、`src/proxy.rs`

#### 动作
1. 新增同时检查 header 与 body 的 helper
2. 将最终 streaming 判断移动到请求体收集之后
3. 保持既有 Chat API 行为不变

#### 验收
- 带 `"stream": true` 的请求被正确标记为 streaming
- 基于 header 的 streaming 检测继续可用

### 任务 2：给日志增加 `api_type`
**文件**：`src/stats.rs`、`src/proxy.rs`

#### 动作
1. 扩展 `LogEntry`
2. 根据 path 推导 `api_type`
3. 在日志创建路径中填充该字段

#### 验收
- `/v1/chat/completions` -> `chat`
- `/v1/responses` -> `responses`
- 其它路径 -> `other`

### 任务 3：验证 Responses token usage
**文件**：`src/proxy.rs` 测试

#### 动作
1. 为 Responses 非流式 usage payload 新增测试
2. 验证 `input_tokens` / `output_tokens` 能映射到现有统计模型

#### 验收
- 在不引入新 token parser 设计的前提下，测试通过

### 任务 4：端到端验证
**文件**：无

#### 动作
1. 验证非流式 `/v1/responses`
2. 验证 streaming `/v1/responses`
3. 验证 Chat API 回归路径
4. 验证 retryable 上游失败下的 failover 行为保持不变

#### 验收
- OpenAI Responses 请求能通过 ZZ 正常工作
- 既有 Chat API 行为不受影响

## 4. 顺序

1. 实现 streaming 检测修复
2. 增加 `api_type` 日志字段
3. 增加 Responses usage 测试
4. 执行手工端到端验证

## 5. 预计工时

| 任务 | 估时 |
|------|------|
| streaming 检测 | 1-2 小时 |
| `api_type` 日志 | 30-60 分钟 |
| token usage 验证 | 30-60 分钟 |
| 端到端验证 | 1-2 小时 |

**当前 active 批次总计**：约 1 天。

## 6. 质量关口

- `cargo test` 通过
- `cargo clippy` 通过
- Responses 请求经由 ZZ 访问 OpenAI 正常
- 既有 Chat API 行为没有回归

## 7. 延期后续项

不要把 roadmap 工作混入当前批次。后续内容请查看：
- `docs/roadmap/codex-responses/observability.md`
- `docs/roadmap/codex-responses/path-based-routing.md`
- `docs/roadmap/codex-responses/adapter-rfc.md`
