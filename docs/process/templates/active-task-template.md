# Active 任务模板

## README.md 模板

```md
---
status: active
horizon: current
workflow_stage: scoped
next_command: /breakdown-active-work
last_reviewed: YYYY-MM-DD
---

# <任务名称>

## 目标
<一句话说明当前交付目标>

## 当前交付范围
- <当前项 1>
- <当前项 2>

## 明确不做
- <未来项 1>
- <未来项 2>

## 文档
- [prd.md](./prd.md)
- [tech-spec.md](./tech-spec.md)
- [implementation-plan.md](./implementation-plan.md)
```

## prd.md 模板

```md
---
status: active
horizon: current
workflow_stage: scoped
next_command: /breakdown-active-work
last_reviewed: YYYY-MM-DD
---

# <任务名称> PRD

## 问题陈述
<当前真正卡住的问题>

## 当前交付目标
<这一批必须做到什么>

## 范围内
- <项>
- <项>

## 范围外
- <项>
- <项>

## 成功标准
- [ ] <标准>
- [ ] <标准>
```

## tech-spec.md 模板

```md
---
status: active
horizon: current
workflow_stage: breakdown
next_command: /sync-active-work-with-code
last_reviewed: YYYY-MM-DD
---

# <任务名称> 技术规格

## 技术原则
<必须维持的架构规则>

## 当前代码现实
- <已具备>
- <缺失项>

## 必要变更
- <文件与变更>
- <文件与变更>

## 验证计划
- <单测>
- <手工验证>
```

## implementation-plan.md 模板

```md
---
status: active
horizon: current
workflow_stage: breakdown
next_command: /sync-active-work-with-code
last_reviewed: YYYY-MM-DD
---

# <任务名称> 实施计划

## 交付目标
<当前批次目标>

## 工作拆分
### 任务 1
- 目标
- 文件
- 验收

### 任务 2
- 目标
- 文件
- 验收

## 顺序
1. <步骤>
2. <步骤>

## 质量关口
- <关口>
- <关口>
```

## 重要规则
active 模板只描述当前交付批次。任何延期、高风险或可选内容都应进入 `docs/roadmap/`。
