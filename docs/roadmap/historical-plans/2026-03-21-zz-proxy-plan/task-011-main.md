---
status: deferred
horizon: long_term
workflow_stage: archived
next_command: /route-task-by-status
last_reviewed: 2026-03-22
---

# 任务 011：主入口 - 服务启动

## 目标

实现主入口，包括 CLI 参数解析与 HTTP 服务启动。

## BDD 场景

```gherkin
Scenario: Start server on configured address
  Given config.toml has server.listen = "127.0.0.1:9090"
  When zz is started with default config path
  Then server listens on 127.0.0.1:9090
  And responds to HTTP requests

Scenario: Override config path via CLI
  Given CLI argument --config /tmp/custom.toml
  When zz is started
  Then loads config from /tmp/custom.toml instead of default

Scenario: Override listen address via CLI
  Given CLI argument --listen 0.0.0.0:8080
  When zz is started
  Then server listens on 0.0.0.0:8080 (overrides config)

Scenario: Show help message
  Given CLI argument --help
  When zz is started
  Then prints usage information
  And exits with code 0

Scenario: Route admin and proxy paths
  Given request path = /zz/health
  When request arrives at server
  Then handled by admin router
  Given request path = /v1/chat/completions
  When request arrives at server
  Then handled by proxy router

Scenario: Graceful shutdown on SIGINT
  Given server is running
  When user presses Ctrl+C
  Then server stops accepting new connections
  And waits for in-flight requests to complete
  And exits cleanly
```

## 涉及文件

**修改**：
- `src/main.rs` - 完整实现

## 历史实施步骤

1. 使用 clap 增加 CLI 参数解析：
   ```rust
   #[derive(Parser)]
   struct Args {
       #[arg(short, long, default_value = "config.toml")]
       config: String,

       #[arg(short, long)]
       listen: Option<String>,

       #[arg(short, long, default_value = "info")]
       log_level: Option<String>,
   }
   ```

2. 实现 main 函数：
   - 解析 CLI 参数
   - 加载配置（必要时用 CLI 参数覆盖）
   - 初始化日志
   - 根据配置创建 ProviderManager
   - 根据配置创建 Router
   - 构建共享状态（`Arc<AppState>`）

3. 创建 HTTP 服务：
   - 使用 `hyper_util::server::conn::auto::Builder` 支持 HTTP/1.1 与 HTTP/2
   - 使用 `tokio::net::TcpListener` 绑定地址
   - 实现 service fn：
     - 先检查 admin 路径（`admin::handle_admin_request`）
     - 未命中时回退到 `proxy::proxy_handler`
   - 启动服务任务，并支持优雅关闭

4. 实现请求处理入口：
   ```rust
   async fn handle_request(req: Request<Incoming>, state: AppState) -> Result<Response<...>> {
       if let Some(resp) = admin::handle_request(&req) {
           return Ok(resp);
       }
       proxy::proxy_handler(req, state).await
   }
   ```

5. 增加 graceful shutdown：
   - 监听 SIGINT / SIGTERM
   - 停止接收新连接
   - 等待正在处理的请求完成
   - 平滑关闭 tokio runtime

## 历史验证方式

运行：

```bash
cargo run -- --help
cargo run 2>&1 | grep "Listening"
```

预期：
- help 信息显示正确
- 服务成功启动并绑定到配置地址
- 启动过程无 panic

## 依赖
- 任务 008（Proxy handler）
- 任务 009（Admin 端点）
- 任务 010（日志初始化）
