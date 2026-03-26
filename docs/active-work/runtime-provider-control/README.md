---
status: active
horizon: current
workflow_stage: breakdown
last_reviewed: 2026-03-26
---

# Runtime Provider Control - 当前工作

## 当前交付范围

该目录包含运行时 Provider 调度控制的当前交付批次。

## 当前目标

让用户能在 ZZ 运行过程中动态控制 provider 调度行为，无需重启。

## 文档

- [spec-01-provider-enable-disable.md](./spec-01-provider-enable-disable.md) - Provider 运行时启停
- [spec-02-model-provider-pinning.md](./spec-02-model-provider-pinning.md) - Model 级别 Provider 固定调度

## 当前范围内

- 修复 Routing 页面 toggle 不调用后端 API 的问题
- 配置文件支持 `enabled` 字段
- Model Pinning API 及路由集成

## 明确不做

- 配置文件持久化运行时状态
- Model Pinning UI（后续批次）
- glob/通配符 model pinning（使用现有 model rules）

## 实施顺序

1. 先实施 Spec 01（Provider 启停），因为 Spec 02 依赖 provider 的 enabled 状态正确工作
2. 再实施 Spec 02（Model Pinning）

