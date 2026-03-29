---
  status: active
  horizon: current
  workflow_stage: implementation
  feature: llm-request-journal-remediation
  last_reviewed: 2026-03-26
  ---

  # Spec 03: LLM Request Journal 偏差修复与实现闭环

  ## 1. 背景与问题

  现有实现未满足以下目标：

  1) Request Journal 应记录“实际发送到上游 LLM 的完整请求诊断信息”
  2) 应支持可验证的持久化与查询能力
  3) 应提供独立 UI 查看器，不与 metadata Logs 混用
  4) 当前 Request Journal 无可用内容，排障价值为 0

  本 spec 用于指导后续 agent 做“对齐 spec-01/spec-02 的收敛实现”。

  ## 2. 目标（必须达成）

  - 端到端闭环：`proxy 捕获 -> journal 落盘 -> admin API 查询 -> UI 展示`
  - 默认关闭、显式开启：高敏感日志能力受配置开关控制
  - 能直接定位参数问题：在 UI/API 中能看到 request body 是否包含 `thinking_budget` 等字段
  - 敏感 header 脱敏：`authorization`、`x-api-key`、`cookie`、`set-cookie` 默认脱敏
  - 失败请求不丢失：upstream 失败、provider unavailable 仍写入日志

  ## 3. 与现有实现差异（Gap Checklist）

  ### 3.1 采集链路差异
  - [ ] 确认在 `src/proxy.rs` 的真实转发路径中写 journal（不是旁路或仅部分分支）
  - [ ] 成功与失败分支都写入
  - [ ] failover 场景补全 `failover_chain`

  ### 3.2 存储与可见性差异
  - [ ] 实际写入 `observability.request_journal.storage_dir`
  - [ ] 按日期目录落盘（`YYYY-MM-DD/*.json`）
  - [ ] 写入失败有明确 tracing 错误日志
  - [ ] 提供最小可观测手段：启动后可验证目录创建与文件增长

  ### 3.3 数据结构差异
  - [ ] `RequestJournalEntry` 字段与 spec-01 对齐（id/timestamp/client/provider/upstream/
  status/body/headers 等）
  - [ ] UTF-8 文本进 `request_body_text`，二进制回退 `request_body_base64`
  - [ ] `request_headers` 为脱敏后结果，不输出明文 secret

  ### 3.4 API 差异
  - [ ] `GET /zz/api/request-journal`（分页+过滤）
  - [ ] `GET /zz/api/request-journal/{id}`（完整详情）
  - [ ] `GET /zz/api/request-journal/export`（JSON/JSONL）
  - [ ] 与 `/zz/api/logs` 职责分离，不混用响应模型

  ### 3.5 UI 差异
  - [ ] 新增独立页面 `Request Journal`
  - [ ] 支持过滤：client/provider/model/status/path/date
  - [ ] 列表+详情布局，详情包含 headers/body/upstream/failover/error
  - [ ] body 支持 copy、download、JSON prettify

  ## 4. 统一实现规范（Implementation Contract）

  ### 4.1 配置规范
  在 `src/config.rs` 保证存在并生效：

  ```toml
  [observability.request_journal]
  enabled = false
  storage_dir = "logs/request-journal"
  retention_days = 7
  redact_headers = ["authorization", "x-api-key", "cookie", "set-cookie"]

  要求：

  - enabled=false 时不写盘
  - 配置热更新若已支持，应保证 journal writer 能同步新配置；若未支持，至少重启生效并文档说明

  ### 4.2 写入时机规范

  - 请求体读取完成后构建初始 entry
  - provider/upstream 决策后补充目标信息
  - 响应返回或失败后补全 status/response_bytes/error/failover_chain
  - 每个请求最终最多写 1 条完整 entry（避免重复/碎片）

  ### 4.3 脱敏规范

  - 大小写不敏感匹配 header 名
  - 命中规则值统一替换为 "[REDACTED]"
  - 不可通过 UI/API 获取原始 secret header

  ### 4.4 查询规范

  - 列表接口返回 summary，不返回 body（减载）
  - 详情接口返回完整 entry
  - 过滤行为一致且可组合（AND 语义）
  - 分页参数异常时返回 400（不要 silent fallback）

  ## 5. 验收标准（Blocking）

  ### 5.1 功能验收

  - [ ] 开启配置后，任意一次代理请求都会生成 journal 文件
  - [ ] 文件内可见请求 body 且可确认 thinking_budget 等字段
  - [ ] 失败请求也写入且带 error/status
  - [ ] API 列表/详情/导出可用
  - [ ] UI 可筛选并查看详情

  ### 5.2 安全验收

  - [ ] secret headers 全部脱敏
  - [ ] UI 与导出内容均不泄露明文 key/cookie

  ### 5.3 回归验收

  - [ ] cargo test 通过
  - [ ] cargo clippy 通过
  - [ ] 前端构建通过（如 pnpm build 或项目既有命令）

  ## 6. 建议任务拆分（给实现 agent）

  1. 修复后端采集闭环（src/proxy.rs, src/request_journal.rs）
  2. 修复 admin API（src/admin_api.rs）
  3. 落地独立 UI 页面与路由（ui/src/pages/RequestJournal.tsx 等）
  4. 增加最小测试（后端单元+API 过滤用例）
  5. 联调验证并提交验收记录（示例请求、落盘文件、API 响应截图/片段）

  ## 7. 交付物要求

  - 代码变更清单（按文件）
  - 配置样例与开启说明
  - 验收证据：
      - journal 文件样例（脱敏后）
      - GET /zz/api/request-journal 返回样例
      - GET /zz/api/request-journal/{id} 返回样例
      - UI 查看 thinking_budget 的截图或文本证据
  - 未完成项与风险清单（如 retention 清理策略未实现）

  ## 8. 非目标

  - 不做请求回放（replay）
  - 不做全量 SSE 响应体持久化
  - 不做跨天海量全文检索引擎
