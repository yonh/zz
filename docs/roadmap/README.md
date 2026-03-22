# 路线图

## 状态
Deferred / Future

该目录包含延期事项、未来方向和历史规划材料。

## 进入规则
一个任务属于这里，当且仅当：
- 它不是当前交付所必需
- 它对当前批次来说过大、过宽或风险过高
- 它需要后续验证或二次优先级判断
- 它是当前任务完成后的后续项

## 推荐命令
- `/route-task-by-status` - 确认该任务是否应继续留在 roadmap
- `/activate-roadmap-item` - 将 roadmap 中的事项提升为当前工作
- `/scope-active-task` - 激活后将其重写为当前可交付范围

## 重要规则
不要从 `active-work/` 中偷偷实现 roadmap 内容。
如果某个 roadmap 项变得必要，必须先显式激活并重写成新的 active-work 文档。

## 当前入口
- [codex-responses/](./codex-responses/) - Codex Responses 的后续规划
- [code-remediation/](./code-remediation/) - 已完成整改任务拆出的后续事项
- [historical-plans/](./historical-plans/) - 历史规划批次
