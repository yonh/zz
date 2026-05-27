# Phase P8 — 测试矩阵 + 手动验收 + 文档

**Depends on:** P5, P6, P7
**Type:** verification
**Goal:** 全量回归 + 手动验收 + 对外文档，达到可发布门槛。

---

## 1. 自动化测试矩阵

| 维度 | 用例 |
|---|---|
| `/v1/*` 透明回归 | Claude 原生 / OpenAI 原生 各 1 例（请求/响应字节级一致） |
| 请求转换（a2o） | text、含 image、含 tool_use、含 tool_result、tools 重排、tool_choice 四态、未知字段 skip |
| 请求错误（a2o） | invalid_json、missing messages、bad type、unsupported_block |
| 响应转换（o2a 非流） | 纯文本、tool_calls 合法、tool_calls 非法 args、length、错误响应体映射 |
| 响应错误（o2a 非流） | invalid_json、missing choices、missing message |
| 流式 a2o | 文本流、单 tool_call、文本+tool_call 混合、半包跨 chunk、上游中途断流 |
| 流式 o2a | 镜像 4 例 |
| 降级请求侧 | 强制 invalid_json → 502 + Anthropic 错误体 + 头 |
| 降级响应侧 | 强制 missing choices → 透传 + 失败头 |
| 降级流式 | 中途状态机异常 → 优雅收尾 + message_stop |
| `enable_conversion_fallback=false` | 响应侧错误 → 502；流式错误 → reset |
| 配置默认值 | 旧 config.toml 加载零变更 |
| Provider 过滤 | 无匹配 target api_type → 502 + `no_matching_provider_for_target_api` |
| 截断 | UTF-8 边界（含 emoji） |
| 日志快照 | success / fallback / field_skipped |

**命令：**
```
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
```

## 2. 手动验收脚本

写入 `docs/active-work/api-converter/manual-acceptance.md`，并附 curl 命令：

1. 配置 OpenAI 类型 provider（如 SenseNova），`api_type="openai-chat"`。启动 ZZ。
2. **a2o 非流**：用 Anthropic schema 请求 `/a2o/v1/messages`，断言响应是 Anthropic schema、`X-Conversion-Status: success`。
3. **a2o 流式**：同上加 `stream:true`，断言收到 `message_start`/`content_block_*`/`message_delta`/`message_stop`。
4. **跳过字段**：请求体含 `top_k`/`anthropic_beta`/未知字段，断言响应正常 + 日志含 `field_skipped`。
5. **请求侧降级**：发非法 JSON → 502 + Anthropic 错误体 + `X-Conversion-Phase: request`。
6. **响应侧降级**：mock 上游返回缺 `choices` 的 JSON → 透传 + `X-Conversion-Status: failed` + `X-Conversion-Phase: response`。
7. **`/v1/*` 回归**：Claude 客户端走 `/v1/messages` 配 Anthropic provider，行为与升级前一致。
8. **o2a 反向**：OpenAI 客户端走 `/o2a/v1/chat/completions` + Anthropic provider，验证四步（非流、流、跳过、降级）。
9. **配置兼容**：用未升级前的 `config.toml` 启动，无报错、所有 `/v1/*` 行为不变。

每步在文档中留 ☐ 复选框，验收人勾选并签注日期。

## 3. 文档产出

- `docs/dev/api-converter.md`（开发者指南）：
  - 架构图（路由 → handler → converter → provider）。
  - 路由前缀语义表（引用 `route-matrix.md`）。
  - 字段映射摘要 + 链接到 `field-mapping.md`。
  - 错误模型与降级（链接 `error-model.md`）。
  - **扩展指南**：新增前缀的 7 步流程（已在 `route-matrix.md` §7）。
- `README.md` 增加「协议转换」章节，列出当前支持的前缀与未来计划。
- `CHANGELOG`（如项目维护）：增加新功能条目。

## Acceptance Criteria

- 全部自动化测试在 CI（或本地）通过。
- 手动验收 9 步全部勾选。
- 文档评审：`docs/dev/api-converter.md` 与 README 章节合并。
- 在标准开发机上 `cargo build --release` 成功，二进制可启动并通过手动验收 1-3 步冒烟。

## Release Checklist

- [ ] 所有 P0–P7 acceptance 条目已满足。
- [ ] 手动验收文档勾选完毕并归档。
- [ ] 新增 `X-Conversion-*` 响应头在文档中注明。
- [ ] `config.toml.example` 已更新。
- [ ] 在 `docs/completed/` 归档本计划目录（执行 `/complete-and-archive-task`）。
