---
status: reference
horizon: medium_term
workflow_stage: archived
next_command: /route-task-by-status
last_reviewed: 2026-03-22
---

# ZZ - 带自动 Failover 的 LLM API 反向代理总览

## 概览

ZZ 是一个使用 Rust 编写的轻量、高性能反向代理，位于编码工具（Claude Code、Cursor 等）与多个上游 LLM API Provider 之间。它对外暴露一个统一入口，并在某个 provider 的额度用尽时，自动在多个 provider 账号之间轮换或 failover。

## 问题背景

像 Claude Code 这样的编码工具通常会受到单套餐额度限制。对于同时持有多个 provider 账号（如阿里 DashScope、智谱 GLM、OpenAI 等）的用户，需要一个能够透明聚合这些账号，并在某个账号耗尽后自动切换的代理层。

## 架构图

```text
┌─────────────┐     ┌──────────────────┐     ┌──────────────────┐
│ Coding Tool │────▶│   ZZ Proxy       │────▶│ Provider A (Ali) │
│ (ClaudeCode)│     │                  │     └──────────────────┘
│             │◀────│  - URL 重写      │     ┌──────────────────┐
└─────────────┘     │  - Header 重写   │────▶│ Provider B (Zhipu)│
                    │  - Failover 逻辑 │     └──────────────────┘
                    │  - 配额跟踪      │     ┌──────────────────┐
                    └──────────────────┘────▶│ Provider C (...)  │
                                             └──────────────────┘
```

## 核心设计原则

1. **Body-transparent**：请求体和响应体按透明方式透传，不做业务级修改（包括 SSE）
2. **Header-aware**：按上游 provider 重写 `Authorization` 与 `Host`
3. **URL-rewriting**：将本地请求路径映射为上游 `base_url + path`
4. **Failover-driven**：识别额度耗尽与失败信号，并自动切换到下一个 provider
5. **Zero-downtime rotation**：对符合 failover 条件的错误进行无感切换，尽量不影响当前请求

## 配置

配置文件：`config.toml`

```toml
[server]
listen = "127.0.0.1:9090"          # 本地监听地址
# request_timeout_secs = 300       # 单请求超时（默认 300 秒，适合长 LLM 调用）
# log_level = "info"               # trace | debug | info | warn | error

[routing]
strategy = "failover"              # failover | round-robin | weighted-random | quota-aware | manual
# retry_on_failure = true          # 当前 provider 失败时是否在下一个 provider 上重试当前请求
# max_retries = 3                  # 每个请求最多重试次数
# pinned_provider = ""             # （仅 manual 策略）固定到某个 provider

[health]
# failure_threshold = 3            # 连续失败多少次后标记 unhealthy
# recovery_secs = 600              # 多久后重新探测 unhealthy provider
# cooldown_secs = 60               # quota 错误后冷却多久再尝试同一 provider

[[providers]]
name = "ali-account-1"
base_url = "https://dashscope.aliyuncs.com/compatible-mode"
api_key = "sk-xxxx"
# priority = 1
# weight = 5
# models = ["qwen-plus", "qwen-turbo"]
# token_budget = 1000000
# headers = { "X-Custom" = "val" }

[[providers]]
name = "zhipu-account-1"
base_url = "https://open.bigmodel.cn/api/paas/v4"
api_key = "sk-yyyy"
# priority = 2
# models = ["glm-4", "glm-4-flash"]

[[providers]]
name = "ali-account-2"
base_url = "https://dashscope.aliyuncs.com/compatible-mode"
api_key = "sk-zzzz"
# priority = 3
```

## 路由策略

### 1. Failover（默认）
- 按 `priority` 从小到大尝试 provider
- 遇到 quota / rate-limit 错误时，将 provider 标记为 cooldown，并尝试下一个
- 遇到其他错误（5xx、timeout）时，在下一个 provider 上重试
- healthy provider 永远优先于 cooldown provider

### 2. Round-Robin
- 在所有 healthy provider 之间均匀分发请求
- 跳过 unhealthy / cooldown provider

### 3. Weighted-Random
- 按 `weight` 权重随机选择
- 跳过 unhealthy / cooldown provider

### 4. Quota-Aware
- 按 provider 维度跟踪 token 使用量（从响应 `usage` 字段解析）
- 当使用量超过预设阈值时主动切换到下一个 provider
- 需要为 provider 配置 `token_budget`

### 5. Manual / Fixed
- 将所有流量固定到单一 provider（由 `pinned_provider` 指定）
- 不做自动 failover；若该 provider 不可用则直接报错

## Failover 检测

代理通过以下信号识别额度耗尽或需要切换的异常：

| 信号 | 行为 |
|------|------|
| HTTP 429 | 将 provider 置为 cooldown，重试下一个 |
| HTTP 403 且错误体包含额度相关关键词 | 将 provider 置为 cooldown，重试下一个 |
| HTTP 5xx | 记录失败，重试下一个 |
| 连接超时 / 拒绝连接 | 记录失败，重试下一个 |
| HTTP 2xx | 清空失败计数，正常透传 |

**Quota 关键词**（在错误响应前 1KB 内大小写不敏感匹配）：
- `quota`
- `rate limit`
- `exceeded`
- `insufficient_quota`
- `billing`
- `limit reached`

> 注意：仅对**错误响应**（非 2xx）做错误体检查，不会在成功响应上做 body inspection，从而保持正常流量的透明性。

## 请求流转

```text
1. 客户端向代理发送请求（例如 POST /v1/chat/completions）
2. 代理根据路由策略选择 provider
3. 代理重写：
   - URL: {provider.base_url} + 原请求 path
   - Header: Authorization -> Bearer {provider.api_key}
   - Header: Host -> {provider.host}
4. 按透明方式转发请求体
5. 读取上游响应：
   - 如果是 2xx -> 原样流式返回给客户端
   - 如果是可 failover 错误 -> 换下一个 provider 重试（不超过 max_retries）
   - 如果所有 provider 都不可用 -> 返回最后一个错误
6. 更新 provider 健康状态
```

## Streaming（SSE）支持

这对 LLM API 非常关键。代理必须做到：

- 识别 `Accept: text/event-stream` 或请求中的 `stream: true`
- 对上下游都使用 chunked transfer
- 实时将上游 SSE chunk 转发给客户端，不做额外缓冲
- **不对流中途失败做 retry**（只允许在上游正式响应前 failover）

## API 端点

代理会暴露前缀为 `/zz/` 的管理端点，以避免与上游 API 路径冲突。

### 旧版管理端点（CLI / curl）

| 端点 | 说明 |
|------|------|
| `/*`（所有路径） | 透明代理到上游 |
| `GET /zz/health` | 代理健康检查（返回 provider 状态） |
| `GET /zz/stats` | 各 provider 请求与错误计数 |
| `POST /zz/reload` | 无需重启即可热重载配置 |

### 管理 REST API（Web 控制台）

| 端点 | 说明 |
|------|------|
| `GET /zz/api/providers` | 列出所有 provider 的配置、状态与统计 |
| `POST /zz/api/providers` | 运行时新增 provider |
| `GET /zz/api/providers/{name}` | 获取单个 provider 详情 |
| `PUT /zz/api/providers/{name}` | 更新 provider 配置 |
| `DELETE /zz/api/providers/{name}` | 删除 provider |
| `POST /zz/api/providers/{name}/test` | 测试 provider 连通性 |
| `POST /zz/api/providers/{name}/enable` | 启用 provider |
| `POST /zz/api/providers/{name}/disable` | 禁用 provider |
| `POST /zz/api/providers/{name}/reset` | 重置健康 / cooldown 状态 |
| `GET /zz/api/routing` | 获取当前路由配置与 model 规则 |
| `PUT /zz/api/routing` | 更新路由策略与参数 |
| `GET /zz/api/routing/rules` | 获取 model 路由规则 |
| `PUT /zz/api/routing/rules` | 替换 model 路由规则 |
| `GET /zz/api/stats` | 获取聚合系统统计 |
| `GET /zz/api/stats/timeseries` | 获取图表所需时序数据 |
| `GET /zz/api/logs` | 获取结构化请求日志 |
| `GET /zz/api/config` | 获取配置文件内容与元数据 |
| `PUT /zz/api/config` | 校验、保存并热重载配置 |
| `POST /zz/api/config/validate` | 仅校验配置，不保存 |
| `GET /zz/api/health` | 代理健康检查 |
| `GET /zz/api/version` | 版本信息 |

### WebSocket

| 端点 | 说明 |
|------|------|
| `WS /zz/ws` | 实时推送日志、provider 状态变化、统计快照 |

### 静态文件（生产环境）

| 端点 | 说明 |
|------|------|
| `GET /zz/ui/*` | 内嵌 Web 控制台静态文件 |

> 详细请求/响应 schema 与 WebSocket 协议，请参见 `../admin-api/01-api-spec.md`。

## 模块结构

```text
src/
├── main.rs              # 入口、CLI 参数、服务启动、后台任务
├── config.rs            # TOML 配置解析与校验
├── proxy.rs             # 核心代理逻辑（请求/响应转发、日志采集）
├── router.rs            # provider 选择逻辑（failover/round-robin/weighted/quota-aware/manual）
├── provider.rs          # provider 状态管理（健康、cooldown、计数、延迟）
├── rewriter.rs          # URL 与 header 重写
├── stream.rs            # SSE / chunked streaming 工具
├── admin.rs             # 旧版 /zz/* 管理端点
├── admin_api.rs         # /zz/api/* REST 端点
├── ws.rs                # WebSocket handler 与广播通道
├── cors.rs              # /zz/* CORS 中间件
├── stats.rs             # RPM 统计与时序聚合
├── error.rs             # 错误类型
└── logging.rs           # 结构化日志与 RequestLogBuffer
```

## 依赖

| Crate | 用途 |
|-------|------|
| `tokio` | 异步运行时 |
| `hyper` + `hyper-util` | HTTP 服务端与客户端 |
| `http-body-util` | Body 流处理工具 |
| `toml` + `serde` | 配置解析 |
| `tracing` + `tracing-subscriber` | 结构化日志 |
| `dashmap` 或 `arc-swap` | provider 共享状态 |
| `clap` | CLI 参数解析 |

## V1 非目标

- **不修改请求体**：请求/响应 body 透明透传，`model` 字段最多用于只读日志解析
- **代理本身不做认证**：默认仅监听本地 `127.0.0.1`
- **不做 TLS termination**：上游使用 HTTPS，但代理本地监听 HTTP
- **不做缓存**：每个请求都直接转发
- **不做 model 映射**：客户端发来的 model 名称必须由上游原生支持

## 后续考虑（V2+）

- **Token budget tracking**：根据 `usage` 主动在逼近额度前切换 provider
- **Model aliasing**：跨 provider 的 model 名称映射
- **Config encryption**：配置文件中的 API key 加密存储
- **Multi-protocol**：支持原生 Anthropic API 等非 OpenAI 格式
- **Request/response body logging**：可选的全量 body 调试记录

## 成功标准

1. 通过代理访问 Ali / Zhipu 可以返回正确 LLM 响应
2. SSE streaming 具有可接受的实时性，不引入明显延迟
3. 当 provider A 返回 429 时，请求会自动切到 provider B
4. provider A 在 cooldown 后可自动恢复
5. 支持热重载配置且不影响进行中的请求
6. 单请求代理额外开销低（不含网络）
