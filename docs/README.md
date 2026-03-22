# ZZ 文档总览

该目录按**工作状态**组织，这样你可以一眼看出哪些内容是当前工作、未来计划、系统参考或已完成归档。

## 状态目录

### [active-work/](./active-work/)
当前正在设计或实现、允许直接驱动开发的任务文档。

- `worklist.md` - 当前工作列表与推荐下一步命令
- `codex-responses/` - OpenAI Responses API 支持的当前交付文档

### [roadmap/](./roadmap/)
延期事项、未来方向和历史计划。

- `codex-responses/` - Codex Responses 任务的后续规划
- `code-remediation/` - 已完成整改任务拆出的后续事项
- `historical-plans/` - 旧规划批次与历史记录

### [reference/](./reference/)
系统基线、已实现行为和长期参考规格。

- `core-proxy/` - 核心代理架构
- `admin-api/` - 管理 API 与 WebSocket 合约
- `web-ui/` - 控制台 UI 架构
- `tokens-system/` - Token 统计系统
- `integration/` - 跨组件集成规格

### [completed/](./completed/)
已完成任务的归档文档，主要用于避免未来重复设计或重复实现。

- `backend-fix/` - 已完成的后端修复
- `code-remediation/` - 已完成的代码整改批次

### [process/](./process/)
跨任务流程文档，说明任务如何流转、如何使用 Windsurf workflow、如何做审核与归档。

- `task-lifecycle.md` - 任务状态与允许迁移关系
- `windsurf-workflows.md` - 推荐的 slash command 使用顺序
- `delivery-review-checklist.md` - 交付审核清单
- `document-frontmatter-standard.md` - 文档头标准
- `frontmatter-validation-rules.md` - 文档头校验规则
- `agent-skills.md` - 隐式技能与显式 workflow 的边界
- `quick-start.md` - 如何快速开始一个新需求
- `templates/` - 可复用任务模板

## 工作流驱动的状态流转

```text
新需求
-> /start-new-task
-> /route-task-by-status
-> /scope-active-task
-> active-work/<task>/
-> /breakdown-active-work
-> 实现
-> /sync-active-work-with-code
-> /review-delivery-completion
-> /complete-and-archive-task

roadmap 中的延期事项
-> /activate-roadmap-item
-> active-work/<task>/
```

## 推荐命令

### 任务启动与归类
- `/start-new-task` - 快速启动一个新需求，生成初始交互与文档落点
- `/route-task-by-status` - 判断任务应进入 `active-work`、`roadmap`、`reference` 还是 `completed`
- `/scope-active-task` - 将宽泛任务收敛到当前交付范围

### 当前交付执行
- `/breakdown-active-work` - 将当前任务拆成最小可执行批次
- `/sync-active-work-with-code` - 检查 active-work 文档与代码现实是否一致

### 审核与归档
- `/review-delivery-completion` - 从独立审核视角检查需求完成情况、异常问题与范围漂移
- `/complete-and-archive-task` - 完成后归档当前任务，并将后续项保留到 roadmap

## 隐式 Agent 能力

以下能力应该由 agent 默认执行，而不是依赖你手动触发：

- 任务文档标准化检查
- 目录状态与 frontmatter 状态识别
- active-work 与代码之间的范围漂移检测
- 当前文档中不存在字段、文件、配置项的识别

详见：
- `docs/process/document-frontmatter-standard.md`
- `docs/process/frontmatter-validation-rules.md`
- `docs/process/agent-skills.md`

## 如果你想看当前有什么工作计划

优先查看：
1. `docs/active-work/worklist.md`
2. `docs/active-work/<task>/README.md`
3. `docs/active-work/<task>/implementation-plan.md`

## 阅读顺序

1. 先看 `active-work/`
2. 再看 `roadmap/`
3. 需要了解系统现状时看 `reference/`
4. 避免重复工作时看 `completed/`
5. 不确定流程或命令时看 `process/`
