---
status: active
horizon: current
workflow_stage: breakdown
feature: llm-request-journal-viewer
last_reviewed: 2026-03-26
---

# Spec 02: 请求日志查询 API、导出与 UI 查看器

## 1. 问题陈述

即使后端已经把完整请求日志落盘，如果没有查询接口和查看入口，用户仍然无法高效排查问题。当前 `Logs` 页面和 `/zz/api/logs` API 的定位是“运行态 metadata 监控”，不是“原始请求取证工具”。

因此需要新增一套面向诊断的查看能力，支持：

- 按客户端查看（Claude / Codex / Cursor / unknown）
- 按 provider / model / path / 状态过滤
- 打开某一条请求查看 headers 和 body 详情
- 导出日志做离线 grep / 分析

## 2. 目标

- 提供 request journal 的列表、详情、导出 API
- UI 中能清晰区分“metadata logs”和“raw request journal”
- 用户可以直接在 UI 中检查某条请求是否带了 `thinking_budget`
- 支持按客户端或关键维度快速缩小范围

## 3. 当前代码现实

| 组件 | 现状 | 缺口 |
|------|------|------|
| `GET /zz/api/logs` | 仅返回 `LogEntry[]` metadata | ✗ 没有 body / headers / detail |
| `ui/src/pages/Logs.tsx` | 适合看实时状态，不适合看完整 body | ✗ 信息密度和交互不够 |
| `ui/src/stores/store.ts` | 已有日志初始化与实时追加机制 | ✓ 可复用 API 拉取模式 |

## 4. 设计

### 4.1 新增 API

#### `GET /zz/api/request-journal`

分页返回请求日志摘要列表。

查询参数建议支持：
- `limit`
- `offset`
- `client`
- `provider`
- `model`
- `status`
- `path`
- `date`

摘要列表应包含：
- `id`
- `timestamp`
- `client_name`
- `user_agent`
- `provider`
- `model`
- `path`
- `status`
- `streaming`
- `request_bytes`

#### `GET /zz/api/request-journal/{id}`

返回单条完整请求日志详情，包括：
- request headers
- request body
- upstream URL
- failover chain
- error

#### `GET /zz/api/request-journal/export`

按筛选条件导出 JSON 文件或 JSONL，便于离线搜索。

### 4.2 UI 形态

不要把这个能力硬塞进现有 `Logs` 页面；建议新增独立页面，例如：

- 路由：`/request-journal`
- 导航名：`Request Journal`

理由：
- 当前 `Logs` 页面偏实时监控
- 请求 journal 偏诊断/取证，信息结构完全不同
- 独立页面更容易加入大文本 body 预览、复制、下载等交互

### 4.3 页面结构

页面建议分为两块：

1. **上方过滤栏**
   - client
   - provider
   - model
   - status
   - path 关键词
   - 日期

2. **下方表格 + 详情面板**
   - 左侧/上方：请求摘要列表
   - 右侧/展开区：完整详情

详情区至少展示：
- Request headers（已脱敏）
- Request body（原始文本，支持 copy）
- Upstream URL
- Provider
- Failover chain
- Error / status

### 4.4 关键交互

- 点击某条记录可展开详情
- body 支持“复制”与“下载”
- 支持导出当前过滤结果
- 对 JSON body 自动 prettify；非 JSON 则原样文本展示

### 4.5 与现有 Logs 的关系

保留现有：
- `/zz/api/logs`
- `ui/src/pages/Logs.tsx`

新增 request journal 后，两者职责明确分离：

- **Logs**：运行指标与请求元数据
- **Request Journal**：完整请求内容与诊断详情

## 5. 不做

- 不在本批次做 WebSocket 实时 tail request body
- 不做跨多日超大规模全文检索引擎
- 不做请求回放/replay 功能

## 6. 验收标准

- [ ] `GET /zz/api/request-journal` 能返回请求摘要列表并支持基础过滤
- [ ] `GET /zz/api/request-journal/{id}` 能返回某条请求的完整 headers/body 详情
- [ ] UI 中新增独立页面查看 request journal
- [ ] UI 中可直接查看某条请求是否包含 `thinking_budget`
- [ ] UI 中可按 `client_name` 区分 Claude / Codex / Cursor / unknown
- [ ] 支持导出当前筛选范围内的日志
- [ ] `cargo test` 通过
- [ ] `cargo clippy` 通过

## 7. 涉及文件

| 文件 | 变更类型 |
|------|----------|
| `src/admin_api.rs` | 新增 request journal 列表 / 详情 / 导出端点 |
| `ui/src/App.tsx` | 新增页面路由 |
| `ui/src/components/layout/Layout.tsx` | 新增导航入口 |
| `ui/src/api/client.ts` | 新增 request journal API client |
| `ui/src/api/types.ts` | 新增 request journal 类型 |
| `ui/src/pages/RequestJournal.tsx` | 新增诊断页面 |
| `ui/src/stores/store.ts` | 新增 request journal 状态与加载逻辑（如需要） |

## 8. 预计工时

| 任务 | 估时 |
|------|------|
| API 查询 / 详情 / 导出 | 45-60 分钟 |
| 前端类型与 client 接入 | 20-30 分钟 |
| Request Journal 页面 | 60-90 分钟 |
| 测试验证 | 30-45 分钟 |
| **合计** | **约 2.5-3.5 小时** |