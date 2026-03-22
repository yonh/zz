---
description: 按第一性原理收敛任务范围，并将内容拆分到 active-work、roadmap、reference、completed
---
# /scope-active-task

目标：把一个需求从“模糊大任务”收敛成“当前交付范围”，同时把未来目标、参考资料、已完成内容分流到正确目录。

执行步骤：

1. 读取 `docs/README.md`，确认当前文档状态结构：
   - `docs/active-work/`
   - `docs/roadmap/`
   - `docs/reference/`
   - `docs/completed/`

2. 读取目标任务目录及相关文档，优先检查：
   - `README.md`
   - `prd.md`
   - `tech-spec.md`
   - `implementation-plan.md`

3. 用第一性原理重述问题：
   - 用户当前真正卡住的是什么？
   - 哪部分直接影响当前交付？
   - 哪部分只是未来优化？
   - 哪部分只是参考背景？
   - 哪部分其实已经做完，不应重复？

4. 将内容强制拆分到四个桶：
   - **Active**：当前批次必须交付的内容
   - **Roadmap**：延期、高风险、未来方向
   - **Reference**：系统现状、长期参考规格
   - **Completed**：已完成且主要用于防止重复工作的内容

5. 对 Active 范围只保留：
   - 当前目标
   - 最小实现路径
   - 验收标准
   - 回归边界

6. 如果 Active 文档中混入以下内容，移出到 `docs/roadmap/`：
   - 配置 schema 扩展
   - 协议转换层
   - UI/统计增强
   - 长期性能和平台化目标

7. 更新相关 README 索引，让目录本身能一眼看出状态。

8. 输出最终结论时必须明确：
   - 当前交付范围
   - 明确延期范围
   - 不应重复做的已完成内容
   - 下一步最小开发批次
