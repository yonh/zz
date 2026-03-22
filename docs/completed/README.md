# 已完成工作

## 状态
Completed

该目录包含已完成任务的归档文档，主要用于避免未来重复设计或重复实现。

## 进入规则
一个任务只有在以下条件满足后才应进入这里：
- 当前交付验收标准已经满足
- 剩余后续项已按需拆到 roadmap
- 该任务不再驱动当前实现

## 推荐命令
- `/review-delivery-completion` - 判断任务是否真的完成
- `/complete-and-archive-task` - 将任务归档并保留后续项
- `/route-task-by-status` - 如果不确定该任务是否真的完成，可再次归类

## 重要规则
completed 文档不是废弃文档，而是防重复记录。
它必须保留足够信息，避免以后重复做同样的设计工作。

## 当前入口
- [backend-fix/](./backend-fix/)
