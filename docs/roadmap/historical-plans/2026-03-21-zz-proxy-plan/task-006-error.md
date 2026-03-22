---
status: deferred
horizon: long_term
workflow_stage: archived
next_command: /route-task-by-status
last_reviewed: 2026-03-22
---

# 任务 006：错误模块 - 配额检测与错误类型

## 目标

实现错误类型定义，以及从 HTTP 响应中识别配额耗尽的检测逻辑。

## BDD 场景

```gherkin
Scenario: Detect quota exhaustion from HTTP 429
  Given HTTP response status = 429
  When is_quota_error() is called
  Then returns true

Scenario: Detect quota exhaustion from HTTP 403 with quota keywords
  Given HTTP response status = 403
  And response body contains "quota exceeded"
  When is_quota_error() is called
  Then returns true

Scenario: Detect quota from body keywords case-insensitively
  Given HTTP response status = 403
  And response body contains "INSUFFICIENT_QUOTA"
  When is_quota_error() is called
  Then returns true (case-insensitive match)

Scenario: Don't detect quota from HTTP 200
  Given HTTP response status = 200
  When is_quota_error() is called
  Then returns false (never inspect body on success)

Scenario: Detect other failover-eligible errors
  Given HTTP response status = 500
  When is_failover_eligible() is called
  Then returns true (retry on next provider)

Scenario: Non-failover errors
  Given HTTP response status = 400
  When is_failover_eligible() is called
  Then returns false (client error, don't retry)
```

## 涉及文件

**创建**：
- `src/error.rs` - 完整实现

## 历史实施步骤

1. 定义自定义错误类型：
   ```rust
   enum ProxyError {
       ConfigError(String),
       ProviderError(String),
       RequestError(String),
       QuotaExhausted(String),
       AllProvidersFailed(Vec<ProxyError>),
   }
   ```

2. 实现配额检测：
   - `is_quota_error(status, body: &[u8]) -> bool`
   - 检查状态码：429 -> true，403 -> 检查 body
   - 检查关键词：`quota`、`rate limit`、`exceeded`、`insufficient_quota`、`billing`、`limit reached`
   - 只检查 body 前 1KB
   - 大小写不敏感

3. 实现 failover 资格判断：
   - `is_failover_eligible(status) -> bool`
   - 以下情况返回 true：429、403(quota)、5xx、timeout、连接错误
   - 以下情况返回 false：2xx、400、401、404

4. 实现从 hyper/http 错误到代理错误的转换

## 历史验证方式

运行：

```bash
cargo test --lib error
```

预期：
- 所有关键词变体的 quota 检测测试通过
- 状态码分类测试通过
- 大小写不敏感匹配测试通过

## 依赖
- 任务 002（提供错误上下文所需的 Config 类型）
