---
status: reference
horizon: short_term
workflow_stage: review
next_command: /sync-active-work-with-code
last_reviewed: 2026-03-22
---

# ZZ UI - 实现状态与修正指南

本文用于跟踪当前 UI 原型相对于 `../core-proxy/01-overview.md` 和 `01-ui-spec.md` 的实现状态。

**最近审核时间**：2026-03-21

---

## 实现状态矩阵

### 图例
- ✅ 已完整实现并符合规格
- ⚠️ 部分实现（存在小缺口）
- ❌ 未实现或缺失

---

## 页面 1：Overview

| 功能 | 状态 | 说明 |
|------|------|------|
| 4 张统计卡片 | ✅ | Total Requests、Active/Healthy Providers、Strategy |
| Strategy 快速切换下拉 | ✅ | `<Select>` + 5 种策略 + 切换 toast |
| 请求率折线图（1h） | ✅ | 使用 `generateRequestRateData()` |
| 实时流量分布图 | ✅ | 由 store 中实时 provider stats 驱动 |
| Activity Feed 实时更新 | ✅ | mock WebSocket 推送新日志 |
| Live 指示器 | ✅ | Radio 图标 + 文案 |
| 新日志高亮动画 | ✅ | 新条目 1.5 秒高亮 |
| Failover 事件标识 | ✅ | warning 图标 + `failover` 标签 |

---

## 页面 2：Providers

| 功能 | 状态 | 说明 |
|------|------|------|
| 4 种状态徽标 | ✅ | Healthy/Cooldown/Unhealthy/Disabled |
| 核心统计信息 | ✅ | 请求数、错误率、延迟 |
| 延迟 sparkline | ✅ | Recharts LineChart |
| 拖拽排序 | ✅ | `@dnd-kit` + GripVertical |
| 启用/禁用切换 | ✅ | 含 toast |
| 编辑弹窗 | ✅ | `base_url`、`api_key`、`priority`、`weight`、`models` |
| Test Connection | ✅ | loading spinner + toast |
| API key 遮罩 | ✅ | `sk-****xxxx` |
| cooldown 倒计时 | ✅ | 每秒更新 |
| Add Provider 弹窗 | ✅ | 含基础校验 |

---

## 页面 3：Routing

| 功能 | 状态 | 说明 |
|------|------|------|
| 策略选择卡片 | ✅ | 5 张卡片 |
| 策略切换 toast | ✅ | |
| 动态策略配置区 | ✅ | 随策略切换渲染不同 UI |
| Failover 配置 | ✅ | `max_retries`、`cooldown_secs`、`failure_threshold`、`recovery_secs` |
| Round Robin 配置 | ✅ | provider 开关 |
| Weighted Random 配置 | ✅ | weight slider |
| Quota-Aware 配置 | ✅ | token budget + threshold |
| Manual / Fixed 配置 | ✅ | provider 下拉选择 |
| Priority / Weight 表格 | ✅ | 支持 DnD |
| Model Routing Rules CRUD | ✅ | 增删改查 |

---

## 页面 4：Logs

| 功能 | 状态 | 说明 |
|------|------|------|
| 实时日志流 | ✅ | mock WebSocket |
| 过滤器 | ✅ | status/provider/keyword + 计数 |
| 可展开详情 | ✅ | 展示完整请求详情 |
| Failover chain 可视化 | ✅ | badge + arrow |
| Failover 行高亮 | ✅ | 琥珀色背景 |
| 导出按钮 | ✅ | JSON 下载 |
| Auto-scroll 开关 | ✅ | 绑定 DOM scroll |
| 1000 条缓冲限制 | ✅ | 在 `store.addLog` 中控制 |

---

## 页面 5：Config

| 功能 | 状态 | 说明 |
|------|------|------|
| TOML 编辑器（textarea） | ✅ | 等宽字体、可调整大小、最小高度 500px |
| 语法高亮 | ⚠️ | 当前使用 `<Textarea>` 兜底，CodeMirror 属于后续增强 |
| 实时校验 | ✅ | 基础启发式校验（检查 `[server]` 与 `[[providers]]`） |
| 校验状态徽标 | ✅ | Valid / Invalid / Unsaved |
| Save & Reload | ✅ | 含 toast |
| Reset | ✅ | 含 toast |
| Download | ✅ | 导出 `.toml` 文件 |
| API key 遮罩 | ✅ | eye toggle + `maskApiKeys()` |
| 元数据：Last modified + Last reloaded | ✅ | 编辑与保存后更新 |

---

## Store 与类型

| 项目 | 状态 | 说明 |
|------|------|------|
| `Provider.headers` | ✅ | `headers?: Record<string, string>` |
| `Provider.token_budget` | ✅ | `token_budget?: number` |
| `RoutingConfig.pinned_provider` | ✅ | `pinned_provider?: string` |
| `addProvider` action | ✅ | 同步更新系统统计 |
| `removeProvider` action | ✅ | 通过名称删除并更新统计 |
| `setPinnedProvider` action | ✅ | 设置 `routingConfig.pinned_provider` |
| `updateProviderWeight` action | ✅ | 更新单个 provider weight |
| `SystemStats.per_provider` | N/A | `Provider.stats` 已足够，不强制单独冗余 |

---

## 结构性事项（原型阶段可接受，生产前应处理）

| 项目 | 状态 | 说明 |
|------|------|------|
| 组件文件拆分 | ⚠️ | 当前以页面级文件为主，复杂度继续增长时应拆分 |
| `api/client.ts` REST 包装层 | ❌ | 后端 REST API 真正联通后再补 |
| 独立 hooks（`useProviders.ts` 等） | ❌ | 未来从 store 中抽离 |
| `components.json`（shadcn 配置） | ❌ | 使用 CLI 时再补 |
| 真正的 WebSocket 客户端 | ❌ | 后端 `/zz/ws` 就绪后替换 `useMockWebSocket` |
| CodeMirror 语法高亮 | ⚠️ | Config 页后续可增强 |

---

## 总结

初始审核中识别出的 **P0 / P1 / P2 / P3** 功能缺口，目前都已经被处理。剩余事项主要有：

1. **结构性重构**（组件拆分、专用 hooks）—— 延后到生产阶段
2. **后端真实联通**（REST client、真实 WebSocket）—— 依赖后端实现完成
3. **CodeMirror** 语法高亮 —— 属于可选增强，当前 `<Textarea>` 兜底方案仍可满足规格要求

**TypeScript 编译状态**：✅ 无错误（`npx tsc --noEmit` 可通过）
