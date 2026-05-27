# Phase P9 — Iteration Telemetry: 增量迭代闭环

**Depends on:** P6（日志规范）、P7（配置）
**Type:** observability / iteration loop
**Goal:** 让程序从第一天就主动产出可被人/工具消费的"问题数据"，把"日常使用 → 发现错误/缺失字段 → 增量修复"做成闭环。不依赖偶然抓到的日志，而是在代码里**主动结构化采集**。

---

## 1. 设计原则

- **默认开启、低开销**：所有 converter 都内建采样上报，无需额外配置即可工作。
- **结构化优先**：上报项是带 schema 的事件，不是自由文本日志。
- **可回放**：每个失败/未知样本都保留足够字节数，可在本地用 `cargo run --bin convert-replay` 重放。
- **可聚合**：按 `(direction, error_code, field_path)` 去重计数，避免同一问题灌满磁盘。
- **可对外**：通过 admin API / UI 暴露给开发者，无需 SSH 看日志。
- **可演进**：converter 携带 `version` 字段，便于"修了之后看是否还在发生"。

---

## 2. 数据模型

```rust
// src/converter/telemetry.rs
pub struct ConversionEvent {
    pub ts: chrono::DateTime<Utc>,
    pub req_id: String,
    pub route: String,                       // /a2o/v1/messages
    pub source: ApiType,
    pub target: ApiType,
    pub phase: Phase,                        // Request | Response | Stream
    pub kind: EventKind,                     // Success | FieldSkipped | UnknownField | Fallback | Error
    pub error_code: Option<&'static str>,    // error-model.md §2 短码
    pub field_path: Option<String>,
    pub upstream_status: Option<u16>,
    pub converter_version: &'static str,     // git short sha 或 semver
    pub sample_id: Option<u64>,              // 关联到 sample store
}

pub struct ConversionSample {
    pub id: u64,
    pub event_signature: String,             // hash(direction, error_code, field_path)
    pub first_seen: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    pub hit_count: u64,
    pub request_body_preview: Bytes,         // 截断 4KiB
    pub response_body_preview: Option<Bytes>,
    pub redacted: bool,                      // 是否经过敏感字段脱敏
}
```

**事件签名（去重 key）：** `sha1(direction|error_code|field_path|converter_version)`，用于把"同一问题第 1 次和第 1000 次"折叠成一行带计数的记录。

---

## 3. 存储

复用现有 `request_journal` 基础设施（已存在于 `src/request_journal.rs`），新增独立子存储：

- **事件流**：环形缓冲（内存）+ 可选追加写入磁盘 JSONL（路径配置）。
- **样本表**：按 `event_signature` 去重，`hit_count++`、更新 `last_seen`，仅在首次或每 N 次（默认 100）保留新 body 样本。
- **大小上限**：默认 64MB / 1 万条样本，FIFO 淘汰；可配置。
- **脱敏**：保存前对 Authorization、`api_key`、`x-api-key` 字段做掩码（与 `rewriter.rs` 既有清理同源工具）。

## 4. 采集点（converter 内部主动上报）

P2/P3/P5 的字段映射代码必须调用：

```rust
ctx.report_field_mapped("system", "messages[0]");
ctx.report_field_skipped("top_k", FieldSkipReason::UnsupportedInTarget);
ctx.report_unknown_field("metadata.custom_xyz");      // 新关键 API
ctx.report_error(ConversionError { ... });
```

`report_unknown_field` 是核心：**任何不在已知映射白名单中的字段**都自动上报，这是发现"我们漏考虑的接口细节"的主入口。实现方式：

- 在请求/响应转换器入口处用 `serde_json::Value` 解析；
- 维护硬编码 `KNOWN_FIELDS_REQUEST_ANTHROPIC` / `KNOWN_FIELDS_RESPONSE_OPENAI_CHAT` 等集合；
- 遍历顶层与已知嵌套路径，差集即未知。
- 路径用 `field-mapping.md` 既定 dotted notation。

## 5. 字段覆盖率指标

输出 `GET /admin/api/conversion/coverage`：

```json
{
  "converter_version": "abc1234",
  "directions": [{
    "source": "Anthropic", "target": "OpenAIChat",
    "request": { "known_fields_seen": 14, "unknown_fields_seen": 3, "top_unknown": [{"path":"metadata.custom_xyz","count":42}] },
    "response": { "known_fields_seen": 11, "unknown_fields_seen": 1, "top_unknown": [...] },
    "stream":   { ... }
  }]
}
```

含义：让开发者一眼看到"还有哪些字段在线上出现但我们没处理"。

## 6. 回放工具

新增 `src/bin/convert_replay.rs`：

```
convert-replay --sample-id 12345
convert-replay --signature <hash>
convert-replay --file path/to/captured.json
```

行为：从样本表（或 JSON 文件）读出原始 body → 用当前代码再跑一遍 `convert_request`/`convert_response` → 打印新结果与历史结果的 diff。用于：
- 修了某个字段后验证"现在不再 skip"。
- TDD：把样本固化成 `tests/fixtures/regressions/<hash>.json`，加进自动化套件。

## 7. Admin API & UI

**API（已有 admin 体系）：**
- `GET /admin/api/conversion/events?since=...&kind=...&limit=...`
- `GET /admin/api/conversion/samples?signature=...`
- `GET /admin/api/conversion/samples/{id}/body`（脱敏后原文，用于复现）
- `GET /admin/api/conversion/coverage`
- `POST /admin/api/conversion/samples/clear`（手动清理）

**UI（`ui/src/pages/`）：**
- 新增 `Conversion.tsx`：
  - Top issues 列表（按 hit_count 降序，显示签名、错误码、字段路径、首次/最近时间、样例预览）。
  - 点击某行 → 详情页，展示 redacted body、可一键"下载样本 JSON"。
  - Coverage tab：展示已知/未知字段统计。
- 复用 `Playground.tsx` 模式（已在 memory 中），无需新框架依赖。

## 8. 配置

`config.toml`（默认值即可工作，所有项可省略）：

```toml
[conversion.telemetry]
enabled = true
sample_max_count = 10000          # 样本上限
sample_max_bytes = 67108864       # 64 MiB
sample_resave_every = 100         # 同 signature 每 N 次刷新一次 body
persist_path = ""                 # 空=仅内存；非空=追加 JSONL
unknown_field_log_level = "warn"  # debug/info/warn
redact_extra_headers = []         # 用户自定义敏感头
```

## 9. 与日志（P6）的关系

- 日志是**人**第一时间看的；telemetry 是**程序与 UI**消费的。
- 同一事件在两边都产出，但 telemetry 多了：
  - 跨请求去重计数；
  - body 样本持久化；
  - 可被 admin API/UI 查询；
  - 可被 replay 工具重放。

---

## 10. Files Touched

- `src/converter/telemetry.rs`（新增）
- `src/converter/known_fields.rs`（新增：白名单常量）
- `src/converter/mod.rs`（注入 ctx）
- `src/converter/anthropic_to_openai.rs` / `openai_to_anthropic.rs`（call-site 接入）
- `src/converter/stream.rs`（流式事件采集）
- `src/admin_api.rs`（新增 conversion endpoints）
- `src/request_journal.rs`（如需复用其存储抽象）
- `src/bin/convert_replay.rs`（新增）
- `src/config.rs`（telemetry 配置 section）
- `ui/src/pages/Conversion.tsx`（新增）
- `ui/src/api/conversion.ts`（新增）
- `tests/converter_telemetry.rs`、`tests/converter_replay.rs`、`tests/integration_admin_conversion.rs`

## 11. Acceptance Criteria

- 启动 ZZ 后无任何额外配置，跑一组 a2o/o2a 请求 → `GET /admin/api/conversion/events` 立即有结构化记录。
- 故意发送含未知字段 `metadata.custom_xyz` 的 Anthropic 请求 → 该字段出现在 `coverage.top_unknown` 中，且 hit_count 随重复请求递增（去重生效）。
- `convert-replay --signature <hash>` 能从样本重放并输出转换结果。
- 样本中 Authorization 头与 api_key 字段已脱敏（grep 不到原值）。
- 内存占用上限受 `sample_max_bytes` 控制：注入 100MB 噪声后仍稳定在 64MiB 上下。
- UI Conversion 页面可看到 top issues 列表与详情。
- `cargo test` 全绿；`cargo clippy -- -D warnings`。

## 12. 迭代工作流（团队约定，文档化）

写入 `docs/dev/api-converter.md` "Iteration Loop" 小节：

1. 用户/客户端反馈某场景失败 → 复制 `req_id` 或 `signature`。
2. 在 UI Conversion 页或 admin API 取出脱敏样本。
3. 用 `convert-replay` 本地复现。
4. 在 `tests/fixtures/regressions/<signature>.json` 固化样本 + 期望输出，加入 `tests/converter_regressions.rs` 自动化用例（先红）。
5. 在 `field-mapping.md` 增补该字段映射或将其从 unknown 列表迁出。
6. 修代码 → 测试转绿。
7. `converter_version` 自然变化，UI 上该 signature 不再产生新事件即视为"已修复"。

## 13. Non-Goals

- 不接入外部 APM/OTel（首版）；后续可在 telemetry layer 之上加 exporter。
- 不做线上 body 全量持久化；仅按 signature 抽样。
- 不实现自动字段映射推断（仅记录 + 让人来加）。
