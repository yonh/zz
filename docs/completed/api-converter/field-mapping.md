# Field Mapping: Anthropic ↔ OpenAI Chat Completions

> Phase P0 deliverable. 冻结后作为 P2/P3/P5 实现的唯一字段映射依据。

**约定：**
- `→` 表示直接复制（语义等价）。
- `⇒` 表示需要变形/重排。
- `skip+warn` 表示丢弃并记录 `field_skipped` 日志。
- `default` 表示目标侧无此概念时填默认值。
- 路径使用 `request.foo[0].bar` 风格，便于 `ConversionError.field_path`。

---

## 1. 路径与 HTTP 方法

| 方向 | 入站路径 | 出站路径 | 方法 |
|---|---|---|---|
| a2o | `/a2o/v1/messages` | `/v1/chat/completions` | POST |
| o2a | `/o2a/v1/chat/completions` | `/v1/messages` | POST |

`converter::target_path(source, target, inbound_path)` 负责输出。其余路径段（如 query string）原样保留。

---

## 2. 请求体：Anthropic → OpenAI Chat (a2o)

### 2.1 顶层字段

| Anthropic | OpenAI Chat | 处理 |
|---|---|---|
| `model` | `model` | → （provider 重写时可二次替换） |
| `messages[]` | `messages[]` | ⇒ 见 2.2 |
| `system` (string) | prepend `messages[0]={role:"system",content:<string>}` | ⇒ |
| `system` (array of `{type:"text",text}`) | prepend system 消息，content = 拼接所有 text，分隔符 `\n\n` | ⇒ |
| `max_tokens` | `max_tokens` 或 `max_completion_tokens` | ⇒ 由 `provider.api_type` + 模型名决定（reasoning 模型用 `max_completion_tokens`） |
| `temperature` | `temperature` | → |
| `top_p` | `top_p` | → |
| `top_k` | — | skip+warn |
| `stop_sequences[]` | `stop` | → （数组保留） |
| `stream` | `stream` | → |
| `tools[]` | `tools[]` | ⇒ 见 2.3 |
| `tool_choice` | `tool_choice` | ⇒ 见 2.4 |
| `metadata.user_id` | `user` | → |
| `metadata.*` (其它) | — | skip+warn |
| `anthropic_version` | — | skip+warn |
| `anthropic_beta` | — | skip+warn |
| `service_tier` | `service_tier` | → 若存在 |
| 其它未知字段 | — | skip+warn (`field_skipped=<name>`) |

### 2.2 messages 转换

Anthropic role: `user` | `assistant`。System 来自顶层 `system`。

每条 message 的 `content`：

| Anthropic block | OpenAI 对应 | 处理 |
|---|---|---|
| string（整条 content） | `content: string` | → |
| `{type:"text", text}` | 拼接到 `content` 字符串（首版） | ⇒ 多 text block 用 `\n` 拼接 |
| `{type:"image", source:{type:"base64",media_type,data}}` | `content[]` 中的 `{type:"image_url", image_url:{url:"data:<media_type>;base64,<data>"}}` | ⇒ 出现 image 时 `content` 必须为数组形式 |
| `{type:"image", source:{type:"url", url}}` | `{type:"image_url", image_url:{url}}` | ⇒ |
| `{type:"tool_use", id, name, input}` (assistant) | `tool_calls[]` 中 `{id, type:"function", function:{name, arguments: JSON.stringify(input)}}` | ⇒ 该消息 `content` 设为 `null` 或保留 text |
| `{type:"tool_result", tool_use_id, content}` (user) | 单独输出一条 `{role:"tool", tool_call_id, content}` 消息 | ⇒ 拆分为独立消息 |
| 其它未知 type | skip+warn | |

**输出格式选择：** 当所有 block 都是纯 text 时输出 `content: <string>`；含 image/混合时输出 `content: [...]`。

### 2.3 tools schema 重排

| Anthropic | OpenAI |
|---|---|
| `[{name, description, input_schema}]` | `[{type:"function", function:{name, description, parameters: <input_schema>}}]` |

`input_schema` 直接作为 `parameters`（均为 JSON Schema）。`cache_control` 等 Anthropic 特有字段 skip+warn。

### 2.4 tool_choice 映射

| Anthropic | OpenAI |
|---|---|
| `{type:"auto"}` | `"auto"` |
| `{type:"any"}` | `"required"` |
| `{type:"tool", name}` | `{type:"function", function:{name}}` |
| `{type:"none"}` | `"none"` |
| `disable_parallel_tool_use=true` | `parallel_tool_calls=false` |

---

## 3. 响应体（非流）：OpenAI Chat → Anthropic (o2a)

### 3.1 顶层包装

| OpenAI | Anthropic | 处理 |
|---|---|---|
| `id` | `id` | → |
| `model` | `model` | → |
| `object="chat.completion"` | `type="message"` | 固定值 |
| — | `role="assistant"` | 固定值 |
| `choices[0].message.content` | `content[]` | ⇒ 见 3.2 |
| `choices[0].message.tool_calls[]` | `content[]` 中追加 `tool_use` block | ⇒ 见 3.3 |
| `choices[0].finish_reason` | `stop_reason` | ⇒ 见 3.4 |
| — | `stop_sequence` | `null`（OpenAI 无对应概念） |
| `usage.prompt_tokens` | `usage.input_tokens` | → |
| `usage.completion_tokens` | `usage.output_tokens` | → |
| `usage.prompt_tokens_details.cached_tokens` | `usage.cache_read_input_tokens` | → 若存在 |
| `system_fingerprint` | — | skip（不影响功能） |
| `choices[1..]` | — | skip+warn（Anthropic 单 message） |

### 3.2 content 文本

- `message.content` 为 string 且非空 ⇒ `content: [{type:"text", text:<string>}]`。
- 为空且无 tool_calls ⇒ `content: []`。
- 为 array（vision 输出，少见）⇒ 逐项映射，未知项 skip+warn。

### 3.3 tool_calls

每个 `tool_calls[i]`：
```
{type:"tool_use", id: <id>, name: function.name, input: JSON.parse(function.arguments)}
```
`arguments` 解析失败：`input: {}` + `field_skipped=tool_calls[i].arguments_invalid_json`，并保留原字符串到 `original_value`。

### 3.4 finish_reason → stop_reason

| OpenAI | Anthropic |
|---|---|
| `stop` | `end_turn` |
| `length` | `max_tokens` |
| `tool_calls` / `function_call` | `tool_use` |
| `content_filter` | `end_turn` + warn |
| `null` / 未知 | `end_turn` + warn |

---

## 4. 流式 SSE 映射（P5）

### 4.1 OpenAI Chat 流事件

```
data: {choices:[{index:0, delta:{role:"assistant"}}]}
data: {choices:[{index:0, delta:{content:"Hel"}}]}
data: {choices:[{index:0, delta:{content:"lo"}}]}
data: {choices:[{index:0, delta:{tool_calls:[{index:0,id,function:{name,arguments:"{\"a\""}}]}}]}
data: {choices:[{index:0, delta:{tool_calls:[{index:0,function:{arguments:":1}"}}]}]}
data: {choices:[{index:0, delta:{}, finish_reason:"stop"}], usage:{...}}
data: [DONE]
```

### 4.2 Anthropic 目标事件序列

```
event: message_start            data: {type:"message_start", message:{id,model,role:"assistant",content:[],usage:{input_tokens,output_tokens:0}}}
event: content_block_start      data: {type:"content_block_start", index:0, content_block:{type:"text", text:""}}
event: content_block_delta      data: {type:"content_block_delta", index:0, delta:{type:"text_delta", text:"Hel"}}
event: content_block_delta      data: {type:"content_block_delta", index:0, delta:{type:"text_delta", text:"lo"}}
event: content_block_stop       data: {type:"content_block_stop", index:0}
# 若有 tool_calls：再开一个 index=1 的 tool_use block
event: content_block_start      data: {type:"content_block_start", index:1, content_block:{type:"tool_use", id, name, input:{}}}
event: content_block_delta      data: {type:"content_block_delta", index:1, delta:{type:"input_json_delta", partial_json:"{\"a\""}}
event: content_block_delta      data: {type:"content_block_delta", index:1, delta:{type:"input_json_delta", partial_json:":1}"}}
event: content_block_stop       data: {type:"content_block_stop", index:1}
event: message_delta            data: {type:"message_delta", delta:{stop_reason:"end_turn", stop_sequence:null}, usage:{output_tokens}}
event: message_stop             data: {type:"message_stop"}
```

### 4.3 状态机要点

- `message_id`：取首个 OpenAI chunk 的 `id`，整段复用。
- `current_text_block_index`：首个 text delta 时分配 index 0，结束时 stop。
- `tool_blocks`：按 `tool_calls[i].index` 维护映射；`function.name` 在 start 时确定，`arguments` 用 `input_json_delta` 累加。
- 收到 `finish_reason` ⇒ 关闭所有未关闭 block ⇒ 发 `message_delta` + `message_stop`。
- 上游 `data: [DONE]` ⇒ 兜底关闭并发 `message_stop`。
- 半包：按 `\n\n` 切事件，跨 chunk 缓冲。

### 4.4 反向 a2o 流（O2A 的对偶）

P5 同步实现：解析 Anthropic SSE，输出 OpenAI delta chunks。规则镜像 4.2，留待 P5 补充事件级单测样本。

---

## 5. 错误响应映射

| OpenAI 错误体 | Anthropic 错误体 |
|---|---|
| `{error:{message,type,code,param}}` | `{type:"error", error:{type, message}}` |
| HTTP 状态码 | 透传 |

`type` 映射：

| OpenAI `type` | Anthropic `error.type` |
|---|---|
| `invalid_request_error` | `invalid_request_error` |
| `authentication_error` | `authentication_error` |
| `permission_error` | `permission_error` |
| `not_found_error` | `not_found_error` |
| `rate_limit_exceeded` | `rate_limit_error` |
| `server_error` / 5xx 默认 | `api_error` |
| 其它未知 | `api_error` + warn |

---

## 6. 已知未覆盖项（首版 skip+warn，列入 roadmap）

- Anthropic prompt caching (`cache_control`)。
- OpenAI `logprobs`、`response_format`（json_schema）双向。
- OpenAI `parallel_tool_calls=false` 反向到 Anthropic `disable_parallel_tool_use`（资源响应方向）。
- 多 `choices[]` 输出。
- Vision 完整映射（base64 vs URL 边界）。
- Audio modality。
