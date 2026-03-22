# 文档 Frontmatter 标准

## 目的

本文件定义任务文档的统一 frontmatter 字段，使任务状态可以被人和 agent 一致识别。

## 为什么需要它

目录位置是第一层状态信号。
frontmatter 是第二层状态信号。

两者结合后，可以更稳定地做到：
- 当前任务更容易识别
- 延期任务不被误当作当前交付
- 已完成文档更容易归档和追溯
- agent 可以默认执行文档标准化，而不是每次都依赖人工解释

## 必填字段

```yaml
---
status: active | deferred | reference | completed
horizon: current | short_term | medium_term | long_term
workflow_stage: scoped | breakdown | implementation | sync | review | archived
next_command: /scope-active-task
owner: <optional>
last_reviewed: YYYY-MM-DD
---
```

## 字段说明

### `status`
文档当前状态。

允许值：
- `active`
- `deferred`
- `reference`
- `completed`

### `horizon`
规划时间尺度。

允许值：
- `current`
- `short_term`
- `medium_term`
- `long_term`

### `workflow_stage`
当前流程阶段。

允许值：
- `scoped`
- `breakdown`
- `implementation`
- `sync`
- `review`
- `archived`

### `next_command`
下一条推荐的 Windsurf slash command。

示例：
- `/start-new-task`
- `/scope-active-task`
- `/breakdown-active-work`
- `/sync-active-work-with-code`
- `/review-delivery-completion`
- `/complete-and-archive-task`

### `owner`
可选。用于团队协作中的责任人标记。

### `last_reviewed`
最近一次主动校对该文档准确性的日期。

## 目录与状态对齐规则

默认应满足：
- `docs/active-work/` -> `status: active`
- `docs/roadmap/` -> `status: deferred`
- `docs/reference/` -> `status: reference`
- `docs/completed/` -> `status: completed`

如果目录和 frontmatter 不一致，必须先修正，再继续任务流转。

## 标准化判定规则

一份任务文档被视为“已标准化”，当且仅当：
- 目录与 `status` 一致
- `workflow_stage` 与任务真实阶段一致
- `next_command` 与当前状态匹配
- active 文档中没有明显未来内容混入
- completed 文档中不再包含开放执行承诺

## 示例：当前任务

```yaml
---
status: active
horizon: current
workflow_stage: breakdown
next_command: /breakdown-active-work
last_reviewed: 2026-03-22
---
```

## 示例：路线图事项

```yaml
---
status: deferred
horizon: medium_term
workflow_stage: scoped
next_command: /activate-roadmap-item
last_reviewed: 2026-03-22
---
```

## 示例：已完成任务

```yaml
---
status: completed
horizon: short_term
workflow_stage: archived
next_command: /complete-and-archive-task
last_reviewed: 2026-03-22
---
```
