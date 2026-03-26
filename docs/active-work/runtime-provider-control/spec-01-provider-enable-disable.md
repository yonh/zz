---
status: active
horizon: current
workflow_stage: breakdown
feature: runtime-provider-enable-disable
last_reviewed: 2026-03-26
---

# Spec 01: Provider 运行时启停

## 1. 问题陈述

当前 ZZ 后端已有 `Provider.enabled: AtomicBool` 和 `/zz/api/providers/{name}/enable`、`/disable` API 端点，UI Providers 页面也有对应的 Enable/Disable 按钮并正确调用了 API。但存在以下缺口导致该功能未完全生效或体验不完整：

### 缺口 A：Routing 页面 toggle 仅更新本地状态

`ui/src/pages/Routing.tsx` 中的 provider toggle 调用的是 `store.toggleProvider(name)`，该方法仅更新 Zustand 本地状态，**不会**调用后端 API。因此在 Routing 页面切换 provider 开关后，后端不知道状态变化，代理行为不变。

**对比**：`ui/src/pages/Providers.tsx` 中的 `handleToggle` 是正确的 —— 先调 API 再更新本地状态。

### 缺口 B：配置文件无 `enabled` 字段

`ProviderConfig` 没有 `enabled` 字段。所有 provider 启动时一律 `enabled=true`。用户无法在配置文件中预设某个 provider 为禁用状态。

### 缺口 C：配置热重载可能覆盖运行时状态

`ProviderManager::reload()` 对已有 provider 仅更新 config 字段、不触碰 `enabled` AtomicBool（这是正确的）。但对新增 provider 总是 `enabled=true`，且没有读取 config 中的 enabled 字段。当 config 支持 enabled 后，reload 需要尊重该字段在 **新增** provider 上的值。

## 2. 目标

- 运行时通过 API / UI 禁用 provider 后，代理请求**立即**不再调度该 provider
- 运行时启用 provider 后，代理**立即**恢复调度该 provider
- 配置文件支持 `enabled` 字段（可选，默认 `true`），启动时尊重
- UI 所有 provider toggle 交互统一调用后端 API
- 重启后以配置文件为准（不持久化运行时状态）

## 3. 当前代码现实（已有可复用部分）

| 组件 | 现状 | 可复用 |
|------|------|--------|
| `Provider.enabled: AtomicBool` | 存在，`new()` 中硬编码 `true` | ✓ 改为从 config 读取 |
| `Provider.is_available()` | 已检查 `is_enabled()` | ✓ 无需改动 |
| `ProviderManager.get_available()` | 已过滤 `is_available()` | ✓ 无需改动 |
| `ProviderManager.get_available_for_model()` | 已过滤 `is_available()` | ✓ 无需改动 |
| `handle_enable_provider` / `handle_disable_provider` | 存在且正确 | ✓ 无需改动 |
| `Providers.tsx handleToggle` | 正确调用 API + 更新本地状态 | ✓ 无需改动 |
| `Routing.tsx toggle` | 仅更新本地状态，不调 API | ✗ 需修复 |
| `ProviderConfig` | 无 `enabled` 字段 | ✗ 需扩展 |

## 4. 必要变更

### 4.1 后端：`src/config.rs`

在 `ProviderConfig` 中新增：

```rust
#[serde(default = "default_enabled")]
pub enabled: bool,
```

其中 `default_enabled` 返回 `true`。

### 4.2 后端：`src/provider.rs`

#### `Provider::new()`
将 `enabled: AtomicBool::new(true)` 改为 `enabled: AtomicBool::new(config.enabled)`。

#### `ProviderManager::reload()`
对**新增** provider，从 config.enabled 初始化。对**已有** provider，不覆盖运行时 enabled 状态（当前行为已正确）。

### 4.3 前端：`ui/src/pages/Routing.tsx`

将 Routing 页面的 provider toggle 改为调用 API：

```typescript
// 修改前：
onClick={() => toggleProvider(p.name)}

// 修改后：类似 Providers.tsx 的 handleToggle
// 先调 api.enableProvider / api.disableProvider，成功后再 toggleProvider
```

### 4.4 前端：`ui/src/stores/store.ts`（可选优化）

考虑让 `toggleProvider` action 本身变成 async 并包含 API 调用，避免各页面重复实现。但这属于优化，不是本次必须。

## 5. 不做

- 不持久化运行时 enabled 状态到配置文件
- 不修改 `is_available()` 逻辑（已正确）
- 不修改代理路由核心逻辑（`get_available()` 已正确过滤）

## 6. 验收标准

- [ ] `config.toml` 中 provider 设置 `enabled = false` 后启动，该 provider 不参与调度
- [ ] 启动后所有 provider 默认 `enabled = true`（config 不写 enabled 时）
- [ ] 通过 `POST /zz/api/providers/{name}/disable` 禁用 provider 后，后续请求不调度该 provider
- [ ] 通过 `POST /zz/api/providers/{name}/enable` 启用后，恢复调度
- [ ] Routing 页面的 toggle 按钮调用后端 API，且代理行为随之变化
- [ ] Providers 页面 toggle 行为不受影响（已正确）
- [ ] `cargo test` 通过
- [ ] `cargo clippy` 通过

## 7. 涉及文件

| 文件 | 变更类型 |
|------|----------|
| `src/config.rs` | `ProviderConfig` 增加 `enabled` 字段 |
| `src/provider.rs` | `Provider::new()` 读取 config.enabled |
| `ui/src/pages/Routing.tsx` | toggle 调用后端 API |
| `ui/src/stores/store.ts` | （可选）toggleProvider 改为 async with API |

## 8. 预计工时

| 任务 | 估时 |
|------|------|
| config.rs + provider.rs 变更 | 15-30 分钟 |
| Routing.tsx 修复 | 15-30 分钟 |
| 测试验证 | 15-30 分钟 |
| **合计** | **约 1-1.5 小时** |

