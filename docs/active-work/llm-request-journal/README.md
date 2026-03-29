---
status: active
horizon: current
workflow_stage: breakdown
last_reviewed: 2026-03-26
---

# LLM Request Journal - 当前工作

## 当前交付范围

该目录用于规划“查看实际发往 LLM 的完整请求日志”能力。

## 当前目标

让用户能够查看 Claude、Codex、Cursor 或其他编辑器/SDK 实际发送到 ZZ 的**完整请求内容**，用于排查参数兼容性、代理透明性与上游行为差异。

## 文档

- [spec-01-backend-request-capture.md](./spec-01-backend-request-capture.md) - 后端全量捕获与持久化
- [spec-02-query-api-and-ui-viewer.md](./spec-02-query-api-and-ui-viewer.md) - 查询 API、导出与 UI 查看

## 当前范围内

- 捕获所有代理到上游 LLM 的请求
- 记录客户端标识（如 Claude / Codex / Cursor / generic SDK）
- 保存请求 headers、body、目标 provider、上游 URL、状态码等关键信息
- 提供按条件筛选、查看详情、导出

## 明确不做

- 不修改请求体内容
- 不把 secret headers 明文暴露到 UI
- 不在本批次做“全量 SSE 响应体回放”

## 实施建议

1. 先做 Spec 01，确保请求日志真实、完整、可持久化
2. 再做 Spec 02，把这套数据通过 API/UI 暴露出来