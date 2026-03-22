---
status: reference
horizon: medium_term
workflow_stage: archived
next_command: /route-task-by-status
last_reviewed: 2026-03-22
---

# ZZ - 管理 API 与 WebSocket 规格

本文定义 ZZ Rust 后端与 Web 控制台前端之间的完整 API 合约。所有端点都以 `/zz/` 为前缀，以避免与上游 LLM API 路径冲突。

---

## 1. 设计原则

1. **单端口**：管理 API、WebSocket、UI 静态文件与代理共用同一个 `listen` 端口（默认 `9090`）
2. **统一 JSON**：所有 API 请求与响应默认使用 `Content-Type: application/json`
3. **需要 CORS**：开发模式下（Vite `localhost:5173` -> 后端 `localhost:9090`）需要 CORS 头
4. **前缀路由**：`/zz/api/*` -> REST API，`/zz/ws` -> WebSocket，`/zz/ui/*` -> 静态文件，其余路径 -> 代理
5. **REST 无状态**：每次 API 调用都自包含；实时推送只走 WebSocket

---

## 2. CORS 配置

所有 `/zz/` 响应都必须包含：

```text
Access-Control-Allow-Origin: *
Access-Control-Allow-Methods: GET, POST, PUT, DELETE, OPTIONS
Access-Control-Allow-Headers: Content-Type, Authorization
Access-Control-Max-Age: 86400
```

所有发往 `/zz/*` 的 `OPTIONS` 预检请求都必须返回 `204 No Content`，并带上上述 headers。

> 生产环境中，如有需要，可将 `*` 替换为具体 origin。

---

## 3. REST API 端点

### 3.1 Providers

#### `GET /zz/api/providers`

列出所有 provider 的配置、运行时状态与统计信息。

**响应** `200 OK`：
```jsonc
{
  "providers": [
    {
      "name": "ali-account-1",
      "base_url": "https://dashscope.aliyuncs.com/compatible-mode",
      "api_key": "sk-xxxx",
      "priority": 1,
      "weight": 50,
      "enabled": true,
      "models": ["qwen-plus", "qwen-turbo"],
      "headers": { "X-Custom": "val" },
      "token_budget": null,
      "status": "healthy",
      "cooldown_until": null,
      "consecutive_failures": 0,
      "stats": {
        "total_requests": 5432,
        "total_errors": 12,
        "error_rate": 0.22,
        "avg_latency_ms": 1200,
        "latency_history": [1100, 1250, 1180, 1300, 1200, 1150, 1220, 1190, 1280, 1210, 1230, 1200]
      }
    }
  ]
}
```

**实现说明（后端）**：
- `enabled`：后端需要为每个 provider 维护运行时启用开关（与健康状态分离）
- `status`：`ProviderState::Healthy -> "healthy"`，`Cooldown -> "cooldown"`，`Unhealthy -> "unhealthy"`；若 `enabled == false`，无论健康状态如何都返回 `"disabled"`
- `stats.error_rate`：按 `(total_errors / total_requests) * 100` 计算；若 `total_requests == 0` 返回 `0`
- `stats.avg_latency_ms`：可使用 EMA（α=0.1）或滑动窗口
- `stats.latency_history`：保留最近 12 个延迟样本

---

#### `GET /zz/api/providers/{name}`

获取单个 provider 的详细信息。schema 与上面的单个 provider 对象一致。

**响应** `200 OK`：单个 provider 对象
**响应** `404 Not Found`：`{ "error": "Provider not found: {name}" }`

---

#### `POST /zz/api/providers`

运行时新增 provider。

**请求体**：
```jsonc
{
  "name": "ali-account-3",
  "base_url": "https://...",
  "api_key": "sk-newkey",
  "priority": 4,
  "weight": 10,
  "models": [],
  "headers": {},
  "token_budget": null
}
```

**响应** `201 Created`：返回完整 provider 对象
**响应** `400 Bad Request`：
- `{ "error": "Provider name already exists" }`
- `{ "error": "name is required" }`

**副作用**：
- 将 provider 加入 `ProviderManager.providers`
- 追加到内存中的 `provider_configs`
- 通过 WebSocket 广播 `provider_state` 事件
- **不会自动写回 `config.toml`**（运行时有效，直到显式保存配置）

---

#### `PUT /zz/api/providers/{name}`

更新已有 provider 的配置。

**请求体**（所有字段可选，与现有配置合并）：
```jsonc
{
  "base_url": "https://new-url.com",
  "api_key": "sk-newkey",
  "priority": 2,
  "weight": 30,
  "enabled": false,
  "models": ["model-a", "model-b"],
  "headers": { "X-New": "header" },
  "token_budget": 500000
}
```

**响应** `200 OK`：更新后的完整 provider 对象
**响应** `404 Not Found`：`{ "error": "Provider not found: {name}" }`

**副作用**：
- 更新 `ProviderManager.providers` 中的条目
- 如果 `enabled` 变化，则通过 WebSocket 广播 `provider_state`
- **不会自动写回 `config.toml`**

---

#### `DELETE /zz/api/providers/{name}`

运行时删除 provider。

**响应** `200 OK`：`{ "removed": "ali-account-3" }`
**响应** `404 Not Found`：`{ "error": "Provider not found: {name}" }`

**副作用**：
- 从 `ProviderManager.providers` 中移除
- 通过 WebSocket 广播 `provider_state`（`type: "removed"`）
- **不会自动写回 `config.toml`**

---

#### `POST /zz/api/providers/{name}/test`

通过发送轻量请求测试 provider 连通性。

**实现方式**：向 `GET {base_url}/v1/models` 发送请求，并使用 provider 的 API key，记录延迟。

**响应** `200 OK`：
```jsonc
{
  "success": true,
  "latency_ms": 350,
  "status_code": 200
}
```

**响应** `200 OK`（测试失败但接口本身正常）：
```jsonc
{
  "success": false,
  "latency_ms": 5000,
  "status_code": 401,
  "error": "Unauthorized"
}
```

**响应** `404 Not Found`：`{ "error": "Provider not found: {name}" }`

---

#### `POST /zz/api/providers/{name}/enable`

启用一个已禁用的 provider。

**响应** `200 OK`：`{ "name": "...", "enabled": true }`

---

#### `POST /zz/api/providers/{name}/disable`

禁用一个 provider（停止接收流量）。

**响应** `200 OK`：`{ "name": "...", "enabled": false }`

---

#### `POST /zz/api/providers/{name}/reset`

重置 provider 的健康状态（清除 cooldown / unhealthy 与失败计数）。

**响应** `200 OK`：`{ "name": "...", "status": "healthy" }`

---

### 3.2 Routing

#### `GET /zz/api/routing`

获取当前路由配置。

**响应** `200 OK`：
```jsonc
{
  "strategy": "failover",
  "max_retries": 3,
  "cooldown_secs": 60,
  "failure_threshold": 3,
  "recovery_secs": 600,
  "pinned_provider": null,
  "model_rules": [
    { "id": "rule_1", "pattern": "qwen-*", "target_provider": "ali-account-1" },
    { "id": "rule_2", "pattern": "glm-*", "target_provider": "zhipu-account-1" }
  ]
}
```

#### `PUT /zz/api/routing`

更新路由策略与相关参数。

**请求体**：
```jsonc
{
  "strategy": "weighted-random",
  "max_retries": 5,
  "cooldown_secs": 120,
  "failure_threshold": 5,
  "recovery_secs": 300,
  "pinned_provider": "ali-account-1"
}
```

**响应** `200 OK`：完整更新后的路由配置

#### `GET /zz/api/routing/rules`

获取 model 路由规则。

#### `PUT /zz/api/routing/rules`

替换所有 model 路由规则。

**请求体**：
```jsonc
{
  "rules": [
    { "pattern": "qwen-*", "target_provider": "ali-account-1" },
    { "pattern": "glm-*", "target_provider": "zhipu-account-1" }
  ]
}
```

---

### 3.3 Stats

#### `GET /zz/api/stats`

获取聚合系统统计。

**响应** `200 OK`：
```jsonc
{
  "total_requests": 12847,
  "requests_per_minute": 23.5,
  "active_providers": 3,
  "healthy_providers": 4,
  "total_providers": 5,
  "strategy": "failover",
  "uptime_secs": 86400
}
```

#### `GET /zz/api/stats/timeseries?period=1h`

获取用于图表展示的时序请求率数据。

**查询参数**：
- `period`：`1h`（默认）、`6h`、`24h`

**响应** `200 OK`：
```jsonc
{
  "period": "1h",
  "interval_secs": 60,
  "data": [
    { "time": "2026-03-21T12:05:00Z", "value": 23 },
    { "time": "2026-03-21T12:06:00Z", "value": 25 }
  ]
}
```

---

### 3.4 Logs

#### `GET /zz/api/logs?limit=100&offset=0`

分页获取请求日志（按时间倒序）。

**查询参数**：
- `limit`：默认 100，最大 1000
- `offset`：默认 0
- `status`：按 `2xx`、`4xx`、`5xx`、`error` 过滤（可选）
- `provider`：按 provider 名称过滤（可选）
- `search`：按 path / model / provider / id 关键词搜索（可选）

**响应** `200 OK`：
```jsonc
{
  "logs": [
    {
      "id": "req_abc123",
      "timestamp": "2026-03-21T13:05:02Z",
      "method": "POST",
      "path": "/v1/chat/completions",
      "provider": "ali-account-1",
      "status": 200,
      "duration_ms": 2300,
      "ttfb_ms": 800,
      "model": "qwen-plus",
      "streaming": true,
      "request_bytes": 1200,
      "response_bytes": 3400,
      "failover_chain": null
    }
  ],
  "total": 12847,
  "offset": 0,
  "limit": 100
}
```

---

### 3.5 Config

#### `GET /zz/api/config`

获取当前配置文件内容（原始 TOML 字符串）。

#### `PUT /zz/api/config`

校验、写入磁盘并热重载配置。

**请求体**：
```jsonc
{
  "content": "[server]
listen = "127.0.0.1:9090"
..."
}
```

**成功响应**：
```jsonc
{
  "saved": true,
  "reloaded": true,
  "last_modified": "2026-03-21T14:00:00Z",
  "last_reloaded": "2026-03-21T14:00:01Z"
}
```

**失败响应**：
```jsonc
{
  "saved": false,
  "error": "TOML parse error: expected `=`, found newline at line 5"
}
```

#### `POST /zz/api/config/validate`

只校验 TOML，不写盘。

---

### 3.6 System

#### `GET /zz/api/health`

返回系统健康信息。

#### `GET /zz/api/version`

返回版本信息。

---

## 4. WebSocket 协议

### 端点

`ws://127.0.0.1:9090/zz/ws`

### 服务端 -> 客户端消息

所有消息都使用 JSON，并通过 `type` 字段区分类型。

#### `log`
每个代理请求完成后发送。

#### `provider_state`
当 provider 健康状态发生变化时发送。

#### `stats`
每 **5 秒** 向所有连接客户端推送一次统计快照。

### 客户端 -> 服务端消息

#### `subscribe`
可选地订阅特定事件类型。

```jsonc
{
  "type": "subscribe",
  "events": ["log", "stats"]
}
```

有效事件类型：`"log"`、`"provider_state"`、`"stats"`

### 连接管理

- 前端应在连接关闭后做自动重连（指数退避：1s、2s、4s、8s，最大 30s）
- 后端每 30 秒发送一次 ping，客户端返回 pong
- 不设置硬编码客户端上限，使用 `tokio::sync::broadcast`，丢弃过慢消费者

---

## 5. 静态文件服务

### 开发模式

Vite 开发服务运行在 `localhost:5173`，并将 API 请求代理到后端。

### 生产模式

后端将 `ui/dist/**` 静态文件挂载到 `/zz/ui/*`：
- `GET /zz/ui/` -> `index.html`
- `GET /zz/ui/assets/*` -> 构建后的 JS/CSS
- 可使用 `rust-embed` 或 `include_dir` 在编译时内嵌文件

---

## 6. 错误响应格式

所有 API 错误统一采用：

```jsonc
{
  "error": "Human-readable error message"
}
```

HTTP 状态码：
- `200` 成功
- `201` 已创建
- `204` 无内容
- `400` 请求错误
- `404` 资源不存在
- `500` 服务端内部错误

---

## 7. 请求 ID 生成

每个代理请求都会生成唯一 ID：`req_{nanoid(12)}`。

---

## 8. 后端数据采集要求

为支持 REST API 与 WebSocket，后端至少需要采集：

| 数据 | 存储 | 保留策略 |
|------|------|----------|
| 每请求日志 | `VecDeque<LogEntry>` | 最近 10,000 条 |
| 每 provider 延迟样本 | `VecDeque<u64>` | 最近 12 条 |
| 每 provider 请求 / 错误计数 | `AtomicU64` | 进程生命周期 |
| 每分钟请求率 | Ring buffer `[u32; 1440]` | 最近 24 小时 |
| 服务启动时间 | `Instant` | 进程生命周期 |
| provider enabled 标志 | `AtomicBool` | 进程生命周期 |
| model 路由规则 | `RwLock<Vec<ModelRule>>` | 进程生命周期 |

---

## 9. 与现有旧端点的兼容性

现有管理端点（`/zz/health`、`/zz/stats`、`/zz/reload`）应继续保留，用于 CLI / curl 和向后兼容。新的 `/zz/api/*` 是 UI 的主要接口。

| 旧端点 | 新等价端点 |
|--------|------------|
| `GET /zz/health` | `GET /zz/api/health` |
| `GET /zz/stats` | `GET /zz/api/stats` |
| `POST /zz/reload` | `PUT /zz/api/config` |
