---
status: completed
horizon: short_term
workflow_stage: archived
next_command: /route-task-by-status
last_reviewed: 2026-03-22
---

# 代码整改 Spec

## 状态

Completed

## 完成摘要

- **完成日期**：2026-03-22
- **完成结论**：Pass（已通过 `/review-delivery-completion`）
- **完成范围**：FIX-01 到 FIX-09 已按 spec 落地，其中 FIX-06 完成阶段一（已知限制文档化）
- **已完成文件**：`src/admin_api.rs`、`src/provider.rs`、`src/proxy.rs`、`ui/src/pages/Providers.tsx`
- **后续事项**：SSE streaming token usage 提取阶段二已拆分到 `docs/roadmap/code-remediation/01-sse-streaming-token-usage.md`

## 防重复说明

本任务已经完成对当前代码修改的整改审核、spec 生成、实现核对与交付审核。
后续如果再次发现相同问题，不应重新开启本任务，而应：

1. 先核对当前代码是否已回归
2. 若是新回归，创建新的 bug 或 remediation 批次
3. 若是历史 follow-up，转到 roadmap 中的后续事项处理

---

## 原始整改内容

> 本文档基于对当前未提交代码修改（`src/admin_api.rs`、`src/provider.rs`、`src/proxy.rs`、`ui/src/api/client.ts`、`ui/src/api/mock.ts`、`ui/src/api/types.ts`、`ui/src/pages/Providers.tsx`、`ui/src/stores/store.ts`）的系统性审查产出。
>
> 所有条目均按优先级排序，实现 agent 应按此顺序执行。

## 整改条目总览

| ID | 级别 | 问题描述 | 涉及文件 |
|---|---|---|---|
| FIX-01 | Bug | provider test 接口 HTTP 方法前后端不一致 | `admin_api.rs` + `client.ts` |
| FIX-02 | Bug | config 校验失败未返回 HTTP 400 状态码 | `admin_api.rs` |
| FIX-03 | Bug | 前端 toggleProvider 未调用后端 API | `store.ts` + `Providers.tsx` |
| FIX-04 | Bug | 前端 addProvider / editProvider 未调用后端 API | `store.ts` + `Providers.tsx` |
| FIX-05 | Risk | provider 配置更新会重置全部 runtime 状态 | `provider.rs` |
| FIX-06 | Risk | SSE 响应的 token usage 完全不统计 | `proxy.rs` |
| FIX-07 | Quality | `mask_api_key` 输出格式冗余 | `admin_api.rs` |
| FIX-08 | Quality | `last_reloaded` 返回假值，与 `last_modified` 相同 | `admin_api.rs` |
| FIX-09 | Quality | `handleTestConnection` 是纯前端模拟，未调用后端接口 | `Providers.tsx` |

## 已完成范围说明

- FIX-01：已统一 provider test 接口为 `POST`
- FIX-02：已让非法 config 保存返回 `400`
- FIX-03：已让 enable/disable 走真实后端 API
- FIX-04：已让 add/edit provider 走真实后端 API
- FIX-05：已将 provider config 更新改为原地更新，保留 runtime 状态
- FIX-06：已完成阶段一，在代码中明确记录当前限制
- FIX-07：已修正 `mask_api_key` 输出格式
- FIX-08：已让 `last_reloaded` 使用真实值
- FIX-09：已让 Test Connection 使用真实后端接口

## 未完成但保留为后续事项

- SSE streaming token usage 提取阶段二
