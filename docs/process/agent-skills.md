# Agent 隐式技能与显式 Workflow 边界

## 目的

本文件说明哪些能力应被视为**隐式 agent 技能**，哪些能力应保留为**显式 Windsurf workflow**。

## 基本原则

以下情况应使用显式 workflow：
- 用户需要主动触发状态切换
- 该动作会改变任务范围或任务状态
- 该动作会做归档或审核结论

以下情况应作为隐式 agent 技能：
- 该检查在正常工作中应默认发生
- 如果要求用户每次手动触发，会造成重复负担
- 它本质上是安全、质量或一致性护栏

## 隐式技能

### `normalize-task-docs`
应作为始终开启的 agent 行为，而不是手工 slash command。

预期行为：
- 把目录位置视为状态信号
- 把 frontmatter 视为第二状态信号
- 检查目录与 frontmatter 是否冲突
- 检查 active 文档是否混入 deferred 内容
- 检查文档中是否引用不存在的文件、字段、配置或能力
- 在继续重大流程前先警告或修正文档结构问题

### `status-signal-reader`
也应作为隐式能力。

预期行为：
- 从文档位置推断任务大概率属于哪个状态
- 使用 `docs/README.md` 与 `docs/process/task-lifecycle.md` 作为状态判定规则
- 尽量避免在仓库已经有明确信号时仍让用户反复说明状态

### `scope-drift-detector`
应在实现与审核过程中默认启用。

预期行为：
- 对比 active 文档与实际代码变更
- 发现 roadmap 内容是否泄漏到当前实现
- 发现实现是否已经偏离当前 active 范围

## 显式 Workflow

以下命令应继续作为人工显式触发的流程命令：
- `/start-new-task`
- `/route-task-by-status`
- `/scope-active-task`
- `/breakdown-active-work`
- `/activate-roadmap-item`
- `/sync-active-work-with-code`
- `/review-delivery-completion`
- `/complete-and-archive-task`

## 工作规则

agent 应把文档标准化和状态识别视为默认卫生能力，而不是可选动作。

workflow 应保留给真正有意义的状态切换与审核关口。
