---
status: deferred
horizon: long_term
workflow_stage: archived
next_command: /route-task-by-status
last_reviewed: 2026-03-22
---

# 任务 003：Provider 状态管理 - 健康追踪

## 目标

实现 Provider 状态管理，包括健康追踪、cooldown 与失败计数。

## BDD 场景

```gherkin
Scenario: Provider health state transitions
  Given a healthy provider
  When mark_failure() is called
  Then failure_count increments by 1
  And health remains healthy until threshold reached

Scenario: Cooldown on quota error
  Given a healthy provider
  When mark_quota_exhausted() is called
  Then state changes to Cooldown
  And cooldown_until is set to current time + cooldown_secs

Scenario: Auto-recovery after cooldown period
  Given a provider in Cooldown state
  When cooldown period expires
  And is_available() is called
  Then returns true (provider is available again)

Scenario: Failure threshold triggers unhealthy state
  Given a provider with failure_threshold = 3
  When mark_failure() is called 3 times
  Then state changes to Unhealthy
  And recovery_at is set for recovery_secs later

Scenario: Shared state across threads
  Given a ProviderManager with multiple providers in DashMap
  When one thread marks a provider as failed
  Then another thread sees the updated failure count immediately
```

## 涉及文件

**创建**：
- `src/provider.rs` - 完整实现

## 历史实施步骤

1. 定义 Provider 状态枚举：
   ```rust
   enum ProviderState {
       Healthy,
       Cooldown { until: DateTime<Utc> },
       Unhealthy { recovery_at: DateTime<Utc> },
   }
   ```

2. 定义 Provider 结构：
   - 配置（不可变）
   - 用于 state、failure_count、last_error 的并发安全状态容器
   - 方法：`mark_quota_exhausted()`、`mark_failure()`、`is_available()`、`reset()`

3. 定义 ProviderManager（包装在 `Arc<DashMap<String, Provider>>` 中）：
   - `new(config: &Config)` - 从配置初始化
   - `get_available()` - 返回 healthy / 非 cooldown 的 provider
   - `get_by_name(name: &str)` - 按名称查询
   - `mark_quota_exhausted(name: &str)` - 标记指定 provider 配额耗尽
   - `mark_failure(name: &str)` - 增加失败计数

4. 实现健康检查逻辑：
   - cooldown 时长来自配置（默认 60 秒）
   - recovery 时长来自配置（默认 600 秒）
   - 每次访问时自动检查并清理已过期 cooldown / unhealthy 状态

## 历史验证方式

运行：

```bash
cargo test --lib provider
```

预期：
- 健康状态流转测试通过
- 并发访问的线程安全测试通过

## 依赖
- 任务 002（用于加载 provider 配置的 Config 模块）
