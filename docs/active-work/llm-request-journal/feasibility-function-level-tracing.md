# 函数级调用追踪可行性分析

> 日期: 2026-03-30
> 状态: 草案
> 目的: 评估在 ZZ LLM Proxy 中引入多层调用追踪（函数级 + 请求级），为 LLM 自动化分析提供运行时数据

---

## 1. 背景与目标

### 1.1 问题

当前系统已有请求级别的日志记录（request_journal），但缺乏对内部函数/模块调用的追踪能力。当出现 bug 或异常行为时，开发者需要：

- 手动翻阅日志，拼凑调用链路
- 依赖经验猜测问题位置
- 难以将内部函数行为与外部请求关联

### 1.2 目标

在运行时自动记录：

1. **外部请求层**：HTTP 请求/响应的完整记录（已有）
2. **内部函数/模块层**：函数之间的调用关系、入参、返回值、耗时

这些数据供 LLM 分析使用，实现：

- 自动化 bug 定位
- 异常行为模式识别
- 性能瓶颈分析
- 请求处理链路还原

---

## 2. 方案对比

### 方案 A: Rust `tracing` + `#[instrument]` 宏（推荐）

**原理**：使用 `tracing` crate 的 `#[instrument]` 属性宏，自动在函数入口/出口生成 span 事件，记录函数名、参数、返回值和耗时。

**优点**：
- Rust 生态标准方案，成熟稳定
- 零/低侵入：仅需在函数上加 `#[instrument]` 注解
- 自动采集入参（通过 `Debug` trait）
- 支持结构化日志输出
- 可与 `tracing-subscriber` 自定义处理逻辑
- 可选对接 OpenTelemetry 扩展

**缺点**：
- 入参类型需实现 `Debug` trait（部分类型需手动实现）
- 对大型数据结构（如完整请求体）需要配置 `skip` 或 `truncate`
- 每个需追踪的函数需手动添加注解

**示例**：
```rust
#[instrument(skip(request_body), fields(request_id = %request_id))]
async fn select_provider(
    &self,
    model: &str,
    request_id: &str,
    request_body: &Value,
) -> Result<Provider, String> {
    // 业务逻辑...
}
```

### 方案 B: Tower Middleware 拦截层

**原理**：在 HTTP 服务层（tower）添加中间件，拦截请求/响应进行记录。

**优点**：
- 完全不侵入业务代码
- 天然覆盖所有 HTTP 请求
- 易于开关

**缺点**：
- 仅覆盖 HTTP 层，无法追踪内部函数调用
- 缺少函数级粒度
- 无法获取中间处理逻辑的细节

### 方案 C: OpenTelemetry 完整集成

**原理**：集成 `opentelemetry-rust` SDK，完整实现分布式追踪标准。

**优点**：
- 工业级标准，生态丰富
- 可对接 Jaeger/Zipkin 等可视化工具
- 支持跨服务追踪

**缺点**：
- 引入重量级依赖
- 配置复杂度高
- 对单服务代理场景过度设计
- 额外基础设施需求（Collector, 存储后端）

### 方案 D: 自定义宏 + 统一存储

**原理**：编写自定义 proc macro，在函数入口/出口注入记录逻辑，写入统一存储。

**优点**：
- 完全可控，可深度定制
- 可精确控制记录内容和格式
- 与现有 request_journal 深度集成

**缺点**：
- 开发维护成本高
- 需要自行处理异步上下文传播
- 重复造轮子

### 方案对比矩阵

| 维度 | A: tracing | B: Tower | C: OpenTelemetry | D: 自定义宏 |
|------|-----------|----------|-----------------|------------|
| 函数级追踪 | ✅ | ❌ | ✅ | ✅ |
| 请求级追踪 | ✅ | ✅ | ✅ | ✅ |
| 实现成本 | 低 | 低 | 高 | 高 |
| 侵入程度 | 低（注解） | 无 | 中 | 低（注解） |
| LLM 分析友好 | ⭐⭐⭐ | ⭐⭐ | ⭐⭐ | ⭐⭐⭐ |
| 可扩展性 | ⭐⭐⭐ | ⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐ |
| 维护成本 | 低 | 低 | 中 | 高 |

---

## 3. 推荐方案: 混合方案（A + B）

### 3.1 架构设计

```
请求进入
    │
    ▼
┌─────────────────────────────────────────────────┐
│  Layer 1: Tower TraceLayer (请求级)              │
│  - 记录 HTTP method, path, headers              │
│  - 生成 trace_id (复用 request_id)              │
│  - 记录响应状态码、总耗时                        │
└───────────────────┬─────────────────────────────┘
                    │ trace_id 注入 span context
                    ▼
┌─────────────────────────────────────────────────┐
│  Layer 2: tracing #[instrument] (函数级)         │
│  - proxy_handler: 记录请求处理入口               │
│  - extract_model: 记录模型解析结果               │
│  - select_provider: 记录路由决策                 │
│  - attempt_request: 记录上游调用                 │
│  - rewrite_url/headers: 记录重写操作             │
│  - write_request_journal: 记录持久化             │
└───────────────────┬─────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────────┐
│  Layer 3: 自定义 tracing Subscriber              │
│  - 收集所有 span/event                          │
│  - 按 trace_id 聚合                              │
│  - 写入 RequestJournal 扩展存储                  │
│  - 敏感数据脱敏                                  │
│  - 大数据截断 (>4KB)                            │
└─────────────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────────┐
│  Layer 4: LLM 分析接口                           │
│  - REST API: 按请求查询完整调用链                │
│  - WebSocket: 实时推送追踪数据                   │
│  - 导出格式: JSON (LLM 友好)                     │
└─────────────────────────────────────────────────┘
```

### 3.2 数据模型

```jsonc
{
  "trace_id": "req_abc123",
  "request": {
    "method": "POST",
    "path": "/v1/chat/completions",
    "headers": { "content-type": "application/json" },
    "body_size_bytes": 1024
  },
  "response": {
    "status": 200,
    "ttfb_ms": 150,
    "total_ms": 3200,
    "token_usage": { "prompt": 100, "completion": 200 }
  },
  "spans": [
    {
      "name": "proxy_handler",
      "start_us": 1711766400000000,
      "duration_us": 3200000,
      "fields": { "model": "gpt-4", "provider_count": 3 }
    },
    {
      "name": "extract_model",
      "start_us": 1711766400000100,
      "duration_us": 50,
      "fields": { "result": "gpt-4" }
    },
    {
      "name": "select_provider",
      "start_us": 1711766400000200,
      "duration_us": 200,
      "fields": { "strategy": "failover", "selected": "provider-a", "candidates": 3 }
    },
    {
      "name": "attempt_request",
      "start_us": 1711766400000500,
      "duration_us": 2800000,
      "fields": { "provider": "provider-a", "upstream_status": 200 }
    }
  ],
  "failover_chain": ["provider-a"],
  "created_at": "2026-03-30T12:00:00Z"
}
```

### 3.3 与现有系统的集成点

| 现有模块 | 集成方式 | 改动范围 |
|---------|---------|---------|
| `request_journal.rs` | 扩展数据结构，增加 `spans` 字段 | 中 |
| `proxy.rs` | 为关键函数添加 `#[instrument]` | 小 |
| `config.rs` | 增加 tracing 配置项 | 小 |
| `stats.rs` | 复用现有 LogBuffer 扩展 | 小 |
| `admin_api.rs` | 新增追踪查询 API | 中 |
| `ws.rs` | 扩展事件类型，推送 span 数据 | 小 |
| `ui/pages/RequestJournal.tsx` | 展示调用链时间线 | 中 |

---

## 4. 关键函数插桩清单

### 4.1 第一优先级（核心链路）

| 函数 | 文件 | 记录内容 |
|------|------|---------|
| `proxy_handler` | proxy.rs:16 | 请求 ID、模型、provider 数量 |
| `extract_model` | proxy.rs:618 | 解析结果、耗时 |
| `select_provider_normal` | proxy.rs:103 | 策略、候选列表、选中结果 |
| `attempt_request` | proxy.rs:339 | provider、上游 URL、状态码、耗时 |
| `write_request_journal` | proxy.rs:870 | 写入路径、数据大小 |

### 4.2 第二优先级（辅助链路）

| 函数 | 文件 | 记录内容 |
|------|------|---------|
| `RequestRewriter::rewrite_url` | rewriter.rs | 原始 URL → 重写后 URL |
| `RequestRewriter::rewrite_headers` | rewriter.rs | 修改的 header 列表 |
| `ProviderManager::get_available_for_model` | provider.rs | 可用 provider 列表 |
| `Router::select_provider` | router.rs | 路由算法详情 |
| `LogBuffer::push` | stats.rs | 日志缓冲状态 |

### 4.3 第三优先级（可选）

| 函数 | 文件 | 记录内容 |
|------|------|---------|
| `handle_api_request` | admin_api.rs | API 请求详情 |
| `handle_ws_request` | ws.rs | WebSocket 连接事件 |
| `Config::reload` | config.rs | 配置变更 |

---

## 5. 技术选型

### 5.1 核心依赖

| 依赖 | 版本 | 用途 |
|------|------|------|
| `tracing` | 0.1 | 核心追踪框架 |
| `tracing-subscriber` | 0.3 | 自定义 subscriber |
| `tracing-futures` | 0.2 | 异步函数支持 |
| `tower-http` | 0.5 | TraceLayer (HTTP 级) |
| `serde_json` | 已有 | span 数据序列化 |

### 5.2 可选依赖

| 依赖 | 用途 | 条件 |
|------|------|------|
| `tracing-opentelemetry` | OpenTelemetry 导出 | 需要分布式追踪时 |
| `tracing-chrome` | Chrome Trace 格式导出 | 性能分析时 |
| `serde` (已有) | 数据序列化 | 始终需要 |

---

## 6. 性能影响与缓解策略

### 6.1 预期开销

| 场景 | CPU 开销 | 内存开销 | 延迟增加 |
|------|---------|---------|---------|
| 完整追踪 (所有函数) | 10-30% | +50MB | <1ms |
| 关键路径追踪 (5函数) | 3-8% | +10MB | <0.5ms |
| 采样追踪 (1%) | <1% | +5MB | <0.1ms |
| 仅请求级 (TraceLayer) | <2% | +5MB | <0.2ms |

### 6.2 缓解策略

1. **采样**：可配置采样率，生产环境默认 1%
2. **异步写入**：span 数据通过 channel 异步写入，不阻塞请求
3. **数据截断**：大于 4KB 的参数自动截断
4. **条件编译**：通过 feature flag 在 release 构建中完全禁用
5. **缓冲批量写入**：合并多个 span 批量持久化

### 6.3 配置示例

```toml
[observability.tracing]
enabled = true
level = "info"                    # tracing 级别
sampling_rate = 1.0               # 采样率 (0.0-1.0)
max_field_size = 4096             # 字段截断阈值 (bytes)
async_write = true                # 异步写入
feature_flag = "fn-tracing"       # 编译期 feature flag

[observability.tracing.functions]
# 可配置哪些函数启用/禁用追踪
proxy_handler = true
extract_model = true
select_provider = true
attempt_request = true
rewrite_url = false               # 可选关闭
```

---

## 7. LLM 集成分析

### 7.1 数据格式设计原则

LLM 友好的追踪数据应满足：

1. **结构化**：JSON 格式，字段名自解释
2. **完整性**：一条 trace 包含完整调用链
3. **紧凑性**：去除冗余，控制 token 消耗
4. **上下文丰富**：包含足够的环境信息（时间、配置状态）

### 7.2 LLM 分析场景

| 场景 | 输入 | 预期输出 |
|------|------|---------|
| Bug 定位 | 异常请求的完整 trace | 可能的失败原因和位置 |
| 性能分析 | 多条慢请求的 trace | 瓶颈函数和优化建议 |
| 异常检测 | 批量 trace 数据 | 异常模式和行为特征 |
| 路由诊断 | 路由决策 trace | 路由策略问题分析 |
| Provider 评估 | 多 provider 对比 trace | provider 质量评估 |

### 7.3 API 设计

```
GET /zz/api/traces/:trace_id        # 获取单条完整追踪
GET /zz/api/traces?limit=100        # 最近追踪列表
GET /zz/api/traces?error=true       # 错误请求追踪
GET /zz/api/traces?slow=true        # 慢请求追踪
GET /zz/api/traces/export?format=json  # LLM 分析导出
```

---

## 8. 实施路径

### Phase 1: 基础设施（预计 1-2 天）

- 添加 `tracing`, `tracing-subscriber`, `tracing-futures` 依赖
- 实现自定义 `JournalSubscriber`，将 span 数据写入 request_journal
- 添加配置项支持

### Phase 2: 核心链路插桩（预计 1-2 天）

- 为 `proxy_handler`, `extract_model`, `select_provider`, `attempt_request` 添加 `#[instrument]`
- 添加 Tower TraceLayer
- 扩展 request_journal 数据结构

### Phase 3: 查询与展示（预计 1-2 天）

- 新增追踪查询 API
- UI 展示调用链时间线
- LLM 导出接口

### Phase 4: 生产优化（预计 1 天）

- 采样策略实现
- 敏感数据过滤
- 性能测试和调优
- Feature flag 控制

### Phase 5: 高级功能（可选）

- OpenTelemetry 集成
- 自动异常检测
- LLM 自动分析触发

---

## 9. 风险评估

| 风险 | 概率 | 影响 | 缓解措施 |
|------|------|------|---------|
| 性能影响超出预期 | 中 | 高 | 采样 + feature flag |
| 磁盘占用增长过快 | 中 | 中 | 保留策略 + 压缩 |
| 敏感数据泄露 | 低 | 高 | 脱敏过滤 + 配置控制 |
| 维护成本增加 | 低 | 中 | 控制插桩范围 |
| 依赖版本冲突 | 低 | 低 | Cargo 已有解析机制 |

---

## 10. 结论

### 可行性: ✅ 高

当前项目具备良好的接入条件：

1. **已有基础设施**：request_id、request_journal、WebSocket 推送、LogBuffer
2. **模块化架构**：各模块职责清晰，插桩点明确
3. **Rust 生态成熟**：`tracing` 是标准方案，文档和社区支持完善
4. **渐进式实施**：可从核心链路开始，逐步扩展

### 建议优先级

1. **立即实施**：Phase 1 + Phase 2（基础设施 + 核心插桩）
2. **短期实施**：Phase 3（查询与展示）
3. **按需实施**：Phase 4 + Phase 5（优化与高级功能）

### 预期收益

- 开发调试效率提升 3-5 倍（无需手动拼凑日志）
- LLM 可自动化分析 80% 以上的常见问题
- 系统可观测性从"黑盒"变为"白盒"
- 为后续 AIOps 功能奠定数据基础
