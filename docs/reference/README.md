# 参考文档

## 状态
Reference / Baseline

该目录包含系统基线、已实现行为和长期参考规格。

## 进入规则
一个文档属于这里，当它：
- 描述已实现或基线系统行为
- 会被多个任务复用
- 不应直接作为当前 backlog 执行项

## 推荐命令
- `/route-task-by-status` - 判断一个文档是否应继续保留为 reference，还是需要拆分
- `/sync-active-work-with-code` - 当 active 任务依赖 reference 文档时，用来核对 reference 是否仍然贴合代码现实

## 重要规则
reference 文档可以影响理解，但不应默默变成当前交付承诺。
如果当前任务要依赖它，应该在 `docs/active-work/` 中创建或更新任务文档。

## 当前入口
- [core-proxy/](./core-proxy/)
- [admin-api/](./admin-api/)
- [web-ui/](./web-ui/)
- [tokens-system/](./tokens-system/)
- [integration/](./integration/)
