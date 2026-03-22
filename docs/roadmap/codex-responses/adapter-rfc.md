---
status: deferred
horizon: long_term
workflow_stage: scoped
next_command: /activate-roadmap-item
last_reviewed: 2026-03-22
---

# 延期事项 - Responses 到 Chat Adapter RFC

## 状态
Deferred / High Risk

## 目标
探索 ZZ 是否应支持将 Responses API 请求转换为 Chat Completions 请求，从而让不原生支持 `/v1/responses` 的 provider 也能被使用。

## 为什么不属于当前交付
当前交付目标是让 Codex 通过透明代理方式使用 OpenAI Responses API。adapter 属于**协议转换产品能力**，而不是透明代理修复。

## 主要挑战
- Responses API 使用 `input`、`instructions`、typed `output` items 和不同的 SSE 事件语义
- Chat Completions 使用 `messages`、`choices` 和不同的 streaming 格式
- tools、reasoning items、`previous_response_id` 都没有干净的一一映射
- 错误处理与 retry 语义需要显式重设计

## 护栏
- 在当前 retry 模型下，不允许通过上游 404 自动触发 adapter
- adapter 行为必须是显式 opt-in，不允许隐式触发
- 当前 P0 批次不得引入 adapter 工作

## 最小 RFC 问题
1. v1 支持哪些字段？
2. v1 支持 streaming 还是只支持非 streaming？
3. 遇到不支持的能力时如何明确报错？
4. provider 选择如何控制？
5. payload 被转换后如何做观测与审计？

## 建议起步范围
如果未来真的开始做，应从以下最小范围起步：
- 仅非 streaming
- 仅简单文本输入
- 显式 provider allowlist
- 对 tools、reasoning items、stateful chaining 进行硬拒绝
