# Phase P7 — 配置扩展 (api_type / fallback / log level)

**Depends on:** P1（与 P4/P6 并行可行）
**Type:** config
**Goal:** 在 `config.rs` 与 `config.toml.example` 中暴露转换相关配置，全部带默认值保证旧配置零变更加载。

---

## Scope

### 1. `ProviderConfig` 新字段

```rust
#[serde(default)]
pub api_type: String, // "anthropic" | "openai-chat" | "openai-responses" | "auto" | ""
#[serde(default = "default_true")]
pub enable_conversion_fallback: bool,
```

- 空字符串 / `auto` 首版按 OpenAIChat 处理（写入文档警告）。
- 提供 `ProviderConfig::resolved_api_type() -> ApiType`。

### 2. 全局 / section 配置

```rust
#[serde(default)]
pub conversion_log_level: String, // "debug" | "info" | "warn"
```

放置位置：与现有日志配置同 section（如 `[logging]` 或 `[server]`），保持就近原则。读取后注入 `tracing` filter（仅影响 `zz::conversion` target）。

### 3. Provider 选择函数

`provider::select_for_target(state, target: ApiType, model: &str) -> Option<&Provider>`：
- 仅返回 `resolved_api_type() == target` 或 `auto` 的 provider。
- 集成 P4 的过滤逻辑。

### 4. Admin API 暴露

- `GET` 接口在响应中包含新字段（已序列化）。
- 写入接口（如有）支持新字段；旧 payload 缺字段时使用默认值。
- `cc-switch` 与 ZZ 共用配置 schema 的部分需同步检查（仅文档说明，不必同 PR 改动）。

### 5. `config.toml.example` 更新

- 新增样例片段，注释解释每个值的含义与默认值。
- 至少给出一份 OpenAI Chat provider 的示例（用于 `/a2o/*`）。

## Files Touched

- `src/config.rs`
- `src/provider.rs`（`select_for_target` + `resolved_api_type`）
- `src/admin_api.rs`（响应字段）
- `src/logging.rs`（注入 conversion log level）
- `config.toml.example`
- `tests/config_defaults.rs`
- `tests/config_new_fields.rs`

## Acceptance Criteria

- 旧 `config.toml`（不含新字段）加载行为与 main 分支字节级等价。
- 新字段单测：默认值、显式值、非法值的容错（非法 `api_type` → 视为 `auto` + warn）。
- Admin API 序列化输出包含新字段且兼容旧客户端（字段缺省解析）。
- `cargo test` 全绿。

## Non-Goals

- 不实现 `auto` 的真正运行时推断（首版固定回退 OpenAIChat）。
- 不引入新的 provider 选择策略（仅按 target api_type 过滤）。
