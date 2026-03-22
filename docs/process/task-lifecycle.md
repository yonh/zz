# 任务生命周期

## 目的

本文件定义任务应如何在仓库的状态化文档目录之间流转。

## 状态模型

### `docs/active-work/`
只允许放置可以直接驱动当前实现的文档。

进入条件：
- 它属于当前交付批次
- 它有明确验收标准
- 它的实现范围是收敛的
- 它应立即影响当前开发决策

### `docs/roadmap/`
用于延期事项、未来方向、高风险设计和历史规划材料。

进入条件：
- 它不是当前交付必须完成的内容
- 它依赖后续验证或后续优先级判断
- 它对于当前批次来说过大、过宽或风险过高
- 它是当前批次完成后的后续项

### `docs/reference/`
用于系统说明、基线架构和长期参考规格。

进入条件：
- 它描述当前已实现行为
- 它会被多个任务长期参考
- 它不应被当作当前 backlog 直接执行

### `docs/completed/`
用于已完成任务的归档文档，主要用于防止重复工作。

进入条件：
- 当前验收标准已满足
- 当前 active 批次已关闭
- 后续项已按需拆分到 roadmap

## 允许的迁移

```text
新需求
-> /start-new-task
-> /route-task-by-status
-> active-work | roadmap | reference | completed

roadmap
-> /activate-roadmap-item
-> active-work

active-work
-> /complete-and-archive-task
-> completed

active-work
-> 拆出后续项
-> roadmap
```

## 强规则

1. 不要在同一份 active 文档里混入当前交付和未来设计。
2. 不要从 active-work 中默默实现 roadmap 内容。
3. 当前任务关闭后，不要把其文档继续留在 active-work。
4. 如果一个主题横跨多个状态，应拆文档，而不是让一个目录承担混合语义。

## 评审关口

实现前：
- 使用 `/start-new-task`（新需求时）
- 使用 `/scope-active-task`
- 使用 `/breakdown-active-work`

实现中：
- 使用 `/sync-active-work-with-code`

实现后：
- 使用 `/review-delivery-completion`
- 使用 `/complete-and-archive-task`
