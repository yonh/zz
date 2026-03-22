# ZZ - LLM API 反向代理与自动故障转移

[![Rust](https://img.shields.io/badge/rust-1.82+-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

一个轻量级、高性能的LLM API反向代理，用Rust编写，支持多提供商自动故障转移和负载均衡。

## 🎯 核心功能

- **🔄 自动故障转移**：当配额用完或服务不可用时，自动切换到下一个提供商
- **⚖️ 多种负载均衡策略**：支持故障转移、轮询、加权随机、配额感知等策略
- **📊 实时监控面板**：Web界面实时查看请求统计、提供商状态和日志
- **🔧 热重载配置**：无需重启即可更新配置和提供商
- **🌐 模型路由规则**：基于模型名称的智能路由
- **📈 Token统计**：详细的Token使用量跟踪（支持OpenAI/Anthropic格式）
- **🔍 请求日志**：完整的请求响应日志记录

## 🚀 快速开始

### 1. 编译

```bash
git clone https://github.com/your-repo/zz.git
cd zz
cargo build --release
```

### 2. 配置

创建配置文件 `config.toml`：

```toml
[server]
listen = "127.0.0.1:9090"
log_level = "info"

[routing]
strategy = "failover"
max_retries = 3

[health]
failure_threshold = 3
recovery_secs = 600
cooldown_secs = 60

[[providers]]
name = "ali-account-1"
base_url = "https://dashscope.aliyuncs.com/compatible-mode"
api_key = "sk-your-api-key"
priority = 1
models = ["qwen-plus", "qwen-turbo"]

[[providers]]
name = "zhipu-account-1"
base_url = "https://open.bigmodel.cn/api/paas/v4"
api_key = "sk-your-api-key"
priority = 2
models = ["glm-4", "glm-4-flash"]
```

### 3. 启动

```bash
./zz --config config.toml
```

服务将在 `http://127.0.0.1:9090` 启动。

### 4. 配置客户端

将你的AI工具（如Claude Code、Cursor等）的API端点设置为：
```
http://127.0.0.1:9090
```

API密钥使用任意值（ZZ会自动重写为正确的提供商密钥）。

## 📖 详细配置

### 服务器配置

```toml
[server]
listen = "127.0.0.1:9090"          # 监听地址
request_timeout_secs = 300         # 请求超时时间（秒）
log_level = "info"                  # 日志级别：trace, debug, info, warn, error
```

### 路由策略

```toml
[routing]
strategy = "failover"              # 路由策略
max_retries = 3                     # 最大重试次数
```

#### 支持的路由策略

| 策略 | 描述 | 适用场景 |
|------|------|----------|
| `failover` | 按优先级顺序，失败时切换 | 主备模式，高可用性 |
| `round-robin` | 轮询分配请求 | 负载均衡 |
| `weighted-random` | 按权重随机选择 | 不同容量的提供商 |
| `quota-aware` | 基于配额使用量选择 | 需要精确控制成本 |
| `manual` | 固定到指定提供商 | 测试或临时切换 |

### 健康检查配置

```toml
[health]
failure_threshold = 3                # 连续失败多少次后标记为不健康
recovery_secs = 600                  # 不健康状态恢复检查间隔（秒）
cooldown_secs = 60                   # 配额耗尽后的冷却时间（秒）
```

### 提供商配置

```toml
[[providers]]
name = "provider-name"              # 提供商名称（唯一）
base_url = "https://api.example.com" # API基础URL
api_key = "sk-your-api-key"          # API密钥
priority = 1                        # 优先级（数字越小优先级越高）
weight = 10                          # 权重（用于weighted-random策略）
enabled = true                       # 是否启用
models = ["model1", "model2"]        # 支持的模型列表（可选）
headers = { "X-Custom" = "value" }   # 额外请求头（可选）
```

## 🎛️ Web管理界面

启动服务后，访问 `http://127.0.0.1:9090/zz/ui/` 打开管理界面。

### 功能特性

- **概览面板**：实时统计、请求图表、流量分布
- **提供商管理**：添加、编辑、删除、测试提供商
- **路由配置**：切换路由策略、设置模型路由规则
- **日志查看**：实时请求日志、搜索过滤、Token使用统计
- **配置管理**：查看和编辑配置文件

### API接口

```bash
# 获取所有提供商
curl http://127.0.0.1:9090/zz/api/providers

# 添加新提供商
curl -X POST http://127.0.0.1:9090/zz/api/providers \
  -H "Content-Type: application/json" \
  -d '{"name": "new-provider", "base_url": "https://api.example.com", "api_key": "sk-new-key"}'

# 更新路由策略
curl -X PUT http://127.0.0.1:9090/zz/api/routing \
  -H "Content-Type: application/json" \
  -d '{"strategy": "round-robin"}'

# 获取系统统计
curl http://127.0.0.1:9090/zz/api/stats
```

## 🔧 高级功能

### 模型路由规则

为特定模型指定专属提供商：

```bash
curl -X PUT http://127.0.0.1:9090/zz/api/routing/rules \
  -H "Content-Type: application/json" \
  -d '{
    "rules": [
      {"pattern": "claude-*", "target_provider": "anthropic-account"},
      {"pattern": "gpt-*", "target_provider": "openai-account"}
    ]
  }'
```

支持glob模式匹配：`*` 匹配任意字符，`?` 匹配单个字符

### 故障检测机制

| 错误类型 | 检测方式 | 处理方式 |
|----------|----------|----------|
| HTTP 429 | 状态码 + 响应内容 | 冷却提供商，切换下一个 |
| HTTP 403 | 状态码 + 配额关键词 | 冷却提供商，切换下一个 |
| HTTP 5xx | 状态码 | 标记失败，切换下一个 |
| 连接超时 | 网络错误 | 标记失败，切换下一个 |

## 🛠️ 开发

```bash
# 启动后端开发服务器
cargo run

# 启动前端开发服务器（新终端）
cd ui && npm install && npm run dev
```

## 🔒 安全考虑

- **本地监听**：默认只监听127.0.0.1，避免暴露到公网
- **API密钥保护**：配置文件中的API密钥不会在日志中暴露
- **HTTPS支持**：上游请求使用HTTPS，确保传输安全

## 📄 许可证

本项目采用MIT许可证。详见 [LICENSE](LICENSE) 文件。

## 🆘 故障排除

**Q: 提供商显示为不健康状态**
A: 检查API密钥是否正确，网络连接是否正常，查看日志了解具体错误。

**Q: 请求失败但没有故障转移**
A: 确认路由策略配置正确，检查`max_retries`设置。

**Q: Token统计不准确**
A: 确认上游API返回了正确的usage字段。

启用调试日志：
```bash
RUST_LOG=debug ./zz --config config.toml
```