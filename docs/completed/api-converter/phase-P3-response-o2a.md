# Phase P3 — OpenAI Chat → Anthropic 响应体转换（非流）

**Depends on:** P2
**Type:** impl
**Goal:** 实现 `OpenAIChatToAnthropicConverter::convert_response(.., is_stream=false)`。

---

## Scope（严格按 `field-mapping.md` §3 执行）

1. 顶层包装：`type=message`、`role=assistant`、复制 `id`/`model`、`stop_sequence=null`。
2. `choices[0].message.content`：
   - 非空 string → `content:[{type:"text",text}]`；
   - 空且无 tool_calls → `content:[]`；
   - 数组形式 → 逐项映射，未知项 skip+warn。
3. `tool_calls[]` → 追加 `tool_use` blocks（`arguments` JSON.parse；失败时 `input:{}` 并记录 `tool_args_invalid_json`，但不视为致命错误，继续处理其余字段）。
4. `finish_reason` → `stop_reason`：`stop→end_turn`, `length→max_tokens`, `tool_calls/function_call→tool_use`, 其它 → `end_turn` + warn。
5. `usage`：`prompt_tokens→input_tokens`、`completion_tokens→output_tokens`、`prompt_tokens_details.cached_tokens→cache_read_input_tokens`（若存在）。
6. `choices[1..]` skip+warn。
7. `system_fingerprint` 静默忽略。

## Error Cases（致命）

| 场景 | 短码 | field_path |
|---|---|---|
| body 非合法 JSON | `invalid_json` | None |
| 缺 `choices` 或为空数组 | `missing_field` | `response.choices` |
| `choices[0].message` 缺失 | `missing_field` | `response.choices[0].message` |
| `choices[0].message.content` 与 `tool_calls` 同时缺失 | `missing_field` | `response.choices[0].message` |

非致命：`arguments` 解析失败、`finish_reason` 未识别、未知 content 项、多 choices —— 仅日志，不报错。

## OpenAI 错误响应映射

若上游返回 `{"error":{...}}`：
- 转换为 Anthropic 错误体（见 `field-mapping.md` §5）。
- 状态码透传。
- `X-Conversion-Status: success`（成功转换错误体也算成功）。

## Files Touched

- `src/converter/openai_to_anthropic.rs`（新增）
- `src/converter/mod.rs`
- `tests/converter_response_o2a.rs`

## Acceptance Criteria

- 单测 ≥8：纯文本、tool_calls（合法 args、非法 args）、length、empty content、多 choices、cached tokens、未知 finish_reason、错误响应体映射。
- 错误用例覆盖 4 条致命场景。
- 输出严格符合 Anthropic Messages 响应 schema（字段名/层级）。
- `cargo clippy -- -D warnings` 通过。

## Non-Goals

- 不做流式。
- 不实现反向（Anthropic→OpenAI）的响应转换（用于 a2o 链路无需此方向）。
