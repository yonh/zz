# 当前工作列表

## 目的

快速回答两个问题：
- 现在有哪些任务正在进行？
- 我下一步应该用哪个命令？

## 当前活跃任务

### Codex Responses API 支持
- **位置**：`docs/active-work/codex-responses/`
- **状态**：Active
- **目标**：让 Codex 通过 ZZ 稳定使用 OpenAI Responses API
- **当前范围**：
  - streaming 检测修复
  - `api_type` 日志字段
  - 非流式 token usage 验证
  - OpenAI 端到端验证
- **下一步命令**：`/breakdown-active-work`
- **实现过程中建议命令**：`/sync-active-work-with-code`
- **完成后建议命令**：`/review-delivery-completion`

### 运行时 Provider 调度控制
- **位置**：`docs/active-work/runtime-provider-control/`
- **状态**：Active
- **目标**：运行时动态控制 provider 启停与模型级别固定调度
- **当前范围**：
  - Spec 01：修复 Routing 页面 toggle 不调 API 的 bug + config 支持 enabled 字段
  - Spec 02：Model Pinning API 及路由集成（按模型固定到指定 provider）
- **实施顺序**：先 Spec 01，再 Spec 02
- **下一步命令**：`/breakdown-active-work`

### LLM 请求全量日志
- **位置**：`docs/active-work/llm-request-journal/`
- **状态**：Active
- **目标**：查看 Claude / Codex / Cursor 等客户端实际发送到 ZZ 的完整 LLM 请求日志
- **当前范围**：
  - Spec 01：后端全量捕获并持久化请求日志（含 headers / body / client 标识）
  - Spec 02：日志查询 API、详情查看、导出与 UI 浏览器
- **关键要求**：不是现有 metadata logs，而是可用于排查 `thinking_budget` 等参数问题的原始请求日志
- **下一步命令**：`/breakdown-active-work`

## 使用方式

### 我想看当前计划
按顺序查看：
1. 本文件
2. 对应任务目录的 `README.md`
3. 对应任务目录的 `implementation-plan.md`

### 我想开始一个新需求
优先使用：
1. `/start-new-task`
2. `/route-task-by-status`
3. `/scope-active-task`

### 我想确认有没有遗漏工作
使用：
- `/sync-active-work-with-code`
- `/review-delivery-completion`
