# Dev Prompts — API Converter

> 交给 coding agent 使用的开发提示词集合。**主提示词**用于驱动整个项目；**阶段提示词**逐阶段使用，每段都可独立粘贴运行。所有 prompt 都强约束：先读规划文档、再动代码、不破坏现状。

---

## 0. Master Prompt（项目级，长任务编排用）

```
你是 ZZ proxy 项目的 Rust 开发 agent。

项目路径：/Users/yonh/workspaces/rust/zz
项目语言：Rust（axum/hyper/tokio/serde_json/bytes/tracing），前端 React+TS（Vite）。
当前分支策略：每个阶段创建独立分支 feat/api-converter-P<N>，PR 合并后再开下一阶段。

【强制阅读，开始任何编码前必须读完】
1. docs/plans/2026-05-04-api-converter-plan/_index.md
2. docs/plans/2026-05-04-api-converter-plan/field-mapping.md
3. docs/plans/2026-05-04-api-converter-plan/error-model.md
4. docs/plans/2026-05-04-api-converter-plan/route-matrix.md
5. 你正在执行的那一阶段对应的 phase-P<N>-*.md
6. 相关源文件：src/main.rs, src/proxy.rs, src/rewriter.rs, src/provider.rs, src/config.rs, src/request_journal.rs, src/admin_api.rs

【全局规则】
- 不要修改 /v1/* 透明代理的现有行为（字节级一致）。
- 不要一次性实现所有协议；严格按阶段（P1→P2→P3→P4→P5→P6→P7→P9→P8）推进，P7 与 P9 可与 P5/P6 并行。
- 转换失败时降级返回原始响应（响应侧）或 502 错误体（请求侧），永不无降级地把错误抛给客户端。
- 所有源代码（注释、日志、错误信息、配置注释、UI 字符串）必须使用英文；本规划文档保持中文。
- 每个函数/struct 增加 doc-comment，复杂逻辑加行内注释。
- 提交信息使用 `type(scope): subject`，例如 `feat(converter): add ApiType enum and trait skeleton`。
- 不创建临时脚本或一次性辅助文件留在仓库里。
- 每完成一个阶段：cargo build && cargo clippy --all-targets -- -D warnings && cargo test 全绿后再进入下一阶段。

【输出要求】
- 每阶段开始前先生成一个简短的 task list（in_progress 仅一项）。
- 阶段结束时输出：变更文件清单、跑过的命令清单、acceptance criteria 勾选状态。
- 任何偏离规划文档的设计决策必须在阶段结束时回写到对应 phase-P<N>-*.md。

【完成定义（DoD）】
当前阶段 phase-P<N>-*.md 文件中 "Acceptance Criteria" 全部勾选；cargo test/clippy/build 全绿；新增/修改的字段映射规则已与 field-mapping.md 对齐；如新增协议路径已更新 route-matrix.md；如新增错误码已更新 error-model.md。

请从 P1 开始。
```

---

## 1. Phase P1 — Skeleton

```
任务：实现 docs/plans/2026-05-04-api-converter-plan/phase-P1-skeleton.md。

强制先读：
- _index.md, field-mapping.md, error-model.md, route-matrix.md, phase-P1-skeleton.md
- src/main.rs（确认现有 mod 声明风格）

要做：
1. 新增 src/converter/mod.rs（按现有项目风格选择目录化或单文件，如 src/proxy.rs 是单文件，则用 src/converter.rs；如有 src/foo/mod.rs 模式则用目录）。
2. 暴露：
   - pub enum ApiType { Anthropic, OpenAIChat, OpenAICompletions, OpenAIResponses, Gemini, Unknown }
     impl Display + FromStr（按 error-model.md 中字段命名："Anthropic","OpenAIChat",...）
   - pub trait ApiConverter {
       fn convert_request(&self, body: &Bytes, target: ApiType) -> Result<Bytes, ConversionError>;
       fn convert_response(&self, body: &Bytes, source: ApiType, target: ApiType, is_stream: bool) -> Result<Bytes, ConversionError>;
     }
   - pub struct ConversionError { message, field_path, original_value, original_body, kind }
   - pub enum ConversionErrorKind { InvalidJson, SchemaMismatch, UnsupportedFeature, StreamProtocol, Internal, NotImplemented }
     impl 提供 short_code() -> &'static str（按 error-model.md §2 表）。
   - pub fn target_path(source: ApiType, target: ApiType, inbound_path: &str) -> Result<String, ConversionError>
     首版仅返回 NotImplemented。
3. 占位实现：AnthropicToOpenAIConverter、OpenAIChatToAnthropicConverter，方法均返回 Err(NotImplemented)。
4. 提供 UTF-8 安全截断工具 truncate_bytes_utf8(b: &[u8], max: usize) -> Bytes，含 emoji 边界处理（按 error-model.md §5）。
5. main.rs 仅追加 `mod converter;`，不接路由。

不要做：
- 不实现任何字段映射。
- 不修改 proxy.rs / rewriter.rs / 路由。
- 不读取 provider 配置。

测试（必写，全部通过）：
- api_type_display_and_from_str_roundtrip
- conversion_error_kind_short_code_matches_table
- truncate_bytes_utf8_handles_4byte_emoji_boundary
- not_implemented_returns_expected_kind
- target_path_returns_not_implemented_for_now

验收：
- cargo build && cargo clippy --all-targets -- -D warnings && cargo test converter 全绿。
```

---

## 2. Phase P2 — Anthropic → OpenAIChat 请求体转换

```
任务：实现 phase-P2-request-a2o.md。前置阶段 P1 已合并。

强制先读：
- field-mapping.md（§2 全文，逐字段对照实现）
- error-model.md（§1, §2, §3）
- phase-P2-request-a2o.md
- P9 phase-P9-iteration-telemetry.md §4 §10（必须在每个字段处理点调用 telemetry 采集 API；首版可先定义 trait/no-op 实现，P9 阶段再接真实存储）

要做：
1. 新增 src/converter/anthropic_to_openai.rs，实现 AnthropicToOpenAIConverter::convert_request。
2. 严格按 field-mapping.md §2 处理：
   - system (string/array) → messages[0] system 拼接
   - messages[].content 字符串/数组/含 image/tool_use/tool_result 拆分
   - tools schema 重排（input_schema -> parameters）
   - tool_choice 四态映射 + disable_parallel_tool_use → parallel_tool_calls=false
   - max_tokens 输出键由 TargetQuirks 决定（首版结构体定义好但字段值固定 max_tokens；P7 接配置）
   - 跳过字段 top_k/anthropic_beta/anthropic_version/metadata.* 调 report_field_skipped
   - 未知顶层字段调 report_unknown_field（首版 telemetry trait 可 no-op）
3. 错误返回严格匹配 phase-P2 中 "Error Cases" 表（短码 + field_path）。
4. 表驱动单测 ≥10 + 错误用例 ≥6，断言 (a) 输出 JSON 等价；(b) 失败 field_path/kind 精确。

不要做：
- 不实现响应转换、流式、路由接入。

验收：
- cargo test converter::anthropic_to_openai 全绿；clippy 无警告。
- 输出经 serde_json 解析后字段集合与期望集合相等（不依赖 key 顺序）。
```

---

## 3. Phase P3 — OpenAIChat → Anthropic 响应体转换（非流）

```
任务：实现 phase-P3-response-o2a.md。

强制先读：field-mapping.md §3 §5、error-model.md、phase-P3-response-o2a.md。

要做：
1. 新增 src/converter/openai_to_anthropic.rs，实现 OpenAIChatToAnthropicConverter::convert_response（is_stream=false 分支）。
2. 严格按 §3 处理顶层包装、content 文本、tool_calls、stop_reason、usage、cached_tokens。
3. tool_calls.function.arguments 解析失败：input={}，记录 field_skipped=tool_calls[i].arguments_invalid_json 但**不**返回 Err。
4. 上游错误体 {"error":{...}} → Anthropic 错误体（§5 type 映射），状态码透传，仍标 success。
5. 致命错误按 phase-P3 表返回。
6. 单测 ≥8（含 tool_calls 合法/非法、length、empty content、多 choices、cached tokens、未知 finish_reason、错误响应体）+ 错误用例 ≥4。

不要做：
- 不做流式（is_stream=true 仍返回 NotImplemented）。
- 不做反向 Anthropic→OpenAI 响应转换。

验收：cargo test 全绿；输出严格符合 Anthropic Messages 响应 schema。
```

---

## 4. Phase P4 — 路由分发 + conversion_proxy_handler

```
任务：实现 phase-P4-routing-handler.md。

强制先读：
- route-matrix.md 全文（§3 严格匹配顺序、§4 职责边界、§5 provider 选择）
- phase-P4-routing-handler.md
- src/main.rs, src/proxy.rs, src/rewriter.rs, src/provider.rs

要做：
1. main.rs 严格按 route-matrix.md §3 顺序匹配前缀；/a2r/* /r2a/* /anthropic/* /openai/* /responses/* 返回 501 + 错误体（首版桩）。
2. 在 src/proxy.rs 中提取共享 helper（provider 选择、attempt_request、retry），新增 conversion_proxy_handler(req, state, source, target)。
3. 流程严格按 phase-P4 §2 步骤；流式分支留 todo!() + 注释，等 P5 实装。
4. converter::target_path 实装首版两条映射；其它子路径返回 unsupported_feature。
5. rewriter.rs 移除（如有）协议级路径硬编码分支；只管 host/auth。
6. provider.rs 新增 select_for_target(state, target, model)，按 resolved_api_type 过滤；无匹配返回 None。
7. 集成测试：
   - tests/integration_conversion_a2o.rs：mock 上游 server，断言上游收到 OpenAI 格式、客户端收到 Anthropic 格式。
   - tests/integration_conversion_o2a.rs：反向。
   - tests/integration_v1_passthrough.rs：/v1/messages /v1/chat/completions 字节级回归。
   - tests/integration_no_matching_provider.rs：返回 502 + no_matching_provider_for_target_api。

不要做：
- 不修改 /v1/* 行为。
- 不实现流式（P5）。
- 不引入新 provider 选择策略，仅按 target api_type 过滤。

验收：cargo test 全绿；启动后 admin/UI/ws 路由不受影响；clippy 无警告。
```

---

## 5. Phase P5 — 流式 SSE 双向转换

```
任务：实现 phase-P5-stream.md。

强制先读：field-mapping.md §4、phase-P5-stream.md、P4 已合并代码。

要做：
1. 新增 src/converter/stream.rs，实现 StreamConverter（OAToAnState / AnToOAState 状态机）。
2. SSE 解析按 \n\n 切事件，跨 chunk 半包缓冲；data:[DONE] 触发 finalize。
3. OpenAI→Anthropic 状态机严格按 §3：message_start / content_block_start / content_block_delta(text_delta or input_json_delta) / content_block_stop / message_delta / message_stop。
4. tool_calls 流式：用 input_json_delta 累计 function.arguments 字符串，不中途 JSON.parse。
5. Anthropic→OpenAI 镜像。
6. 异常处理：
   - 状态机违例（如 delta 前未 start）：已发出 message_start 时优雅收尾 + 日志 sse_state；未发出时返 Err 让上层走响应侧降级。
   - 上游断流：已开始发 message_stop；未开始透传。
   - 单事件解析失败：透传该事件原文 + warn sse_parse。
7. proxy.rs 流式分支：包装上游 body stream，调用 StreamConverter::push(chunk) 输出 Vec<Bytes>。
8. 单测 ≥6 + 集成测试；SSE 样本放 tests/fixtures/sse/ 并提交。

不要做：
- 不做 audio/video 流。
- output_tokens 缺失时首版填累计字符近似或 0 + warn，不要做精确 token 推断。

验收：cargo test 全绿；流式集成测试断言事件顺序 + 字段；finalize 后必有 message_stop 且流以 \n\n 结束。
```

---

## 6. Phase P6 — 降级 + 响应头 + 日志规范化

```
任务：实现 phase-P6-fallback-logging.md。

强制先读：error-model.md 全文、phase-P6-fallback-logging.md、当前 P4/P5 中临时实现的位置。

要做：
1. src/converter/error.rs：truncate_bytes_utf8（如 P1 已建则补 emoji 跨边界用例）、ConversionErrorKind::short_code（已建则补全）、error_body(target_inbound, code) 生成对应错误体。
2. src/converter/logging.rs：tracing target=zz::conversion，提供宏/函数封装必填字段（req_id, route, source, target, phase, status）。
3. src/proxy.rs：把 P4/P5 中临时的错误返回与日志替换为统一调用：
   - 请求侧失败 → 502 + error_body + 失败响应头（X-Conversion-Status/Phase/Error）。
   - 响应侧失败 → 透传上游 body + 失败响应头。
   - 流式失败按 P5 §5 行为 + 标头。
   - 成功 → success 头 + Source/Target。
4. enable_conversion_fallback=false 路径：响应侧失败也 502；流式失败 reset。
5. 测试：
   - tests/converter_truncate.rs（4097 ASCII / 4 字节 emoji 跨 4096）
   - tests/integration_fallback_request.rs / response.rs / stream.rs
   - 日志快照（tracing-test 或自写订阅器）覆盖 success/fallback/field_skipped 三类。

不要做：
- 不改字段映射规则。
- 不引入新协议路径。

验收：每个短码至少一条断言；clippy 无警告；cargo test 全绿。
```

---

## 7. Phase P7 — 配置扩展

```
任务：实现 phase-P7-config.md。

强制先读：phase-P7-config.md、src/config.rs、src/admin_api.rs、config.toml.example。

要做：
1. ProviderConfig 增 api_type:String（默认空="auto"）、enable_conversion_fallback:bool（默认 true）。
2. 提供 ProviderConfig::resolved_api_type() -> ApiType；非法值视为 auto + 一次性 warn。
3. 全局/section 增 conversion_log_level:String（默认 "info"），注入 tracing filter（仅影响 zz::conversion target）。
4. provider::select_for_target(state, target, model) 按 resolved_api_type==target 或 auto 过滤；与 P4 已建 helper 对接。
5. admin_api.rs：GET 响应序列化包含新字段；写入接口接受新字段；旧 payload 缺字段使用默认值。
6. config.toml.example 增带注释的示例片段，含 OpenAI Chat provider 示范（用于 /a2o/*）。
7. 测试：
   - tests/config_defaults.rs：旧 config.toml（不含新字段）字节级行为等价。
   - tests/config_new_fields.rs：默认值、显式值、非法值。

不要做：
- 不实现 auto 的运行时推断（首版固定回退 OpenAIChat）。
- 不引入新 provider 选择策略。

验收：旧配置加载零变更；admin API 兼容旧客户端；clippy/cargo test 全绿。
```

---

## 8. Phase P9 — Iteration Telemetry（与 P6/P7 并行可行；P8 之前完成）

```
任务：实现 phase-P9-iteration-telemetry.md。

强制先读：phase-P9-iteration-telemetry.md 全文、src/request_journal.rs、src/admin_api.rs、ui/src/pages/Playground.tsx（参考前端模式）。

要做：
1. 新增 src/converter/telemetry.rs：
   - ConversionEvent / ConversionSample 结构体（按 §2）
   - 事件签名 = sha1(direction|error_code|field_path|converter_version)
   - 内存环形缓冲 + 样本表（按 signature 去重 hit_count++、刷新 last_seen、按 sample_resave_every 周期重存 body）
   - 大小上限 FIFO 淘汰（默认 64MiB / 1万条）
   - 脱敏：保存前清理 Authorization / api_key / x-api-key（复用 rewriter 同源工具）
   - 提供 trait Telemetry { report_field_mapped/skipped/unknown_field/error/success(...) }
   - 提供 NoopTelemetry 与 InMemoryTelemetry 两种实现
2. 新增 src/converter/known_fields.rs：常量集合
   - KNOWN_FIELDS_REQUEST_ANTHROPIC / KNOWN_FIELDS_RESPONSE_OPENAI_CHAT 等
   - 转换器入口处计算 unknown = top_level_keys - known，逐项 report_unknown_field。
3. 改造 P2/P3/P5 的实现，在所有字段处理点调用 telemetry（不是事后补日志）。
4. converter_version 常量：编译期注入 git short sha（build.rs）或 fallback 到 CARGO_PKG_VERSION。
5. admin_api.rs 新增端点：
   - GET /admin/api/conversion/events?since&kind&limit
   - GET /admin/api/conversion/samples?signature
   - GET /admin/api/conversion/samples/{id}/body
   - GET /admin/api/conversion/coverage
   - POST /admin/api/conversion/samples/clear
6. 新增 src/bin/convert_replay.rs：
   - --sample-id / --signature / --file 三种入口
   - 重放 convert_request/convert_response，输出新结果与历史 diff
7. 新增 ui/src/pages/Conversion.tsx + ui/src/api/conversion.ts；在 Layout 中加导航项。Top issues 列表 + 详情页 + Coverage tab。
8. config.toml.example 增 [conversion.telemetry] section（按 §8）。
9. 测试：
   - tests/converter_telemetry.rs：去重、计数、FIFO、脱敏。
   - tests/converter_replay.rs：从样本回放与历史 diff。
   - tests/integration_admin_conversion.rs：5 个端点。

不要做：
- 不接 OTel/外部 APM（首版）。
- 不做线上 body 全量持久化。
- 不实现自动字段映射推断（仅记录）。

验收：
- 启动后零额外配置即跑出结构化事件。
- 故意发未知字段请求 → coverage.top_unknown 出现该字段，重复请求 hit_count 递增。
- convert-replay 能从样本重放。
- grep 样本无 Authorization 原值。
- 注入 100MB 噪声后内存稳定在 64MiB 上下。
- UI Conversion 页可见 top issues 与详情。
- cargo test 全绿；clippy 无警告。
```

---

## 9. Phase P8 — 测试矩阵 + 手动验收 + 文档（最后阶段）

```
任务：实现 phase-P8-verification.md。前置 P1-P7 + P9 全部完成。

要做：
1. 自动化测试矩阵（按 §1 表）：补齐缺失维度的测试用例。
2. cargo test && cargo clippy --all-targets -- -D warnings && cargo build --release 全绿。
3. 写 docs/active-work/api-converter/manual-acceptance.md，含 9 步手动验收 curl 命令与 ☐ 复选框。
4. 写 docs/dev/api-converter.md：架构图、路由前缀语义表（链接 route-matrix.md）、字段映射摘要（链接 field-mapping.md）、错误模型（链接 error-model.md）、Iteration Loop 工作流（来自 P9 §12）、扩展指南（route-matrix.md §7 的 7 步）。
5. 更新 README.md：增加"协议转换"章节，列出当前支持的前缀与未来计划。
6. 更新 CHANGELOG（如有）。
7. Release Checklist 全部勾选后，执行 /complete-and-archive-task 把本计划归档到 docs/completed/。

不要做：
- 不要在文档外引入新功能。
- 不要在此阶段大改 converter 行为；如有发现的字段缺失，走 P9 迭代闭环新建小任务。

验收：所有阶段 acceptance 条目勾选；手动验收 9 步勾选；release checklist 满足。
```

---

## 10. 单步小任务模板（用于 P9 迭代闭环里的单字段修复）

```
任务：基于线上发现的 conversion signature <HASH> 修复字段映射。

步骤：
1. GET /admin/api/conversion/samples?signature=<HASH> 取样本元数据。
2. GET /admin/api/conversion/samples/{id}/body 下载脱敏 body。
3. 把样本固化到 tests/fixtures/regressions/<HASH>.json，新增 tests/converter_regressions.rs 用例（先红，断言期望输出）。
4. 在 docs/plans/2026-05-04-api-converter-plan/field-mapping.md 增补该字段映射，或将其从未知集合中迁出。
5. 修代码（converter/anthropic_to_openai.rs 或 openai_to_anthropic.rs 或 stream.rs）。
6. cargo test converter::regressions 转绿；cargo test 全套绿。
7. 提交：`fix(converter): handle <field> in <direction> (sig:<HASH>)`，PR 描述链到原 signature。
8. 部署后观察 24h，该 signature 不再产生新事件 → 视为修复完成。

不要做：
- 不要"顺手"扩大改动范围。
- 不要在不更新 field-mapping.md 的情况下改代码。
```

---

## 使用建议

- **顺序**：P1 → P2 → P3 → P4 → P5 → (P6 // P7 // P9 可并行三分支) → P8。
- **每段提示词独立可粘贴**：agent 不需要看上下文也能开工；但建议把 Master Prompt 作为 system 或第一条用户消息，再把阶段提示词作为后续消息追加。
- **DoD 检查**：每段末尾"验收"是 hard gate，未达成不开下一段。
- **偏差回写**：agent 若在执行中调整了设计，必须改对应 phase 文档而不是只改代码。
