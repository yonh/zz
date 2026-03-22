# 当前工作

## 状态
Active

该目录只包含**允许直接驱动当前实现**的任务文档。

## 进入规则
一个任务只有在满足以下条件时才应进入这里：
- 它属于当前交付批次
- 它有清晰验收标准
- 它应当立即影响当前编码决策

## 推荐命令
- `/start-new-task` - 启动一个新需求的初始交互与文档落点
- `/route-task-by-status` - 确认该任务确实属于当前工作
- `/scope-active-task` - 收敛当前交付范围
- `/breakdown-active-work` - 拆成最小可执行批次
- `/sync-active-work-with-code` - 检查文档与代码是否仍然一致
- `/review-delivery-completion` - 在实现完成后做独立审核

## 退出规则
任务完成后不要继续留在这里。
应使用 `/complete-and-archive-task` 将已完成内容移动到 `docs/completed/`，并把后续事项保留到 `docs/roadmap/`。

## 当前入口
- [worklist.md](./worklist.md) - 当前工作列表
- [codex-responses/](./codex-responses/) - OpenAI Responses API 支持的当前交付批次
