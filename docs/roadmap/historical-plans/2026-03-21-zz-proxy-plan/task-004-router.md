---
status: deferred
horizon: long_term
workflow_stage: archived
next_command: /route-task-by-status
last_reviewed: 2026-03-22
---

# 任务 004：路由模块 - Failover 策略

## 目标

实现多种路由策略，并以 failover 作为默认策略，支持按 provider 健康状态与优先级进行选择。

## BDD 场景

```gherkin
Scenario: Failover selects highest priority healthy provider
  Given three providers: ali-account-1 (priority=1), zhipu-account-1 (priority=2), ali-account-2 (priority=3)
  And all providers are healthy
  When select_provider() is called with strategy=failover
  Then returns ali-account-1

Scenario: Failover skips cooldown providers
  Given ali-account-1 (priority=1, state=Cooldown)
  And zhipu-account-1 (priority=2, state=Healthy)
  When select_provider() is called
  Then returns zhipu-account-1 (skips ali-account-1)

Scenario: Failover returns None when all providers unavailable
  Given all providers are in Cooldown or Unhealthy state
  When select_provider() is called
  Then returns None

Scenario: Round-robin distributes evenly
  Given three healthy providers
  When select_provider() is called 3 times with strategy=round-robin
  Then each provider is selected once (no repeats)

Scenario: Weighted-random respects weights
  Given provider A (weight=5), provider B (weight=1)
  When select_provider() is called many times
  Then provider A is selected approximately 5x more often than B
```

## 涉及文件

**创建**：
- `src/router.rs` - 完整实现

## 历史实施步骤

1. 定义 `RoutingStrategy` 枚举：
   ```rust
   enum RoutingStrategy {
       Failover,
       RoundRobin,
       WeightedRandom,
   }
   ```

2. 定义 `Router` 结构：
   - 持有对 ProviderManager 的引用（`Arc`）
   - 当前策略
   - round-robin 索引（使用 `AtomicUsize` 保证线程安全）

3. 实现 `select_provider(&self) -> Option<Provider>`：
   - **Failover**：按 priority 排序，过滤 healthy/available，返回第一个
   - **RoundRobin**：在 healthy providers 间轮转
   - **WeightedRandom**：按 weight 分布选择

4. 实现 `try_next_provider(exclude: &str) -> Option<Provider>`：
   - 用于 retry 逻辑，排除已经失败的 provider
   - 逻辑与 `select_provider` 类似，但会过滤掉指定名称

## 历史验证方式

运行：

```bash
cargo test --lib router
```

预期：
- 基于优先级的选择测试通过
- cooldown 跳过测试通过
- 各策略测试通过

## 依赖
- 任务 003（带健康状态管理的 Provider 模块）
