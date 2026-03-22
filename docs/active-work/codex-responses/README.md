---
status: active
horizon: current
workflow_stage: breakdown
next_command: /breakdown-active-work
last_reviewed: 2026-03-22
---

# Codex Responses - 当前工作

## 当前交付范围

该目录仅包含 OpenAI Responses API 支持任务的**当前交付批次**。

## 当前目标

在不破坏 ZZ 透明代理架构的前提下，让 Codex 稳定通过 ZZ 使用 OpenAI Responses API。

## 文档
- [prd.md](./prd.md) - 当前交付需求
- [tech-spec.md](./tech-spec.md) - 当前交付技术设计
- [implementation-plan.md](./implementation-plan.md) - 当前交付实施计划

## 当前范围内
- streaming 检测修复
- `api_type` 日志字段
- 非流式 token usage 验证
- OpenAI 端到端验证

## 明确不做
以下内容不属于当前批次：
- UI 观测增强
- path-based routing
- Responses-to-Chat adapter

这些后续内容请查看 `docs/roadmap/codex-responses/`。
