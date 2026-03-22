---
status: deferred
horizon: long_term
workflow_stage: archived
next_command: /route-task-by-status
last_reviewed: 2026-03-22
---

# 任务 002：配置模块 - TOML 解析与校验

## 目标

实现基于 TOML 的配置解析，并补齐校验与默认值处理。

## BDD 场景

```gherkin
Scenario: Parse valid config with multiple providers
  Given a config.toml file with server, routing, health sections
  And three [[providers]] entries with name, base_url, api_key
  When the config is loaded via Config::load()
  Then parsing succeeds without errors
  And all three providers are available in config.providers
  And default values are applied (request_timeout_secs=300, log_level=info)

Scenario: Validate required fields
  Given a config.toml file missing provider.api_key
  When Config::load() is called
  Then it returns an error
  And the error message indicates missing api_key field

Scenario: Apply default routing strategy
  Given a config.toml with no [routing.strategy] field
  When config is loaded
  Then routing.strategy equals "failover"

Scenario: Parse provider priority and weight
  Given a config.toml with providers having priority=1 and weight=5
  When config is loaded
  Then provider.priority equals 1
  And provider.weight equals 5
```

## 涉及文件

**创建**：
- `src/config.rs` - 完整实现
- `config.toml.example` - 示例配置文件

## 历史实施步骤

1. 定义带 Serde 派生的配置结构：
   - `Config`（根结构）
   - `ServerConfig`
   - `RoutingConfig`
   - `HealthConfig`
   - `ProviderConfig`

2. 在 `Config::load()` 中补齐校验逻辑：
   - 检查必填字段（`name`、`base_url`、`api_key`）
   - 对 `base_url` 做 URL 校验
   - 对可选字段应用默认值

3. 实现 `Config::load(path: &str)`：
   - 读取 TOML 文件
   - 反序列化为 `Config`
   - 校验后返回 `Result<Config, Error>`

4. 创建带注释的示例配置文件

## 历史验证方式

运行：

```bash
cargo build
```

手工测试：

```rust
let config = Config::load("config.toml.example").unwrap();
assert_eq!(config.server.listen, "127.0.0.1:9090");
assert_eq!(config.providers.len(), 3);
```

## 依赖
- 任务 001（项目结构与 serde 依赖）
