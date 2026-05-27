# Phase P1 — Skeleton: ApiType + Converter trait + ConversionError

**Depends on:** P0
**Type:** foundation
**Goal:** 引入数据类型与 trait，但不接入路由；保证 `cargo build` 通过、`cargo test converter::` 全绿（仅桩测试）。

---

## Scope

- 新增模块 `src/converter/mod.rs`（如目录化）或 `src/converter.rs`（按现有风格统一选择）。
- 暴露：`ApiType`、`ApiConverter` trait、`ConversionError`、`ConversionErrorKind`。
- 占位实现：`AnthropicToOpenAIConverter`、`OpenAIChatToAnthropicConverter`，方法返回 `Err(NotImplemented)`。
- `target_path(source, target, inbound_path)` 公共函数（首版仅返回 `Err(NotImplemented)`，路径表在 P4 填充）。
- `main.rs` 仅 `mod converter;`，不接路由。

## Files Touched

- `src/converter/mod.rs`（新增）
- `src/converter/types.rs`（新增，可选拆分）
- `src/converter/error.rs`（新增，可选拆分）
- `src/main.rs`（追加 `mod converter;`）
- `Cargo.toml`（如需 `thiserror` 视情况添加）

## Acceptance Criteria

- `cargo build` 与 `cargo clippy -- -D warnings` 通过。
- `cargo test converter` 至少 4 个测试通过：
  - `api_type_display` / `from_str` 双向。
  - `conversion_error_truncates_body_to_4kib_utf8_safe`。
  - `not_implemented_returns_expected_kind`。
  - `target_path_returns_not_implemented_for_now`。
- 模块 doc-comment 引用 `field-mapping.md` 与 `error-model.md`。

## Non-Goals

- 不实现任何字段映射。
- 不修改 `proxy.rs` / `rewriter.rs` / `main.rs` 路由。
- 不读取/修改任何 provider 配置。
