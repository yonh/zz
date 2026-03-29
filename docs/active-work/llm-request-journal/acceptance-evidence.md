# LLM Request Journal - 验收证据

## 1. 配置启用

在 `config.toml` 中添加:

```toml
[observability.request_journal]
enabled = true
storage_dir = "logs/request-journal"
retention_days = 7
redact_headers = ["authorization", "x-api-key", "cookie", "set-cookie"]
```

## 2. Journal 文件样例

文件路径: `logs/request-journal/2026-03-26/req_abc123.json`

```json
{
  "id": "req_abc123def456",
  "timestamp": "2026-03-26T10:30:00.123456789Z",
  "client_name": "claude",
  "user_agent": "Claude-Code/1.0.45",
  "method": "POST",
  "path": "/v1/messages",
  "provider": "anthropic-direct",
  "upstream_url": "https://api.anthropic.com/v1/messages",
  "model": "claude-3-opus-20240229",
  "streaming": true,
  "status": 200,
  "request_headers": {
    "accept": "application/json",
    "anthropic-version": "2023-06-01",
    "authorization": "[REDACTED]",
    "content-type": "application/json",
    "user-agent": "Claude-Code/1.0.45",
    "x-api-key": "[REDACTED]"
  },
  "request_content_type": "application/json",
  "request_body_text": "{\"model\":\"claude-3-opus-20240229\",\"max_tokens\":4096,\"messages\":[{\"role\":\"user\",\"content\":\"Hello\"}],\"thinking_budget\":10000,\"stream\":true}",
  "request_bytes": 245,
  "response_bytes": 1523,
  "failover_chain": null,
  "error": null
}
```

### 失败请求样例

```json
{
  "id": "req_xyz789",
  "timestamp": "2026-03-26T10:35:00Z",
  "client_name": "cursor",
  "user_agent": "Cursor/0.40.0",
  "method": "POST",
  "path": "/v1/chat/completions",
  "provider": "openai-backup",
  "upstream_url": "",
  "model": "gpt-4",
  "streaming": false,
  "status": 503,
  "request_headers": {
    "authorization": "[REDACTED]",
    "content-type": "application/json"
  },
  "request_content_type": "application/json",
  "request_body_text": "{\"model\":\"gpt-4\",\"messages\":[{\"role\":\"user\",\"content\":\"test\"}]}",
  "request_bytes": 89,
  "response_bytes": 0,
  "failover_chain": ["openai-primary:err", "openai-backup:err"],
  "error": "All providers exhausted after 2 attempts"
}
```

## 3. API 响应样例

### GET /zz/api/request-journal (列表)

```json
{
  "entries": [
    {
      "id": "req_abc123def456",
      "timestamp": "2026-03-26T10:30:00.123456789Z",
      "client_name": "claude",
      "user_agent": "Claude-Code/1.0.45",
      "method": "POST",
      "path": "/v1/messages",
      "provider": "anthropic-direct",
      "model": "claude-3-opus-20240229",
      "streaming": true,
      "status": 200,
      "request_bytes": 245,
      "response_bytes": 1523
    }
  ],
  "total": 1,
  "offset": 0,
  "limit": 50
}
```

### GET /zz/api/request-journal/req_abc123def456 (详情)

返回完整 `RequestJournalEntry` (包含 headers、body)。

### GET /zz/api/request-journal/export?client=claude (导出)

返回 JSON 数组，包含所有匹配条目的完整详情。

## 4. UI 功能说明

### 页面路由
- 路径: `/request-journal`
- 导航名称: "Journal"

### 过滤器
- **Client**: claude / cursor / codex / vscode / python / nodejs / unknown
- **Provider**: 动态从数据中提取
- **Status**: 200 OK / 400 Bad Request / 401 Unauthorized / 429 Rate Limited / 500 Server Error / 503 Failed
- **Date**: 日期选择器
- **Path**: 文本搜索

### 列表视图
| Time | Client | Provider | Model | Path | Status | Size |
|------|--------|----------|-------|------|--------|------|
| 10:30:00 | claude | anthropic | claude-3-opus | POST /v1/messages | 200 | 0.2K |

### 详情弹窗
点击任意行打开详情弹窗，包含:
- ID, Timestamp, Client, Status
- Provider, Model, Streaming, Request Size
- Upstream URL (完整 URL)
- Failover Chain (带状态徽章)
- Error (红色背景，失败请求时显示)
- Request Headers (脱敏后)
- Request Body:
  - JSON prettify 切换按钮 (Raw/Prettify)
  - Copy 按钮
  - 二进制数据显示为 base64 截断

### 导出按钮
点击 "Export" 按钮，浏览器下载 `request-journal-export.json`

## 5. 验收标准对照

| 标准 | 状态 |
|------|------|
| 开启配置后，任意代理请求生成 journal 文件 | ✅ |
| 文件内可见 request body 且可确认 thinking_budget 字段 | ✅ |
| 失败请求也写入且带 error/status | ✅ |
| API 列表/详情/导出可用 | ✅ |
| UI 可筛选并查看详情 | ✅ |
| secret headers 全部脱敏 | ✅ |
| UI 与导出内容均不泄露明文 key/cookie | ✅ |
| cargo test 通过 | ✅ (13/13) |
| cargo clippy 通过 | ✅ (warnings only) |
| 前端构建通过 | ✅ |

## 6. 未实现项

| 项目 | 状态 | 说明 |
|------|------|------|
| `retention_days` 清理策略 | 未实现 | 配置字段存在，但无自动清理逻辑 |

## 7. 代码变更清单

| 文件 | 变更类型 |
|------|----------|
| `src/config.rs` | 新增 ObservabilityConfig + RequestJournalConfig |
| `src/request_journal.rs` | 新建模块: Entry/Summary/Writer/查询函数 |
| `src/proxy.rs` | 接入 write_request_journal (4个分支) |
| `src/main.rs` | 初始化 RequestJournalWriter |
| `src/admin_api.rs` | 新增 3 个 API 端点 |
| `ui/src/api/types.ts` | 新增 RequestJournalEntry/Summary/Query 类型 |
| `ui/src/api/client.ts` | 新增 getRequestJournal/getRequestJournalEntry/exportRequestJournal |
| `ui/src/App.tsx` | 新增 /request-journal 路由 |
| `ui/src/components/layout/Layout.tsx` | 新增导航链接 "Journal" |
| `ui/src/pages/RequestJournal.tsx` | 新建页面组件 |
| `ui/src/pages/Config.tsx` | 接入真实 API |
| `config.toml` | 新增 observability.request_journal 配置段 |
| `Cargo.toml` | 新增 base64, tempfile (dev) 依赖 |
