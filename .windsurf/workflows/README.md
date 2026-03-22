# Windsurf Workflow 索引

该目录包含与仓库状态化文档体系配套的 workflow 命令，用于管理任务范围、实施流程、审核与归档。

## 推荐命令顺序

### 1. 启动、归类与收敛
- `/start-new-task`
- `/route-task-by-status`
- `/scope-active-task`

### 2. 当前交付执行
- `/breakdown-active-work`
- `/sync-active-work-with-code`

### 3. 激活延期事项
- `/activate-roadmap-item`

### 4. 审核与关闭
- `/review-delivery-completion`
- `/complete-and-archive-task`

## 隐式 Agent 技能

以下能力应被视为自动执行的 agent 侧行为，而不是手工 workflow 命令：
- 标准化任务文档
- 根据目录与 frontmatter 推断状态
- 检测 active 文档与代码之间的范围漂移

详见：
- `docs/process/document-frontmatter-standard.md`
- `docs/process/frontmatter-validation-rules.md`
- `docs/process/agent-skills.md`

## 配套流程文档
- `docs/process/task-lifecycle.md`
- `docs/process/windsurf-workflows.md`
- `docs/process/delivery-review-checklist.md`
- `docs/process/quick-start.md`
- `docs/process/templates/`

## 状态目录模型
- `docs/active-work/` - 当前允许驱动实现的文档
- `docs/roadmap/` - 延期和未来工作
- `docs/reference/` - 基线或已实现参考文档
- `docs/completed/` - 已完成任务归档

## 重要规则
workflow 应把文档位置视为强状态信号。如果一个主题跨越多个状态，就拆成多份文档，而不是让一个目录承担混合语义。
