---
status: deferred
horizon: short_term
workflow_stage: scoped
next_command: /activate-roadmap-item
last_reviewed: 2026-03-22
---

# 延期事项 - 观测增强

## 状态
Deferred

## 为什么延期
当前交付目标是让 Codex 通过 ZZ 稳定使用 OpenAI Responses API。观测增强有助于调试和运维，但不是验证主链路可用的必要条件。

## 范围
- 在 UI 中显示 `api_type` 徽标
- 给 `/zz/api/stats` 增加按 API 类型拆分的统计
- 扩展 WebSocket `StatsSnapshot`
- 增加 `chat` / `responses` / `other` 过滤能力

## 为什么不能阻塞当前交付
- 它不影响请求转发正确性
- 它会同时触及后端统计状态、admin API、WebSocket payload 和 UI
- 它会扩大改动面和回归风险，但对当前主目标并非阻塞项

## 开始前置条件
- P0 级 OpenAI Responses API 主链路已完成端到端验证
- 请求日志中已经存在 `api_type`
- UI 目标文件已经与当前前端结构核对过

## 激活触发条件
当团队确认 Codex -> OpenAI 的主链路已稳定，且需要更强观测能力时，再激活该事项。
