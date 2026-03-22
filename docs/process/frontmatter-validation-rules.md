# Frontmatter 校验规则

## 目的

本文件定义 agent 在进行任务文档标准化时应默认执行的 frontmatter 校验规则。

## 校验目标

frontmatter 校验不是装饰性检查，而是任务状态识别的基础能力。

校验的目标是：
- 发现目录与状态不一致
- 发现阶段字段与任务真实阶段不一致
- 发现缺少下一步命令指引
- 发现 active 文档仍夹带 deferred 内容
- 发现 completed 文档仍包含未完成执行承诺

## 必查项

### 1. 目录与 `status` 一致性
- `docs/active-work/` 必须对应 `status: active`
- `docs/roadmap/` 必须对应 `status: deferred`
- `docs/reference/` 必须对应 `status: reference`
- `docs/completed/` 必须对应 `status: completed`

### 2. `workflow_stage` 合理性
常见组合：
- `active` + `scoped`
- `active` + `breakdown`
- `active` + `implementation`
- `active` + `sync`
- `active` + `review`
- `completed` + `archived`

### 3. `next_command` 合理性
例：
- 新需求文档不应直接推荐 `/complete-and-archive-task`
- 已完成文档不应继续推荐 `/breakdown-active-work`
- roadmap 文档通常应推荐 `/activate-roadmap-item`

### 4. 内容与状态一致性
- active 文档不应大量承载未来设计或长期优化
- roadmap 文档不应写成当前实现说明
- completed 文档不应仍保留开放式开发承诺
- reference 文档不应伪装成当前 backlog

## 自动修正 vs 人工确认

### 可自动修正
- 缺少 `last_reviewed`
- frontmatter 字段顺序不一致
- `next_command` 明显为空且可根据状态推断

### 应提示人工确认
- 目录与 `status` 冲突
- `workflow_stage` 与实际状态不匹配
- active 文档中混入 roadmap 内容
- completed 文档仍存在未完成项
- 文档位置本身需要迁移

## 审核输出格式

frontmatter 校验结果至少应分为：
- **Valid**：字段齐全且状态一致
- **Auto-fixable**：存在轻微问题，可自动修正
- **Needs Review**：存在状态冲突或内容冲突，需要人工判断
