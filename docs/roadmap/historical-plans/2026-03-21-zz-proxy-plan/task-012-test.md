---
status: deferred
horizon: long_term
workflow_stage: archived
next_command: /route-task-by-status
last_reviewed: 2026-03-22
---

# 任务 012：集成测试 - 手工验证

## 目标

通过手工测试验证所有核心能力的端到端行为。

## BDD 场景

```gherkin
Scenario: Proxy forwards request to upstream provider
  Given zz server is running with config pointing to Ali provider
  And Ali provider is healthy
  When curl sends request to http://localhost:9090/v1/chat/completions
  Then receives valid LLM response from Ali
  And response status is 200

Scenario: SSE streaming works with zero latency
  Given zz server is running
  When curl sends request with stream=true
  Then receives SSE events in real-time
  And no buffering delay is observed

Scenario: Quota error triggers failover
  Given zz has two providers: ali-account-1, zhipu-account-1
  And ali-account-1 is configured with invalid API key (will return 403)
  When first request is sent
  Then ali-account-1 returns 403
  And zz marks it as cooldown
  And retries with zhipu-account-1
  And client receives response from zhipu-account-1

Scenario: Health endpoint shows provider states
  Given zz server is running
  When GET /zz/health is requested
  Then returns JSON with all provider states
  And shows which providers are healthy/cooldown/unhealthy

Scenario: Config hot-reload works
  Given zz server is running
  And config.toml is modified (add new provider)
  When POST /zz/reload is sent
  Then new provider is available immediately
  And subsequent requests can be routed to it

Scenario: All providers exhausted returns error
  Given all providers have invalid API keys
  When request is sent
  Then zz tries all providers
  And returns last error to client (403 or 401)
```

## 测试准备

1. 创建测试配置：
   ```bash
   cp config.toml.example config.toml
   # 替换成真实测试 API key
   ```

2. 启动服务：
   ```bash
   cargo run
   ```

## 手工测试命令

```bash
# 测试 1：基础代理转发
curl http://localhost:9090/v1/chat/completions   -H "Content-Type: application/json"   -d '{
    "model": "qwen-plus",
    "messages": [{"role": "user", "content": "hi"}]
  }'

# 测试 2：SSE streaming
curl -N http://localhost:9090/v1/chat/completions   -H "Accept: text/event-stream"   -H "Content-Type: application/json"   -d '{
    "model": "qwen-plus",
    "messages": [{"role": "user", "content": "tell me a story"}],
    "stream": true
  }'

# 测试 3：Health 端点
curl http://localhost:9090/zz/health | jq .

# 测试 4：Stats 端点
curl http://localhost:9090/zz/stats | jq .

# 测试 5：热重载
curl -X POST http://localhost:9090/zz/reload

# 测试 6：触发 failover（将第一个 provider 配成错误 key）
# 然后观察日志确认是否切换到第二个 provider
```

## 验证标准

以下项必须全部通过：
- ✅ 基础请求转发可用
- ✅ SSE streaming 没有明显额外延迟
- ✅ Health 端点返回有效 JSON
- ✅ Quota 错误会触发 provider cooldown
- ✅ Config reload 无需重启即可生效
- ✅ 代理额外开销足够低（本地 provider 场景可测）

## 依赖
- 任务 011（主服务已可运行并接收请求）
