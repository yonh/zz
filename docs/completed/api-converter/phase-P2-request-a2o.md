# Phase P2 — Anthropic → OpenAI Chat 请求体转换

**Depends on:** P1
**Type:** impl
**Goal:** 实现非流式请求体的字段映射；产出 `AnthropicToOpenAIConverter::convert_request`。

---

## Scope（严格按 `field-mapping.md` §2 执行）

1. `system` (string / array) → `messages[0]` system 消息（多段 `\n\n` 拼接）。
2. `messages[].content`：
   - string 直通；
   - array of `{type:"text",text}` 拼接为字符串（无 image 时）；
   - 含 `image` 时输出 OpenAI `content[]` 数组形式（base64 → data URL）。
3. `tool_use` block → 同消息 `tool_calls[]`。
4. `tool_result` block → 拆出独立 `{role:"tool",tool_call_id,content}` 消息。
5. `tools` schema 重排：`input_schema` → `parameters`。
6. `tool_choice` 四种形态映射 + `disable_parallel_tool_use → parallel_tool_calls=false`。
7. `max_tokens` 输出键由 `provider.api_type` 决策（接口签名预留参数 `target_quirks: TargetQuirks`，首版结构体字段可空，由 P4/P7 填充）。
8. 跳过字段：`top_k`、`anthropic_beta`、`anthropic_version`、`metadata.*`（除 `user_id`），统一记录 `field_skipped` warn 日志。
9. 未知顶层字段：warn + 跳过，不报错。

## Error Cases（必须返回 `ConversionError`）

| 场景 | 短码 | field_path |
|---|---|---|
| body 非合法 JSON | `invalid_json` | None |
| 顶层缺 `messages` 且 `system` 也缺 | `missing_field` | `request.messages` |
| `messages` 非数组 | `bad_type` | `request.messages` |
| `messages[i].role` 不在 {user,assistant} | `bad_type` | `request.messages[i].role` |
| `content` block `type` 未识别 | `unsupported_block` | `request.messages[i].content[j].type` |
| `tools[i].input_schema` 非对象 | `bad_type` | `request.tools[i].input_schema` |

## Files Touched

- `src/converter/anthropic_to_openai.rs`（新增）
- `src/converter/mod.rs`（导出 + 装配）
- `tests/converter_request_a2o.rs`（集成单测）

## Acceptance Criteria

- 表驱动单测 ≥10 用例：纯文本、含 image、含 tool_use、含 tool_result、system string、system array、tools 重排、tool_choice 四态、max_tokens 双键、未知字段 skip。
- 错误用例 ≥6（对应上表每条）。
- 所有用例断言：(a) 输出 JSON 与期望字节级或 `serde_json::Value` 等价；(b) 失败用例 `field_path` 与 `kind` 精确匹配。
- `cargo clippy -- -D warnings` 通过。

## Non-Goals

- 不做响应转换（P3）。
- 不做流式（P5）。
- 不接路由（P4）。
