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
