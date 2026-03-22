---
status: deferred
horizon: long_term
workflow_stage: archived
next_command: /route-task-by-status
last_reviewed: 2026-03-22
---

# 任务 005：重写模块 - URL 与 Header 重写

## 目标

实现 URL 与 Header 重写，将本地请求正确映射到上游 provider 端点。

## BDD 场景

```gherkin
Scenario: Rewrite URL with base_url + request path
  Given provider.base_url = "https://dashscope.aliyuncs.com/compatible-mode"
  And request path = "/v1/chat/completions"
  When rewrite_url() is called
  Then returns "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions"

Scenario: Rewrite Authorization header
  Given provider.api_key = "sk-xxxx"
  When rewrite_headers() is called
  Then Authorization header equals "Bearer sk-xxxx"

Scenario: Rewrite Host header from base_url
  Given provider.base_url = "https://dashscope.aliyuncs.com/compatible-mode"
  When rewrite_headers() is called
  Then Host header equals "dashscope.aliyuncs.com"

Scenario: Preserve existing headers except rewritten ones
  Given request has headers: User-Agent, Content-Type, Accept
  When rewrite_headers() is called
  Then all three headers are preserved
  And Authorization and Host are added/replaced

Scenario: Inject custom provider headers
  Given provider.headers = { "X-Custom" = "value" }
  When rewrite_headers() is called
  Then X-Custom header equals "value"
```

## 涉及文件

**创建**：
- `src/rewriter.rs` - 完整实现

## 历史实施步骤

1. 实现 URL 重写：
   - 解析 `base_url`，提取 host/port
   - 将 `base_url` 路径与请求 path 拼接
   - 处理末尾斜杠等边界情况
   - 使用 `url::Url` 做正确 URL 操作

2. 实现 Header 重写：
   - 替换 Authorization：`Bearer {api_key}`
   - 替换 Host：从 `base_url` 提取
   - 合并 provider 配置中的自定义 headers
   - 保留原请求中的其他 headers

3. 创建 `RequestRewriter` 结构：
   - `rewrite_request(&self, provider: &Provider, req: Request<Body>) -> Request<Body>`
   - 返回已重写 URL 与 headers 的请求

4. 处理边界情况：
   - 空 path（应直接使用 `base_url`）
   - query 参数（应保留）
   - fragment（应保留）

## 历史验证方式

运行：

```bash
cargo test --lib rewriter
```

预期：
- URL 拼接测试通过
- Header 替换测试通过
- 自定义 Header 注入测试通过

## 依赖
- 任务 004（Router 提供 Provider 结构）
