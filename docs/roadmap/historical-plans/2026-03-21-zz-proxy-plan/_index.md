---
status: deferred
horizon: long_term
workflow_stage: archived
next_command: /route-task-by-status
last_reviewed: 2026-03-22
---

# ZZ LLM API 反向代理 - 历史实施计划索引

## 目的

这是 2026-03-21 的一批历史实施计划文档，用于记录早期拆分方式与任务依赖关系。它属于**历史规划材料**，不应直接作为当前 backlog 执行依据。

## 历史目标

构建一个轻量、高性能的 Rust 反向代理，位于编码工具（Claude Code、Cursor）与多个上游 LLM API Provider 之间，并在额度耗尽时自动 failover。

## 历史架构概览
- **Body-transparent proxy**：请求/响应透明透传（包括 SSE）
- **Header-aware**：按 provider 重写 Authorization 和 Host
- **URL-rewriting**：将本地路径映射到上游 `base_url + path`
- **Failover-driven**：检测额度耗尽并自动切换 provider
- **Zero-downtime**：对可 failover 错误做无感重试

## 历史约束
1. **V1 非目标**：不解析 body、不对代理做认证、不做 TLS termination、不做缓存
2. **性能目标**：单请求代理额外开销低
3. **透明性原则**：除错误响应检测外，不检查或修改正常请求/响应内容
4. **Streaming 优先**：SSE 必须以零缓冲方式工作

## 历史执行计划

```yaml
tasks:
  - id: "001"
    subject: "项目初始化与依赖结构"
    slug: "setup-project"
    type: "setup"
    depends-on: []

  - id: "002"
    subject: "配置模块 - TOML 解析与校验"
    slug: "config-module"
    type: "impl"
    depends-on: ["001"]

  - id: "003"
    subject: "Provider 状态管理 - 健康追踪"
    slug: "provider-state"
    type: "impl"
    depends-on: ["002"]

  - id: "004"
    subject: "路由模块 - Failover 策略"
    slug: "router-failover"
    type: "impl"
    depends-on: ["003"]

  - id: "005"
    subject: "重写模块 - URL 与 Header 重写"
    slug: "rewriter-module"
    type: "impl"
    depends-on: ["004"]

  - id: "006"
    subject: "错误模块 - 配额检测与错误类型"
    slug: "error-module"
    type: "impl"
    depends-on: ["002"]

  - id: "007"
    subject: "流模块 - SSE 支持"
    slug: "stream-module"
    type: "impl"
    depends-on: ["006"]

  - id: "008"
    subject: "代理模块 - 请求/响应转发"
    slug: "proxy-core"
    type: "impl"
    depends-on: ["005", "007"]

  - id: "009"
    subject: "管理端点 - health/stats/reload"
    slug: "admin-endpoints"
    type: "impl"
    depends-on: ["008"]

  - id: "010"
    subject: "日志模块 - 结构化日志"
    slug: "logging-module"
    type: "impl"
    depends-on: ["001"]

  - id: "011"
    subject: "主入口 - 服务启动"
    slug: "main-entry"
    type: "impl"
    depends-on: ["008", "009", "010"]

  - id: "012"
    subject: "集成测试 - 手工验证"
    slug: "integration-test"
    type: "test"
    depends-on: ["011"]
```

## 历史任务文件

- [任务 001：项目初始化与依赖结构](./task-001-setup.md)
- [任务 002：配置模块](./task-002-config.md)
- [任务 003：Provider 状态管理](./task-003-provider.md)
- [任务 004：路由模块](./task-004-router.md)
- [任务 005：重写模块](./task-005-rewriter.md)
- [任务 006：错误模块](./task-006-error.md)
- [任务 007：流模块](./task-007-stream.md)
- [任务 008：代理模块](./task-008-proxy.md)
- [任务 009：管理端点](./task-009-admin.md)
- [任务 010：日志模块](./task-010-logging.md)
- [任务 011：主入口](./task-011-main.md)
- [任务 012：集成测试](./task-012-test.md)

## BDD 覆盖说明

这批历史计划覆盖的核心场景包括：
- 配置解析（TOML 校验、默认值、多 provider）
- Provider 健康追踪（cooldown、失败计数）
- Failover 路由（按优先级选择）
- URL / Header 重写（`base_url + path`、Authorization、Host）
- 配额检测（429、403 + quota 关键词）
- SSE streaming（chunked transfer、零缓冲）
- 管理端点（`/zz/health`、`/zz/stats`、`/zz/reload`）
- 透明代理（body 透传）

## 依赖链

```text
001 (setup)
 ├─ 002 (config) ──┐
 │   ├─ 003 (provider) ── 004 (router) ── 005 (rewriter) ──┐
 │   └───────────────────────────────────────────────────────┘
 └─ 010 (logging)                                               │
                                                                 │
002 (config) ── 006 (error) ── 007 (stream) ────────────────────┤
                                                                 │
                                                                 └─ 008 (proxy) ── 009 (admin) ──┐
                                                                                                  │
                                                                                                  └─ 011 (main) ── 012 (test)
```
