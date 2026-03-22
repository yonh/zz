---
status: deferred
horizon: short_term
workflow_stage: scoped
next_command: /activate-roadmap-item
last_reviewed: 2026-03-22
---

# 延期事项 - SSE Streaming Token Usage 提取

## 状态

Deferred

## 为什么延期

当前代码整改任务的目标是修复已经确认的前后端接口不一致、错误语义不正确、provider runtime 状态丢失和虚假 UI 行为等问题。SSE streaming token usage 提取需要包装流式响应并在流结束时解析最后一个非 `[DONE]` chunk，改动面明显更大，不应阻塞当前整改批次归档。

## 范围

- 在 `src/proxy.rs` 的 SSE 路径中包装 streaming body
- 捕获最后一个非 `[DONE]` 的 SSE 数据块
- 解析其中可能存在的 `usage` 字段
- 如果提取成功，则触发 `provider.record_tokens()`
- 保证对现有 SSE 透传行为、延迟、错误处理无破坏

## 当前已知现实

- 当前 SSE 响应直接透传
- 非流式响应已经能提取 token usage
- 代码中已明确注释该限制是已知问题

## 为什么不能阻塞当前交付

- 当前主整改目标已经通过交付审核
- 该事项不影响已有 fix 的正确性
- 该事项涉及异步流包装和边界条件处理，测试成本更高

## 激活前置条件

- 需要明确 streaming provider 的 SSE 数据格式样例
- 需要补充覆盖 `[DONE]`、中途断流、缺失 `usage` 字段等路径的验证方案
- 需要确认不会影响现有非流式 token usage 统计

## 激活触发条件

当团队需要提升 streaming token 统计准确性，且能够投入单独验证 SSE 流包装行为时，再将其提升为新的 active-work 批次。
