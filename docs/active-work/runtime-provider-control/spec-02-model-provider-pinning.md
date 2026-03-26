---
status: active
horizon: current
workflow_stage: breakdown
feature: model-provider-pinning
last_reviewed: 2026-03-26
---

# Spec 02: Model 级别 Provider 固定调度（Model Pinning）

## 1. 问题陈述

用户需要在运行时临时将某个模型（如 `claude-sonnet-4-20250514`）固定调度到某个指定 provider，绕过常规路由策略。典型场景：

- 测试某个 provider 对特定模型的兼容性
- 某个 provider 的某模型额度充足，希望临时全量走该 provider
- 调试问题时希望排除路由策略的干扰

当前系统已有 **Model Rules**（`ModelRule`：pattern → target_provider），但这些规则的语义是"路由建议"：当目标 provider 不可用时，`select_with_rules` 返回 `RuleMatchedButUnavailable` 并**停止重试**，而不是 fallback 到其他 provider。

Model Pinning 与 Model Rules 的区别：
- **Model Rules**：配置持久化，优先级高于路由策略，目标不可用时停止重试
- **Model Pinning**：运行时临时状态，重启失效，语义是"强制绑定"

## 2. 目标

- 通过 API 设置：将模型 X 固定到 provider Y（运行时生效，重启失效）
- 通过 API 取消固定：恢复为常规路由策略
- 通过 API 查询当前所有 pinning 状态
- 当 pinned provider 被禁用或不可用时，请求应返回明确错误而非静默 fallback
- UI 提供操作入口（可后续批次实现）

## 3. 当前代码现实

| 组件 | 现状 | 与 pinning 的关系 |
|------|------|-------------------|
| `model_rules: Arc<RwLock<Vec<ModelRule>>>` | 存在于 `AppState` | Pinning 优先级应高于 rules |
| `select_with_rules()` in `proxy.rs` | 匹配 rule → 返回 provider 或 unavailable | Pinning 检查应在此之前 |
| `Router.pinned_provider` | 全局级别 pin（Manual 策略用） | 不同概念，model pinning 是 per-model |
| `/zz/api/routing/rules` PUT | 更新 model rules | Pinning 需要独立 API |

## 4. 设计

### 4.1 数据结构

在 `AppState` 中新增：

```rust
/// 运行时 model pinning 状态（重启失效）
/// key: model name (exact match), value: provider name
pub model_pins: Arc<DashMap<String, String>>,
```

使用 `DashMap` 而非 `RwLock<HashMap>` 以保持与 `ProviderManager.providers` 一致的并发模型。

### 4.2 API 端点

#### `GET /zz/api/routing/pins`

返回当前所有 model pinning。

```json
{
  "pins": [
    { "model": "claude-sonnet-4-20250514", "provider": "anthropic-1" },
    { "model": "gpt-4o", "provider": "openai-2" }
  ]
}
```

#### `PUT /zz/api/routing/pins`

批量设置 pinning（覆盖全部）。

```json
{
  "pins": [
    { "model": "claude-sonnet-4-20250514", "provider": "anthropic-1" }
  ]
}
```

验证：
- `provider` 必须存在于 `ProviderManager` 中（不要求 enabled/healthy）
- `model` 不能为空字符串

响应 `200 OK`：返回设置后的完整 pins 列表。

#### `DELETE /zz/api/routing/pins/{model}`

取消单个模型的 pinning。URL 中 `{model}` 需要 URL encode。

响应 `200 OK`：`{ "removed": "claude-sonnet-4-20250514" }`
响应 `404 Not Found`：`{ "error": "No pin found for model: xxx" }`

### 4.3 代理路由集成

在 `proxy.rs` 的 provider 选择逻辑中，pinning 检查插入到 **model rules 之前**：

```text
优先级：model_pins > model_rules > routing_strategy
```

伪代码：

```rust
// 1. 检查 model pinning（最高优先级）
if let Some(pinned_provider_name) = state.model_pins.get(&model) {
    match state.provider_manager.get_by_name(&pinned_provider_name) {
        Some(provider) if provider.is_available() => {
            // 使用 pinned provider
            use provider;
        }
        Some(_) => {
            // pinned provider 存在但不可用，返回错误，不 fallback
            return 503 "Pinned provider '{name}' for model '{model}' is unavailable";
        }
        None => {
            // pinned provider 已被删除，忽略 pin，走正常路由
            // 同时清理过期 pin
            state.model_pins.remove(&model);
            // fall through to normal routing
        }
    }
}

// 2. 检查 model rules
// 3. 使用 routing strategy
```

关键行为：
- pinned provider 可用 → 直接使用，跳过所有其他路由逻辑
- pinned provider 存在但 disabled/unhealthy → **返回 503 错误**（不静默 fallback，这是"固定"的语义）
- pinned provider 已被删除 → 清理该 pin，退回正常路由

### 4.4 日志增强

在 `LogEntry` 或日志输出中标记该请求是否命中了 pinning：

```rust
tracing::info!(
    model = %model,
    pinned_provider = %provider_name,
    "Model pinned to provider"
);
```

## 5. 不做

- 不持久化 pins 到配置文件（运行时状态，重启失效）
- 不支持 glob/通配符 pattern（使用精确匹配；通配符路由使用现有 model rules）
- 不在本批次实现 UI（API 先行，UI 可后续批次补充）
- 不修改现有 model rules 逻辑

## 6. 验收标准

- [ ] `PUT /zz/api/routing/pins` 设置 pinning 后，对应模型请求固定走指定 provider
- [ ] `DELETE /zz/api/routing/pins/{model}` 取消后恢复正常路由
- [ ] `GET /zz/api/routing/pins` 返回当前所有 pinning
- [ ] pinned provider 被 disable 后，pinned 模型请求返回 503 而非 fallback
- [ ] pinned provider 被删除后，pin 自动清理，恢复正常路由
- [ ] 非 pinned 模型不受影响，走正常 rules + strategy
- [ ] 重启后 pins 清空（纯运行时状态）
- [ ] `cargo test` 通过
- [ ] `cargo clippy` 通过

## 7. 涉及文件

| 文件 | 变更类型 |
|------|----------|
| `src/proxy.rs` | `AppState` 新增 `model_pins`，路由逻辑增加 pin 检查 |
| `src/admin_api.rs` | 新增 pins CRUD 端点 |
| `src/main.rs` | 初始化 `model_pins: Arc::new(DashMap::new())` |

## 8. 预计工时

| 任务 | 估时 |
|------|------|
| 数据结构 + AppState 扩展 | 15 分钟 |
| API 端点（3 个） | 30-45 分钟 |
| proxy.rs 路由集成 | 30-45 分钟 |
| 测试 | 30-45 分钟 |
| **合计** | **约 2-2.5 小时** |

