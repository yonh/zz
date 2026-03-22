---
description: 在任务完成后更新文档状态，将其归档到 completed，并保留未完成后续项到 roadmap
---
# /complete-and-archive-task

目标：当一个 active-work 任务完成后，及时归档，避免未来重复设计或重复实现。

执行步骤：

1. 读取目标任务目录与实现结果，确认：
   - 当前交付范围是否已经完成
   - 验收标准是否满足
   - 测试或验证是否完成

2. 将“已完成的当前任务文档”迁移到：
   - `docs/completed/<task>/`

3. 不要把所有内容一起搬走。先区分：
   - 已完成部分 -> `completed/`
   - 未完成但仍有价值的后续项 -> `roadmap/`
   - 长期参考规格 -> `reference/`（如适用）

4. 更新文档状态标记：
   - `Status: Completed`
   - 完成日期
   - 完成范围
   - 未完成后续项链接

5. 更新索引：
   - `docs/README.md`
   - `docs/active-work/README.md`
   - `docs/roadmap/README.md`
   - `docs/completed/README.md`

6. 如果当前任务已无活动内容：
   - 从 `active-work/` 中移除该任务入口
   - 在 `completed/` 中增加防重复说明

7. 最终输出：
   - 已完成内容
   - 被归档的位置
   - 留在 roadmap 的后续事项
   - 后续是否还需要新一轮 active-work
