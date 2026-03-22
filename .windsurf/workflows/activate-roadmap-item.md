---
description: 将 roadmap 中的某个延期事项提升为新的 active-work 当前任务，并重写为当前可交付范围
---
# /activate-roadmap-item

目标：把 `docs/roadmap/` 中的一个未来事项，正式提升为当前工作，并改写成适合执行的 active-work 文档。

执行步骤：

1. 读取用户指定的 roadmap 文档或目录。

2. 判断该事项是否满足激活条件：
   - 用户明确要求开始做
   - 当前问题已具备实现前提
   - 依赖关系清晰
   - 风险可控

3. 不要直接把 roadmap 文档原样搬到 active-work。必须重写为：
   - 当前目标
   - 最小交付范围
   - 当前代码触点
   - 当前验收标准
   - 明确排除项

4. 在 `docs/active-work/<task>/` 中创建或更新：
   - `README.md`
   - `prd.md`
   - `tech-spec.md`
   - `implementation-plan.md`

5. 在原 roadmap 文档中保留清晰状态说明，例如：
   - 已被激活
   - 哪部分仍然延期
   - 哪部分仍保留为长期目标

6. 更新顶层索引：
   - `docs/README.md`
   - `docs/active-work/README.md`
   - `docs/roadmap/README.md`

7. 最终输出：
   - 新 active-work 的范围
   - 仍留在 roadmap 的部分
   - 第一批实际开发工作
