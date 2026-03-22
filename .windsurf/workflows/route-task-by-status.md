---
description: 根据任务内容和当前实现状态判断其应进入 active-work、roadmap、reference 还是 completed，并给出下一步流程命令
---
# /route-task-by-status

目标：在不依赖人工先指定目录的情况下，根据任务本身的性质，自动判断文档和工作流应进入哪个状态目录。

执行步骤：

1. 先读取 `docs/README.md`，确认当前状态目录定义：
   - `active-work/`
   - `roadmap/`
   - `reference/`
   - `completed/`

2. 对输入任务进行四问判断：
   - 这是当前必须交付的吗？
   - 这是未来方向或高风险规划吗？
   - 这是系统现状或长期参考说明吗？
   - 这是已经做完、主要为了防止重复工作的内容吗？

3. 按以下规则归类：
   - **Active Work**：当前必须交付，且应直接指导实现
   - **Roadmap**：延期事项、高风险事项、未来能力、历史计划
   - **Reference**：已存在系统行为、架构说明、长期有效规格
   - **Completed**：当前任务已完成，文档主要用于记录已做内容和防止重复

4. 如果一个任务同时包含多种状态内容：
   - 不要强行放到一个目录
   - 必须拆分成多份文档或多层目录
   - 当前交付内容进入 `active-work/`
   - 未来部分进入 `roadmap/`
   - 已实现基线进入 `reference/`
   - 已完成结果进入 `completed/`

5. 给出下一步命令建议：
   - 如果归类为 Active -> 推荐 `/scope-active-task` 或 `/breakdown-active-work`
   - 如果归类为 Roadmap -> 推荐保留在 roadmap 或用 `/activate-roadmap-item`
   - 如果归类为 Completed -> 推荐 `/complete-and-archive-task`
   - 如果归类为 Reference -> 推荐更新参考索引，不进入实施待办

6. 最终输出必须包含：
   - 归类结果
   - 归类理由
   - 建议目录位置
   - 推荐下一条 workflow 命令
