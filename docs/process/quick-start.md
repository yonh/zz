# 快速开始一个新需求

## 目的

本指南用于回答两个高频问题：
- 我现在想看有哪些工作计划，该看哪里？
- 我现在想快速开始一个新需求，应该怎么做？

## 一、如果你想看当前有什么工作计划

按顺序查看：
1. `docs/active-work/worklist.md`
2. 对应任务目录的 `README.md`
3. 对应任务目录的 `implementation-plan.md`

如果你只是想看未来可能会做什么，再查看：
1. `docs/roadmap/README.md`
2. `docs/roadmap/<task>/README.md`

## 二、如果你想快速开始一个新需求

推荐流程：

```text
/ start-new-task
-> /route-task-by-status
-> /scope-active-task
-> /breakdown-active-work
```

具体说明：

### 第一步：`/start-new-task`
用来启动一个新的需求交互，生成初始任务描述、建议目录位置和推荐下一步命令。

### 第二步：`/route-task-by-status`
判断该需求属于：
- 当前工作
- 路线图
- 参考文档
- 已完成归档

### 第三步：`/scope-active-task`
如果该需求是当前要做的，收敛到当前交付范围。

### 第四步：`/breakdown-active-work`
把当前任务拆成最小开发批次。

## 三、如果你怀疑当前任务文档和代码已经不一致

使用：
- `/sync-active-work-with-code`

## 四、如果实现完成后想确认是否真的做完

使用：
- `/review-delivery-completion`
- `docs/process/delivery-review-checklist.md`

## 五、如果任务真的完成，准备归档

使用：
- `/complete-and-archive-task`
