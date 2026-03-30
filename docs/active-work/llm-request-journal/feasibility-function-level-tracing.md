# 运行时调用追踪可行性分析（修订版）

> 日期: 2026-03-30
> 状态: 第四版（经代码验证 + 边界审查后修订）
> 审视人: rust-reviewer + llm-reviewer（初版）、rust-architect / req-analyst / observability-expert（团队审查）、代码验证审查（第四版）
> 目的: 评估在 ZZ LLM Proxy 中引入运行时调用追踪，为调试和 LLM 分析提供数据

---

## 1. 背景与目标

### 1.1 现状

当前系统已有以下可观测性设施：

| 设施 | 模块 | 能力 | 局限 |
|------|------|------|------|
| 结构化日志 | `logging.rs` + `tracing` crate | info/debug/warn/error 级别日志 | 无函数级上下文，需手动拼凑调用链 |
| 请求日志缓冲 | `stats.rs` RequestLogBuffer | 内存环形缓冲，含 ttfb/duration/token | 仅内存，无持久化 |
| 请求取证 | `request_journal.rs` | 完整请求/响应持久化，按日期存储，敏感数据脱敏 | 无内部函数耗时，无路由决策过程 |
| 实时推送 | `ws.rs` WsBroadcaster | WebSocket 广播日志/状态/统计 | 仅推送已有数据 |

### 1.2 问题

当出现异常时（如请求失败、延迟飙升、路由错误），开发者面临：

1. **知道结果，不知道过程**：request_journal 记录了"选了哪个 provider"，但不知道"为什么选它"、"花了多久选择"、"中间有哪些候选"
2. **日志分散**：各函数的 `tracing::info!` 日志通过 stderr 输出，与具体请求的关联靠人工匹配 request_id
3. **缺乏耗时分解**：只知道总耗时 3200ms，不知道解析模型花了多少、选择 provider 花了多少、上游调用花了多少

### 1.3 目标

**核心目标**：为每个请求记录关键步骤的耗时和决策信息，用于调试和 LLM 辅助分析。

**非目标**：
- 不追求完整的函数调用图（开销大、价值低）
- 不实现分布式追踪（单服务无需跨服务 trace propagation）
- 不引入额外基础设施（Jaeger/Zipkin 等）

---

## 2. 方案设计

### 2.1 分层策略

基于审视结论，采用**两层设计**：

```
┌─────────────────────────────────────────────────┐
│  Tier 1: 关键耗时字段（默认启用）                  │
│  - 在现有 RequestJournalEntry 中增加 timing 字段  │
│  - 零新依赖，零架构变更                            │
│  - 记录 5 个关键耗时节点                           │
└─────────────────────────────────────────────────┘
          │ 默认关闭，按需开启
          ▼
┌─────────────────────────────────────────────────┐
│  Tier 2: 完整函数追踪（可选）                      │
│  - 基于现有 tracing crate + 自定义 Layer          │
│  - #[instrument] 注解关键函数                     │
│  - 独立存储，不影响现有 journal                    │
└─────────────────────────────────────────────────┘
```

---

### 2.2 Tier 1: 关键耗时字段（推荐立即实施）

#### 设计思路

不引入任何新依赖或架构变更，仅在 `proxy_handler` 中用 `std::time::Instant` 累积记录关键节点耗时，写入现有 `RequestJournalEntry`。

**与初版的关键差异**：

1. `rewrite_url`/`rewrite_headers` 在 `attempt_request` 内部调用（proxy.rs:358, proxy.rs:364），不是 `proxy_handler` 的独立步骤。因此删除 `rewrite_ms`，将 rewrite 归入 `upstream_total_ms`
2. provider 选择在 failover 循环内（proxy.rs:91），可能多次执行。`select_provider_ms` 改为累积值
3. `extract_model` 返回 `String`（非 `Option<String>`），解析失败时返回 `"unknown"`
4. `upstream_ttfb_ms` 与 `LogEntry.ttfb_ms` 同源数据，不重复测量
5. `parse_model_ms` 包含 `extract_model` + `get_available_for_model` 两步（模型解析 + 预过滤），统一归入"模型解析阶段"
6. SSE 流式请求：`upstream_total_ms` 记录从发起请求到**上游最后一个 chunk 到达 proxy** 的总耗时（不含向客户端转发的时间，因为那是 proxy 不可控的网络因素）；`upstream_ttfb_ms` 记录到**首个 chunk 从上游到达 proxy** 的时间。两者对 SSE 均适用。**注意**：`upstream_total_ms` 的计时终点是"上游响应完全到达 proxy"，而非"proxy 将数据发完给客户端"
7. 新增 `completed` 字段标记 timing 数据是否完整（区分"未采集"与"耗时为 0"）
8. 新增 `selection_reason` 和 `available_providers` 字段，记录路由决策上下文

#### 实际执行流程

```
proxy_handler:
  proxy_start = Instant::now()
  │
  ├─ body_bytes = collect().await
  ├─ extract_model(&body_bytes) → String           ← 返回 String，失败为 "unknown"
  ├─ get_available_for_model(&model)               ← 预过滤，文档初版未提及
  │
  ├─ for _ in 0..max_retries {                     ← 失败重试循环
  │   ├─ model_pins.get(&model)                    ← 优先检查固定绑定
  │   ├─ select_provider_normal(...)               ← 规则 + 策略选择
  │   ├─ attempt_request(...)
  │   │   ├── rewrite_url(base_url, path)          ← 在 attempt_request 内部
  │   │   ├── rewrite_headers(headers, api_key, …) ← 在 attempt_request 内部
  │   │   ├── 构建 upstream 请求
  │   │   └── HTTP 往返 → 返回 ttfb_ms
  │   ├─ Ok → 记录 timing, return
  │   └─ Err → retry_count++, continue
  │
  └─ All failed → 503 + timing
```

#### 数据模型扩展

在现有 `RequestJournalEntry` 中增加 `timing` 字段：

```rust
// request_journal.rs 扩展

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct RequestTiming {
    /// 模型解析耗时 (ms) — extract_model + get_available_for_model
    /// ⚠️ 已知权衡：两步操作合并为一个字段，无法单独区分解析 vs 过滤耗时。
    /// 理由：两步通常均 <1ms，拆分的增量价值不足以抵消字段膨胀的复杂度
    pub parse_model_ms: u64,
    /// Provider 选择耗时 (ms) — 累积所有重试轮次的 select 耗时
    pub select_provider_ms: u64,
    /// 上游请求总耗时 (ms) — 累积所有 attempt_request 调用
    /// 内含 rewrite_url + rewrite_headers + HTTP 往返
    /// SSE 场景: 从发起请求到上游最后一个 chunk 到达 proxy 的时间（不含向客户端转发时间）
    /// 非 SSE 场景: 完整 HTTP 往返耗时
    pub upstream_total_ms: u64,
    /// 上游首字节耗时 (ms) — 最终一次尝试的 TTFB
    /// 同源 LogEntry.ttfb_ms (stats.rs RequestLogBuffer)，不重复测量
    /// SSE 场景: 首个 chunk 从上游到达 proxy 的时间
    pub upstream_ttfb_ms: u64,
    /// 重试次数 — 在成功之前的失败次数（0 = 首次成功）
    pub retry_count: u8,
    /// 每次重试尝试的 provider 名称列表 — 与现有 failover_chain 同义但位于 timing 上下文内
    /// retry_count > 0 时，前 retry_count 个为失败的 provider，最后一个为成功的 provider
    /// 长度 = retry_count + 1（最后一次成功/全部失败时长度 = retry_count）
    pub retry_providers: Vec<String>,
    /// 每次重试尝试的上游耗时列表 (ms) — 与 retry_providers 等长
    /// 值为每次 attempt_request 的 wall-clock 耗时（含 rewrite + HTTP 往返）
    pub retry_durations_ms: Vec<u64>,
    /// 可用 provider 数量 — get_available_for_model 返回的候选数
    /// 使用 u16 防止 provider 数量超过 255 时溢出
    pub available_providers: u16,
    /// 路由决策原因 — 最终一次成功/失败选择所用的原因
    /// 格式规范（见下方 SelectionReason 枚举）:
    ///   - "pinned:<provider_name>"  — 模型固定绑定
    ///   - "rule:<rule_index>"       — 路由规则匹配，index 为规则序号
    ///   - "strategy:<strategy_name>" — 策略选择（如 failover/random/round_robin）
    /// 最大长度: 128 字节
    pub selection_reason: String,
    /// timing 数据是否完整 — false 表示请求中途失败，部分字段为默认值 0
    pub completed: bool,
}

// RequestJournalEntry 新增字段
pub struct RequestJournalEntry {
    // ... 现有 19 个字段不变 ...
    pub timing: Option<RequestTiming>,  // 新增
}
```

#### 实现方式

在 `proxy.rs` 的 `proxy_handler` 中添加累积计时：

```rust
pub async fn proxy_handler(req: Request<Incoming>, state: AppState) -> ... {
    let proxy_start = std::time::Instant::now();

    // ... collect body ...

    // 1. 模型解析 + 预过滤
    let t_parse = std::time::Instant::now();
    let model = extract_model(&body_bytes);  // 返回 String，失败为 "unknown"
    let base_providers = if model != "unknown" {
        state.provider_manager.get_available_for_model(&model)
    } else {
        state.provider_manager.get_available()
    };
    let parse_model_ms = t_parse.elapsed().as_millis() as u64;

    let mut timing = RequestTiming::default();
    timing.parse_model_ms = parse_model_ms;
    timing.available_providers = base_providers.len() as u16;
    let mut total_select_ms: u64 = 0;
    let mut total_upstream_ms: u64 = 0;
    let mut retry_count: u8 = 0;

    // ——— 前置失败处理 ———
    // 如果无可用 provider，直接 503 + 记录 timing（completed=false）
    if base_providers.is_empty() && model_pins.get(&model).is_none() {
        timing.completed = false;
        // 此处 timing 仅 parse_model_ms 和 available_providers(=0) 有效
        write_request_journal(..., Some(timing));
        return Err(ProxyError::NoProviderAvailable);
    }

    for _ in 0..max_retries {
        // 2. 选择 provider（含 pin 检查 + select_provider_normal）
        let t_select = std::time::Instant::now();
        let (provider_name, provider, is_pinned, rule_idx) = { /* 现有逻辑，额外返回 rule_idx */ };
        total_select_ms += t_select.elapsed().as_millis() as u64;

        // 记录路由决策原因（标准化格式，见 selection_reason 注释）
        if is_pinned {
            timing.selection_reason = format!("pinned:{}", provider_name);
        } else if rule_idx.is_some() {
            timing.selection_reason = format!("rule:{}", rule_idx.unwrap());
        } else {
            timing.selection_reason = format!("strategy:{}", /* 当前策略名 */);
        }
        // selection_reason 最大 128 字节，超出截断
        if timing.selection_reason.len() > 128 {
            timing.selection_reason.truncate(128);
        }

        // 3. 上游请求（内含 rewrite + HTTP）
        let t_attempt = std::time::Instant::now();
        match attempt_request(...).await {
            Ok((response, resp_bytes, resp_ttfb_ms, token_usage, upstream_url)) => {
                let attempt_ms = t_attempt.elapsed().as_millis() as u64;
                total_upstream_ms += attempt_ms;
                timing.select_provider_ms = total_select_ms;
                timing.upstream_total_ms = total_upstream_ms;
                timing.upstream_ttfb_ms = resp_ttfb_ms;  // 同源 stats.rs LogEntry.ttfb_ms
                // SSE: ttfb_ms = 首 chunk 从上游到达 proxy; upstream_total_ms = 上游全部 chunk 到达 proxy
                // 非 SSE: ttfb_ms = 首 byte; upstream_total_ms = 完整往返
                timing.retry_count = retry_count;
                timing.retry_providers.push(provider_name.clone());
                timing.retry_durations_ms.push(attempt_ms);
                timing.completed = true;

                // 现有日志、journal 逻辑（传 timing 到 write_request_journal）
                write_request_journal(..., Some(timing.clone()));
                return Ok(response);
            }
            Err(e) => {
                let attempt_ms = t_attempt.elapsed().as_millis() as u64;
                total_upstream_ms += attempt_ms;
                timing.retry_providers.push(provider_name.clone());
                timing.retry_durations_ms.push(attempt_ms);
                retry_count += 1;
                // ... 现有 failover 逻辑不变 ...
            }
        }
    }

    // 全部失败 — completed = false 标记数据不完整
    timing.select_provider_ms = total_select_ms;
    timing.upstream_total_ms = total_upstream_ms;
    timing.upstream_ttfb_ms = 0;
    timing.retry_count = retry_count;
    // retry_providers 和 retry_durations_ms 已在循环中累积
    timing.completed = false;
    write_request_journal(..., Some(timing));
}
```

#### 输出示例（LLM 分析友好的自然语言摘要）

```
请求 req_abc123 (POST /v1/chat/completions):
- 模型: gpt-4
- Provider: provider-a (failover 策略)
- 状态: 200 OK
- 耗时分解:
  · 解析模型: 5ms (含预过滤, 可用 provider: 3)
  · 选择 Provider: 2ms (决策: strategy:failover, 重试 0 次)
  · 上游总耗时: 2800ms (含 rewrite, SSE=否, 计时终点=上游响应完全到达proxy)
  · 上游首字节: 150ms
  · 总耗时: 3200ms (沿用 duration_ms)
- Token: prompt=100, completion=200
- 数据完整: 是

请求 req_def456 (POST /v1/chat/completions):     ← 重试场景示例
- 模型: gpt-4
- Provider: provider-c (最终成功)
- 状态: 200 OK
- 耗时分解:
  · 解析模型: 3ms
  · 选择 Provider: 8ms (决策: strategy:failover, 累积3次选择)
  · 上游总耗时: 5300ms (累积3次尝试)
  · 上游首字节: 200ms (最终成功尝试)
  · 重试分解:
    ① provider-a: 5000ms (超时)
    ② provider-b: 100ms (连接失败)
    ③ provider-c: 200ms (成功)
- 数据完整: 是
```

#### timing 状态机

| 阶段 | parse_model_ms | select_provider_ms | upstream_total_ms | upstream_ttfb_ms | retry_count | retry_providers | retry_durations_ms | completed |
|------|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|
| body collect 失败 | 0 | 0 | 0 | 0 | 0 | [] | [] | false |
| extract_model 之后失败 | ✓ | 0 | 0 | 0 | 0 | [] | [] | false |
| 无可用 provider | ✓ | 0 | 0 | 0 | 0 | [] | [] | false |
| 首次尝试成功 | ✓ | ✓ | ✓ | ✓ | 0 | [1项] | [1项] | true |
| 重试 N 次后成功 | ✓ | ✓(累积) | ✓(累积) | ✓(最终) | N | [N+1项] | [N+1项] | true |
| 全部重试失败 | ✓ | ✓(累积) | ✓(累积) | 0 | N | [N项] | [N项] | false |

#### 改动范围

| 文件 | 改动 | 工作量 |
|------|------|--------|
| `request_journal.rs` | 增加 `RequestTiming` 结构体（含 10 个字段）+ `RequestJournalEntry` 新增 `timing` 字段 | 小 |
| `proxy.rs` | `proxy_handler` 中添加累积计时点 + 前置失败处理 + SSE 语义处理 + per-retry 记录 | 中 |
| `proxy.rs` | `write_request_journal` 增加可选 timing 参数（建议用 `TimingContext` 结构体封装，当前 16 参数） | 小 |
| `config.rs` | 增加 `TimingConfig { enabled: bool }`（置于 `[observability.timing]` 下） | 小 |
| `admin_api.rs` | journal 查询 API 自动包含 timing + `?failed=true` 筛选 | 小 |
| `ui/` | 展示耗时分解（`timing: None` 时优雅降级，显示"未采集"） | 小 |

**总计：约 1-2 天**

---

### 2.3 Tier 2: 完整函数追踪（可选，按需开启）

#### 设计思路

利用项目已有的 `tracing` crate，实现自定义 `Layer` 收集 span 数据。不引入新依赖（除 `tracing-futures` 可选）。

#### 关键技术决策

| 决策点 | 方案 | 理由 |
|--------|------|------|
| 请求级拦截 | 在 `proxy_handler` 入口手动创建 span | 项目用 hyper service_fn，无 tower |
| tracing 初始化 | 改造 `logging.rs`，在初始化时组合 Layer | `.init()` 后无法追加，需一次性组合 |
| span 收集 | 实现 `tracing_subscriber::Layer` trait | 比自定义 Subscriber 更轻量，可叠加 |
| 数据存储 | 独立目录 `logs/traces/<date>/<trace_id>.json` | 与 journal 分离，独立 TTL 和清理 |
| 函数注解 | `#[instrument]` + `skip(不实现Debug的参数)` | 标准方案，低侵入 |
| rewrite 计时 | 不单独插桩 rewrite，归入 `attempt_request` span | rewrite 在 attempt_request 内部（proxy.rs:358,364），拆出需改函数签名 |

#### 架构

```
proxy_handler 入口
    │
    ├─ 创建 span!(Level::INFO, "proxy", request_id = %id)
    │
    ├── extract_model()     ← #[instrument(name = "extract_model", skip(body))]
    │
    ├── select_provider()   ← #[instrument(name = "select_provider", skip(providers))]
    │
    ├── attempt_request()   ← #[instrument(name = "attempt_request", skip(...))]
    │   ├── rewrite_url()     ← 内部调用，不单独 instrument
    │   └── rewrite_headers() ← 内部调用，不单独 instrument
    │
    └── write_journal()     ← #[instrument(name = "write_journal", skip(entry))]

    │ 所有 span 事件被 JournalTraceLayer 收集
    ▼
┌──────────────────────────────────────┐
│  JournalTraceLayer (自定义 Layer)     │
│  - 按 trace_id 聚合 spans            │
│  - span 关闭时序列化                  │
│  - 通过 mpsc channel 异步写入        │
│  - 独立存储: logs/traces/<date>/     │
└──────────────────────────────────────┘
```

#### logging.rs 改造

```rust
// 改造前（实际代码）
pub fn init_logging(log_level: &str) -> Result<(), anyhow::Error> {
    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(log_level))?;
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();
    Ok(())
}

// 改造后
pub fn init_logging(log_level: &str, trace_layer: Option<JournalTraceLayer>) -> Result<(), anyhow::Error> {
    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(log_level))?;

    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr);

    if let Some(layer) = trace_layer {
        subscriber.with(layer).init();
    } else {
        subscriber.init();
    }
    Ok(())
}
```

#### 插桩函数清单

```rust
// proxy.rs — 核心链路
#[instrument(name = "proxy_handler", skip(req, state), fields(
    request_id = %request_id,
    method = %method,
    path = %path,
))]
pub async fn proxy_handler(req: Request<Incoming>, state: AppState) -> ... { }

#[instrument(name = "extract_model", skip(body), fields(result = %model))]
fn extract_model(body: &[u8]) -> String { }  // 注意：返回 String，非 Option<String>

#[instrument(name = "select_provider_normal", skip(providers, state), fields(
    model = %model,
))]
fn select_provider_normal(
    model: &str,
    providers: &[(String, Arc<Provider>)],
    state: &AppState,
) -> Option<(String, Arc<Provider>)> { }

#[instrument(name = "attempt_request", skip(provider, headers, body_bytes, state), fields(
    provider = %provider_name,
    status = %status,
))]
async fn attempt_request(
    provider_name: &str,
    provider: &Arc<Provider>,
    path: &str,
    method: &Method,
    headers: &HeaderMap,
    body_bytes: &Bytes,
    state: &AppState,
    is_sse: bool,
    request_timeout_secs: u64,
) -> Result<(Response, u64, u64, Option<TokenUsage>, String), ProxyError> { }
// 注意：rewrite_url/rewrite_headers 在 attempt_request 内部调用，不单独 instrument
```

**注意**：所有 `skip` 参数要么不实现 `Debug`（如 `Bytes`、`AppState`），要么体积过大。关键字段通过 `fields()` 显式记录。

#### 配置

```toml
[observability.timing]
enabled = true               # Tier 1 默认开启（开销极低）

[observability.tracing]
enabled = false              # Tier 2 默认关闭
sampling_mode = "adaptive"   # "fixed" | "adaptive"
base_rate = 0.01             # 正常请求基础采样率
slow_threshold_ms = 3000     # 慢请求阈值（超过则 100% 采样）
error_sampling = 1.0         # 错误请求采样率（建议 100%）
storage_dir = "logs/traces"
retention_days = 3
```

> **Tier 1 + Tier 2 数据冗余说明**：当两层同时开启时，Tier 1 的 `upstream_ttfb_ms`/`select_provider_ms` 等与 Tier 2 的 span duration 语义重叠。这是设计意图：Tier 1 数据嵌入 journal 保持自包含性（无需关联 trace 文件即可查看耗时分解），Tier 2 提供更深度的调用链。两者计时均基于 `Instant`，差异在亚毫秒级别可忽略。

#### 改动范围

| 文件 | 改动 | 工作量 |
|------|------|--------|
| `logging.rs` | 改造初始化，支持组合 Layer | 中 |
| `proxy.rs` | 添加 `#[instrument]` 注解（4个函数：proxy_handler, extract_model, select_provider_normal, attempt_request） | 小 |
| 新增 `trace_layer.rs` | 自定义 `Layer` 实现（含 span_id/parent_id、混合采样、mpsc 异步写入） | 大 |
| `config.rs` | 增加 `TracingConfig`（含采样策略配置） | 小 |
| `main.rs` | 初始化 trace_layer 并传入 logging | 小 |
| `admin_api.rs` | 新增 `?include_trace=true` 查询 | 中 |

**总计：约 3-5 天**

**注意**：Tier 2 需验证 `tracing` + `tracing-futures` 版本与 `hyper 1.x` 的兼容性。`#[instrument]` 用于 async fn 需确保 `tracing` >= 0.1.35 或引入 `tracing-futures`。建议在 Phase 2 开始前先做兼容性验证。

---

## 3. 数据存储设计

### 3.1 Tier 1 数据

存储在现有 `RequestJournalEntry` JSON 中，每个文件增加约 100-200 字节（timing 字段）。

```
logs/request-journal/2026-03-30/
├── req_abc123.json      # 包含 timing 字段
└── req_def456.json
```

### 3.2 Tier 2 数据

独立目录存储，与 journal 分离：

```
logs/traces/2026-03-30/
├── req_abc123.trace.json   # 仅包含 spans 数组
└── req_def456.trace.json
```

**Trace 文件格式**：

```json
{
  "trace_id": "req_abc123",
  "spans": [
    {
      "span_id": "span_001",
      "parent_id": null,
      "name": "proxy_handler",
      "start_ms": 0,
      "duration_ms": 3200,
      "fields": { "request_id": "req_abc123", "method": "POST", "path": "/v1/chat/completions" }
    },
    {
      "span_id": "span_002",
      "parent_id": "span_001",
      "name": "extract_model",
      "start_ms": 0,
      "duration_ms": 5,
      "fields": { "result": "gpt-4" }
    },
    {
      "span_id": "span_003",
      "parent_id": "span_001",
      "name": "select_provider",
      "start_ms": 5,
      "duration_ms": 2,
      "fields": { "strategy": "failover", "model": "gpt-4", "selected": "provider-a" }
    },
    {
      "span_id": "span_004",
      "parent_id": "span_001",
      "name": "attempt_request",
      "start_ms": 8,
      "duration_ms": 2800,
      "fields": { "provider": "provider-a", "upstream_url": "https://...", "status": 200 }
    }
  ],
  "created_at": "2026-03-30T12:00:00Z"
}
```

### 3.3 存储增长估算

| 指标 | Tier 1 | Tier 2 (1% 采样) |
|------|--------|------------------|
| 每请求增加 | ~150-200 字节 | ~1-2 KB |
| 日增量 (10k 请求) | ~2 MB | ~200 KB |
| 7 天总量 | ~14 MB | ~1.4 MB |
| 30 天总量 | ~60 MB | ~6 MB |

---

## 4. LLM 分析集成

### 4.1 重新定位

基于审视结论，**不将"LLM 自动分析"作为初始目标**。定位为：

> **调试辅助数据采集** — 为开发者和 LLM 提供比现有日志更丰富的上下文信息

### 4.2 LLM 分析场景重新评估

| 场景 | 可行性 | 所需数据 | 推荐方式 |
|------|--------|---------|---------|
| 请求失败诊断 | ✅ 可行 | 单条 trace + 错误信息 | 按需查询，人工或 LLM 分析 |
| 耗时异常分析 | ✅ 可行 | timing 字段 + 历史基线 | 自动标记 >P95 的请求 |
| 路由决策回放 | ⚠️ 有限 | failover_chain + select_provider span | 人工查看 |
| 批量模式识别 | ⚠️ 成本高 | 大量 trace 数据 | 批量导出，定期分析 |
| Provider 质量评估 | ✅ 可行 | 聚合 stats 数据 | 用现有 ProviderStats 即可 |

### 4.3 数据格式原则

LLM 接收的数据应经过预处理，而非原始 JSON：

1. **模板化自然语言摘要**：`?format=summary` 使用 `format!()` 字符串拼接（非模板引擎、非 LLM 调用），将 timing 数据转为可读纯文本（如 2.2 节示例），开销极低，不引入额外依赖
2. **异常高亮**：只传递偏离正常值的数据（如 `upstream_total_ms > P95`）
3. **上下文精简**：去除对诊断无关的字段

### 4.3.1 LLM 分析触发时机

| 场景 | 触发方式 | 输入 | 成本估算 |
|------|---------|------|---------|
| 请求失败诊断 | 手动 | 单条 summary + error (~200 tokens) | 极低 |
| 慢请求分析 | 自动标记 | timing + baseline 对比 (~300 tokens) | 极低 |
| 批量模式识别 | 定时任务 | 聚合统计 + 异常 trace 样本 (~5k tokens) | 中等 |

### 4.4 API 设计

复用现有 journal API，通过参数控制：

```
GET /zz/api/request-journal/:id                    # 现有，自动包含 timing (Tier 1)
GET /zz/api/request-journal/:id?include_trace=true # 扩展，包含完整 spans (Tier 2)
GET /zz/api/request-journal?slow=true              # 扩展，筛选慢请求（阈值: timing.upstream_total_ms > 配置的 slow_threshold_ms）
GET /zz/api/request-journal?failed=true            # 扩展，筛选失败请求（timing.completed = false）
GET /zz/api/request-journal/export?format=summary  # 扩展，模板生成的自然语言摘要（非 LLM 调用）
```

不新建 `/zz/api/traces` 路径，trace 是 journal 的附属数据。

---

## 5. 实施路径

### Phase 1: Tier 1 关键耗时（1-2 天）

1. 在 `request_journal.rs` 中增加 `RequestTiming` 结构体（含 `#[derive(Default)]`）
2. 在 `request_journal.rs` 的 `RequestJournalEntry` 中增加 `timing: Option<RequestTiming>` 字段
3. 在 `proxy.rs` 的 `proxy_handler` 中添加累积计时点（parse_model、select_provider、attempt_request）
4. 修改 `write_request_journal` 签名，增加 `timing: Option<RequestTiming>` 参数
5. 验证 journal 文件正确包含 timing 数据
6. UI 展示耗时分解

**验收标准**：
- 每个 journal 条目包含 timing 字段，展示页面可见耗时分解
- UI 在 `timing: None`（旧数据）时优雅降级，显示"未采集"而非空白或报错
- failover 场景下 retry_count、retry_providers、retry_durations_ms 和累积耗时正确
- SSE 流式请求的 ttfb_ms（首 chunk 从上游到达 proxy）和 upstream_total_ms（上游全部 chunk 到达 proxy）均正确记录
- 失败请求 `completed = false`，部分字段为 0，不误导消费者
- `selection_reason` 格式遵循 `pinned:|rule:|strategy:` 前缀规范，最大 128 字节
- `available_providers` 正确反映路由决策（使用 u16 类型）
- 前置失败场景（无可用 provider、body collect 失败）正确写入 timing（completed=false）
- `TimingConfig.enabled = false` 时跳过所有计时逻辑，timing 字段为 None

### Phase 2: Tier 2 基础设施（2-3 天）

1. 改造 `logging.rs`，支持组合自定义 Layer
2. 实现 `trace_layer.rs`：自定义 `Layer` trait，收集 span 数据
3. 增加 `TracingConfig` 到 `config.rs`
4. 异步写入 span 数据到独立目录

**验收标准**：开启 tracing 配置后，指定目录生成 trace 文件。

### Phase 3: 核心函数插桩（1 天）

1. 为 `proxy_handler`、`extract_model`、`select_provider_normal`、`attempt_request` 添加 `#[instrument]`
2. 验证 span 嵌套和数据完整性（注意：rewrite_url/rewrite_headers 在 attempt_request 内部，不单独 instrument）

**验收标准**：单条 trace 包含完整的 4 个 spans（proxy_handler > extract_model + select_provider_normal > attempt_request），数据正确。

### Phase 4: 查询与优化（1-2 天）

1. 扩展 journal API 支持 `?include_trace=true`
2. 实现 `?format=summary` 自然语言摘要输出
3. 采样策略实现
4. 实测性能开销，调整采样率

**验收标准**：
- API 可查询 trace 数据（`?include_trace=true`）
- `?format=summary` 输出模板化自然语言摘要
- 混合采样策略生效：慢请求/错误 100% 采样，正常请求按 base_rate 采样
- 性能开销 <5%（测试方法：关闭 tracing 基线 vs 开启 tracing，1000 并发请求，对比 P50/P95/P99 延迟）

---

## 6. 风险评估

| 风险 | 概率 | 影响 | 缓解措施 |
|------|------|------|---------|
| Tier 1 累积计时影响请求延迟 | 极低 | 低 | `Instant::now()` 开销 <50ns，4 次调用总计 <200ns |
| Tier 1 failover 累积逻辑出错 | 中 | 中 | 编写单元测试覆盖单次/多次重试场景 + 状态机全部路径 |
| SSE 场景 ttfb_ms 语义与现有 LogEntry 不一致 | 中 | 中 | 统一 SSE ttfb 定义：计时终点为"上游 chunk 到达 proxy"，明确注释 |
| `retry_providers`/`retry_durations_ms` 与 `failover_chain` 冗余 | 低 | 低 | 两者语义重叠但位置不同（timing 内 vs journal entry 顶层），后续可考虑合并 |
| Tier 2 Layer 实现复杂度 | 中 | 中 | 先完成 Tier 1，评估后再决定 |
| Tier 2 性能开销超标 | 中 | 中 | 默认关闭 + 混合采样 + 实测验证 |
| trace 文件磁盘增长 | 低 | 低 | 独立 TTL，默认仅保留 3 天 |
| `#[instrument]` 与异步上下文不兼容 | 低 | 高 | 验证 tracing >= 0.1.35 或引入 tracing-futures |
| `write_request_journal` 签名变更影响调用点 | 低 | 低 | 用 `TimingContext` 结构体封装，当前 16 参数 |
| mpsc channel 满时丢失 span 数据 | 中 | 低 | 设置 channel capacity = 1000，满时丢弃并计数告警 |

---

## 7. 审视结论摘要

### rust-reviewer 关键意见

1. **Tower TraceLayer 不可行**：项目用 hyper service_fn，非 tower。已从方案中移除。
2. **用 Layer 而非 Subscriber**：`tracing_subscriber::Layer` 比 `Subscriber` 更适合叠加到现有初始化。
3. **#[instrument] 需注意 skip**：`Bytes`、`AppState` 等不实现 Debug 或体积过大，必须 skip。
4. **性能数据不可信**：已删除具体数字，改为"需实测"。
5. **Phase 1 工作量被低估**：已从 0.5-1 天调整为 1-2 天。

### llm-reviewer 关键意见

1. **增量价值有限**：现有 journal 已覆盖大部分诊断需求。已采用轻量级 Tier 1 作为主方案。
2. **LLM 分析场景过度乐观**：已从"LLM 自动分析"降级为"调试辅助数据采集"。
3. **JSON 不如自然语言友好**：已增加 `?format=summary` 摘要输出。
4. **应复用 journal API**：已改为扩展现有 `/zz/api/request-journal` 路径。
5. **推荐轻量级方案**：已采纳，Tier 1 为核心，Tier 2 可选。

### 修订版补充审视（代码对照审查）

1. **rewrite 在 attempt_request 内部**：已删除 `rewrite_ms`，归入 `upstream_total_ms`。
2. **failover 循环未纳入**：`select_provider_ms` 改为累积值，增加 `retry_count`。
3. **`extract_model` 返回 `String`**：已修正文档中错误的 `Option<String>`。
4. **`upstream_ttfb_ms` 与 `LogEntry.ttfb_ms` 同源**：已在注释中明确，不重复测量。
5. **`logging.rs` 初始化 API 细节**：已修正为实际代码的 `.or_else` + `try_new` 风格。
6. **`rewrite_headers` 签名有 4 个参数**：`api_key` 是敏感参数，必须在 `skip` 列表中。
7. **Tier 2 不再单独 instrument rewrite**：减少插桩数量从 7 个到 4 个。

### 第三版团队审查结论（三方专家共识）

**审查团队**：rust-architect（Rust 技术架构）、req-analyst（需求分析）、observability-expert（可观测性/LLM 工程）

**共发现 22 个问题**（1 Critical, 10 Major, 4 Minor），本版已全部修复：

1. **SSE 计时语义**（Critical → 已修复）：明确定义 SSE 场景下 `upstream_total_ms` 和 `upstream_ttfb_ms` 的语义
2. **`parse_model_ms` 语义**（Major → 已修复）：在注释中明确包含两步操作
3. **错误路径误导**（Major → 已修复）：增加 `completed: bool` 字段
4. **Tier 2 span 父子关系**（Major → 已修复）：增加 `span_id` + `parent_id`
5. **`?slow=true` 阈值**（Major → 已修复）：明确基于 `upstream_total_ms > slow_threshold_ms`
6. **`?format=summary` 生成方式**（Major → 已修复）：明确为模板渲染，非 LLM 调用
7. **`write_request_journal` 签名**（Major → 已修复）：建议用 `TimingContext` 结构体封装
8. **路由决策上下文**（Major → 已修复）：增加 `selection_reason` + `available_providers`
9. **`#[instrument]` async 兼容性**（Major → 已修复）：增加版本约束说明
10. **Tier 1 开关**（Major → 已修复）：增加 `TimingConfig.enabled`
11. **采样策略**（Major → 已修复）：从固定头部采样改为混合采样（慢请求/错误 100% + 正常请求低采样）
12. **Phase 4 验收标准量化**（Major → 已修复）：增加测试方法和并发指标

**团队共识**：Tier 1 修复后可立即实施；Tier 2 增量价值有限（<20%），建议等 Tier 1 验证后再决定。

### 第四版修订记录（代码验证 + 边界审查）

基于对实际代码库的逐行验证，本版修复 9 个问题（1 Critical, 4 Major, 4 Minor）：

1. **SSE upstream_total_ms 计时边界**（Critical → 已修复）：明确定义为"上游最后一个 chunk 到达 proxy"，而非含客户端转发时间
2. **Per-retry 耗时分解缺失**（Major → 已修复）：增加 `retry_providers: Vec<String>` + `retry_durations_ms: Vec<u64>`，与 `failover_chain` 等长
3. **selection_reason 格式未标准化**（Major → 已修复）：定义 `pinned:|rule:|strategy:` 前缀规范，最大 128 字节，伪代码补充 rule 路径
4. **前置失败路径未覆盖**（Major → 已修复）：增加完整状态机表格，覆盖 body collect/extract_model/无可用 provider 等场景
5. **available_providers: u8 溢出风险**（Major → 已修复）：改为 `u16`
6. **parse_model_ms 合并权衡未标注**（Minor → 已修复）：标注为已知权衡并说明理由
7. **Tier 1 + Tier 2 数据冗余未说明**（Minor → 已修复）：补充冗余说明，明确为设计意图
8. **format=summary 实现方式模糊**（Minor → 已修复）：明确为 `format!()` 字符串拼接，无额外依赖
9. **UI 向后兼容未提及**（Minor → 已修复）：验收标准增加"timing: None 时优雅降级"

额外修正：`LogBuffer` → `RequestLogBuffer`（与实际代码一致）、`write_request_journal` 参数计数 16 个（非 14+）。

---

## 8. 最终建议

| 层级 | 状态 | 时机 | 预期收益 |
|------|------|------|---------|
| **Tier 1: timing 字段** | ✅ 推荐立即实施 | Phase 1 | 耗时分解可见，调试效率显著提升 |
| **Tier 2: 完整追踪** | ⚠️ 按需实施 | Phase 2-4 | 深度调试能力，但需评估实际需求 |

**核心原则**：先做轻量的、确定有价值的（Tier 1），再根据实际需求决定是否投入完整追踪（Tier 2）。
