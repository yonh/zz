---
status: deferred
horizon: medium_term
workflow_stage: scoped
next_command: /activate-roadmap-item
last_reviewed: 2026-03-22
---

# 延期事项 - Path-Based Routing

## 状态
Deferred

## 目的
允许路由决策同时依赖请求 path 与 model pattern。

示例：
- 将 `/v1/responses` 流量路由到 OpenAI providers
- 将 `/v1/chat/completions` 流量路由到 OpenAI-compatible Chat providers

## 为什么延期
当前 ZZ 路由主要按 model 运作。对于当前 Codex 用例，model-based routing 往往已经足够。引入 path-based routing 会带来配置 schema 变化与更广的测试要求，但并不是当前首批交付必需的。

## 必要变更
- 扩展 `src/router.rs` 中的 `ModelRule`
- 扩展 `src/config.rs` 中的 `ModelRuleConfig`
- 更新配置示例与校验逻辑
- 如果 dashboard 可编辑规则，还需要回看 admin API payload

## 风险
- 未来态与当前态配置示例混淆
- 路由优先级规则更复杂
- 扩大路由行为的回归面

## 开始前置条件
- 已有证据表明 model-based routing 无法满足混合工具场景
- 已明确 path 匹配与 model 匹配的优先级策略
- 配置示例与当前 parser 行为保持一致
