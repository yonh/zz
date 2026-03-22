---
description: 快速开始一个新需求，生成初始任务交互、建议目录位置与下一步 workflow
---
# /start-new-task

目标：为一个新需求提供最快的起步路径，减少“我现在应该先做什么”的阻力。

执行步骤：

1. 先读取 `docs/README.md` 与 `docs/process/quick-start.md`，确认当前仓库的状态化文档模型。

2. 将输入需求重述为一个最小问题陈述：
   - 当前想解决什么问题？
   - 这是否影响当前交付？
   - 它更像当前任务、未来计划、系统参考，还是已完成事项？

3. 给出初始归类建议：
   - `docs/active-work/`
   - `docs/roadmap/`
   - `docs/reference/`
   - `docs/completed/`

4. 如果看起来属于当前任务：
   - 推荐下一步使用 `/route-task-by-status`
   - 再使用 `/scope-active-task`
   - 必要时按模板生成新的 active 任务骨架

5. 如果看起来属于未来事项：
   - 推荐放入 roadmap
   - 不要直接进入当前实现

6. 如果看起来是系统参考：
   - 推荐更新 `docs/reference/`
   - 不要混入当前 backlog

7. 最终输出必须包含：
   - 任务一句话定义
   - 建议目录位置
   - 建议使用的模板
   - 下一条 workflow 命令
