---
status: active
horizon: current
workflow_stage: scoped
next_command: /breakdown-active-work
last_reviewed: 2026-03-22
---

# ZZ - OpenAI Responses API 支持 PRD

## 版本：3.1.0
## 状态：Active
## 创建时间：2026-03-22
## 最后更新：2026-03-22

---

## 1. 问题陈述

在当前讨论的集成场景中，Codex 流量被视为使用 OpenAI Responses API（`POST /v1/responses`）。ZZ 当前本质上是一个 body-transparent 的反向代理，已经可以在不引入新路由架构的前提下，将 `/v1/responses` 请求转发到 OpenAI 兼容上游。

当前真正的缺口不在“能否转发”，而在于若干辅助行为对 Responses API 还不完整。

## 2. 已具备的能力

| 能力 | 当前状态 | 说明 |
|------|----------|------|
| 请求转发 | 可用 | 路径无关的 URL 重写已经能转发 `/v1/responses` |
| Header 重写 | 可用 | Authorization 与 Host 重写是路径无关的 |
| HTTP 429/5xx 失败切换 | 可用 | 当前 retry/failover 仍基于状态码 |
| 模型提取 | 可用 | Responses 请求也保留顶层 `model` 字段 |
| 非流式 token usage 解析 | 大概率可用 | `input_tokens` / `output_tokens` 已可映射到现有逻辑 |
| 流式字节透传 | 可用 | SSE 字节流按透明代理方式透传 |

## 3. 当前交付目标

本批交付的目标是确保以下链路稳定可用：

```text
Codex CLI -> ZZ proxy -> OpenAI /v1/responses
```

## 4. 当前范围

### 4.1 streaming 请求识别
当前 streaming 检测只看 `Accept` header，需要补充识别请求体中的 `"stream": true`。

### 4.2 日志中的 API 类型
请求日志需要区分：
- `chat`
- `responses`
- `other`

### 4.3 非流式 token usage 验证
需要针对 Responses 非流式响应验证当前 token 提取逻辑是否正确。这主要是验证任务，而不是重新设计 token parser。

### 4.4 端到端验证
必须用真实 OpenAI provider 配置完成当前交付验证。

## 5. 明确不做

以下内容明确不属于当前交付批次：

- UI 观测增强
- 按 API 类型拆分 dashboard 统计
- path-based routing
- Responses-to-Chat adapter
- Chat-to-Responses adapter
- 为未来能力扩展新的配置 schema

这些内容统一保留在 `docs/roadmap/codex-responses/`。

## 6. 成功标准

- [ ] `POST /v1/responses` 能通过 ZZ 成功访问 OpenAI
- [ ] Responses streaming 请求能被正确识别
- [ ] 日志项包含正确的 `api_type`
- [ ] 非流式 Responses token usage 有测试验证
- [ ] 既有 Chat API 行为没有回归

## 7. 约束

- 必须保持 ZZ 的透明代理架构
- 当前批次不得引入协议转换层
- 当前批次不得为了未来能力扩大配置 schema
