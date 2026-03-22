---
status: deferred
horizon: long_term
workflow_stage: archived
next_command: /route-task-by-status
last_reviewed: 2026-03-22
---

# 任务 009：管理端点 - Health / Stats / Reload

## 目标

实现用于监控和配置管理的管理 HTTP 端点。

## BDD 场景

```gherkin
Scenario: Health endpoint returns provider states
  Given three providers: ali-account-1 (healthy), zhipu-account-1 (cooldown), ali-account-2 (unhealthy)
  When GET /zz/health is requested
  Then response is JSON with provider states
  And shows ali-account-1 as healthy
  And shows zhipu-account-1 as cooldown
  And shows ali-account-2 as unhealthy

Scenario: Stats endpoint returns request/error counts
  Given proxy has handled 10 requests to ali-account-1
  And ali-account-1 has 2 quota errors
  When GET /zz/stats is requested
  Then response includes request_count=10 for ali-account-1
  And error_count=2 for ali-account-1

Scenario: Reload endpoint hot-reloads config
  Given config.toml is modified on disk
  When POST /zz/reload is requested
  Then config is reloaded without restarting server
  And new providers are available immediately
  And in-flight requests are not dropped

Scenario: Non-admin paths are not intercepted
  Given request path = /v1/chat/completions
  When request reaches admin router
  Then admin router returns None (not handled)
  And request continues to proxy handler

Scenario: Admin paths are intercepted
  Given request path = /zz/health
  When request reaches admin router
  Then admin router returns Some(response)
  And proxy handler is not invoked
```

## 涉及文件

**创建**：
- `src/admin.rs` - 完整实现

## 历史实施步骤

1. 定义管理路由函数：
   - `handle_admin_request(req: &Request<Body>) -> Option<Response<Body>>`
   - 匹配 `/zz/` 前缀
   - 处理时返回 `Some(response)`，否则返回 `None`

2. 实现 `/zz/health`：
   - 仅支持 GET
   - 返回 JSON：`{ "providers": [{ "name": "...", "state": "...", "failure_count": 0 }] }`
   - 包含健康状态、剩余 cooldown 时间、失败计数

3. 实现 `/zz/stats`：
   - 仅支持 GET
   - 返回各 provider 的请求计数与错误计数
   - 包含最后一次错误时间
   - 在 Provider 结构中跟踪这些指标（如 `AtomicU64` 计数器）

4. 实现 `/zz/reload`：
   - 仅支持 POST
   - 从磁盘重新加载配置
   - 用新配置更新 ProviderManager
   - 返回成功 / 失败状态
   - **关键点**：使用 Arc swap 或读写锁保证热重载安全

5. 集成到主请求处理流程：
   - 在代理前先检查是否命中管理路径
   - 如果已处理，直接返回管理响应

## 历史验证方式

运行服务后测试：

```bash
curl http://localhost:9090/zz/health
curl http://localhost:9090/zz/stats
curl -X POST http://localhost:9090/zz/reload
```

预期：
- 所有端点都返回有效 JSON
- Health 能正确展示 provider 状态
- Reload 在不崩溃的前提下成功生效

## 依赖
- 任务 008（用于与主请求处理流程集成）
