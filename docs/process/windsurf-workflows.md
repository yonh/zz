# Windsurf Workflow 使用指南

## 目的

本指南说明在不同任务阶段应该使用哪个 Windsurf slash command。

## 核心命令

### 启动、归类与收敛
- `/start-new-task`
- `/route-task-by-status`
- `/scope-active-task`

适用场景：
- 新需求刚进入
- 需求还比较宽
- 任务还没有确定应该进入哪个状态目录

### 当前交付拆分与对齐
- `/breakdown-active-work`
- `/sync-active-work-with-code`

适用场景：
- 任务已经属于 `docs/active-work/`
- 需要把任务压缩成最小安全可执行批次
- 需要检查文档与代码是否仍然一致

### 从 roadmap 激活
- `/activate-roadmap-item`

适用场景：
- 一个延期事项现在决定开始做

### 审核与归档
- `/review-delivery-completion`
- `/complete-and-archive-task`

适用场景：
- 实现已经完成
- 需要独立确认需求是否真的完成
- 需要决定是否允许归档

## 隐式技能 vs 显式命令

以下能力应视为 agent 端默认卫生行为，而不是手工 slash command：
- 文档标准化检查
- 根据目录与 frontmatter 推断状态
- 检测 active 文档与实现之间的范围漂移

详见：
- `docs/process/document-frontmatter-standard.md`
- `docs/process/frontmatter-validation-rules.md`
- `docs/process/agent-skills.md`

## 推荐端到端流

```text
需求进入
-> /start-new-task
-> /route-task-by-status
-> /scope-active-task
-> /breakdown-active-work
-> 实现
-> /sync-active-work-with-code
-> /review-delivery-completion
-> /complete-and-archive-task
```

## 重要审核原则

写代码的人不应成为“是否完成”的唯一判断来源。

应使用 `/review-delivery-completion` 配合 `docs/process/delivery-review-checklist.md`，从独立审核视角检查：
- 需求是否达成
- 边界情况是否合理
- 回归风险是否存在
- 是否有异常或未完成行为
- 是否错误混入了 roadmap 内容

## 模板入口

新建或重写任务文档时，优先从以下模板开始：
- `docs/process/templates/active-task-template.md`
- `docs/process/templates/roadmap-item-template.md`
- `docs/process/templates/completed-task-template.md`

## 目录识别规则

workflow 与 agent 应把目录位置视为强信号：
- `docs/active-work/` -> 当前执行中
- `docs/roadmap/` -> 延期或未来工作
- `docs/reference/` -> 系统知识，不是 backlog
- `docs/completed/` -> 归档与防重复记录
