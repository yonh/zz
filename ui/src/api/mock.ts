import type {
  Provider,
  SystemStats,
  LogEntry,
  RoutingConfig,
  ModelRule,
  TimeSeriesPoint,
  TrafficEntry,
} from "./types";

/**
 * Mock providers data.
 */
export const mockProviders: Provider[] = [
  {
    name: "ali-account-1",
    base_url: "https://dashscope.aliyuncs.com/compatible-mode",
    api_key: "sk-abcd1234xxxxxxxxxxxx",
    priority: 1,
    weight: 50,
    enabled: true,
    models: ["qwen-plus", "qwen-turbo", "qwen-max"],
    status: "healthy",
    cooldown_until: null,
    consecutive_failures: 0,
    stats: {
      total_requests: 5432,
      total_errors: 12,
      error_rate: 0.22,
      avg_latency_ms: 1200,
      latency_history: [980, 1100, 1050, 1300, 1200, 1150, 900, 1400, 1100, 1000, 1250, 1300],
    },
  },
  {
    name: "zhipu-account-1",
    base_url: "https://open.bigmodel.cn/api/paas/v4",
    api_key: "sk-efgh5678xxxxxxxxxxxx",
    priority: 2,
    weight: 30,
    enabled: true,
    models: ["glm-4", "glm-4-flash", "glm-4-plus"],
    status: "healthy",
    cooldown_until: null,
    consecutive_failures: 0,
    stats: {
      total_requests: 3215,
      total_errors: 5,
      error_rate: 0.16,
      avg_latency_ms: 800,
      latency_history: [750, 820, 780, 900, 850, 700, 810, 770, 830, 800, 750, 790],
    },
  },
  {
    name: "ali-account-2",
    base_url: "https://dashscope.aliyuncs.com/compatible-mode",
    api_key: "sk-ijkl9012xxxxxxxxxxxx",
    priority: 3,
    weight: 20,
    enabled: true,
    models: ["qwen-plus", "qwen-turbo"],
    status: "cooldown",
    cooldown_until: "2026-03-21T13:15:00Z",
    consecutive_failures: 3,
    stats: {
      total_requests: 2100,
      total_errors: 45,
      error_rate: 2.14,
      avg_latency_ms: 1500,
      latency_history: [1200, 1300, 1400, 1600, 1800, 2000, 1500, 1700, 1900, 1400, 1600, 1500],
    },
  },
  {
    name: "deepseek-1",
    base_url: "https://api.deepseek.com",
    api_key: "sk-mnop3456xxxxxxxxxxxx",
    priority: 4,
    weight: 0,
    enabled: false,
    models: ["deepseek-chat", "deepseek-coder"],
    status: "disabled",
    cooldown_until: null,
    consecutive_failures: 0,
    stats: {
      total_requests: 0,
      total_errors: 0,
      error_rate: 0,
      avg_latency_ms: 0,
      latency_history: [],
    },
  },
];

/**
 * Mock system stats.
 */
export const mockSystemStats: SystemStats = {
  total_requests: 10747,
  requests_per_minute: 23.5,
  active_providers: 3,
  healthy_providers: 2,
  total_providers: 4,
  strategy: "failover",
  uptime_secs: 86400,
};

/**
 * Mock request rate time series (last 60 minutes).
 */
export function generateRequestRateData(): TimeSeriesPoint[] {
  const now = new Date();
  const data: TimeSeriesPoint[] = [];
  for (let i = 59; i >= 0; i--) {
    const t = new Date(now.getTime() - i * 60000);
    const hour = t.getHours();
    const minute = t.getMinutes();
    const base = hour >= 9 && hour <= 18 ? 25 : 8;
    const noise = Math.floor(Math.random() * 15) - 5;
    data.push({
      time: `${String(hour).padStart(2, "0")}:${String(minute).padStart(2, "0")}`,
      value: Math.max(0, base + noise),
    });
  }
  return data;
}

/**
 * Mock traffic distribution.
 */
export const mockTrafficData: TrafficEntry[] = [
  { provider: "ali-account-1", requests: 5432, percentage: 50.5, color: "hsl(var(--color-chart-1))" },
  { provider: "zhipu-account-1", requests: 3215, percentage: 29.9, color: "hsl(var(--color-chart-2))" },
  { provider: "ali-account-2", requests: 2100, percentage: 19.6, color: "hsl(var(--color-chart-3))" },
];

/**
 * Mock log entries.
 */
export function generateMockLogs(count: number = 50): LogEntry[] {
  const providers = ["ali-account-1", "zhipu-account-1", "ali-account-2"];
  const models = ["qwen-plus", "qwen-turbo", "glm-4", "glm-4-flash"];
  const paths = ["/v1/chat/completions", "/v1/embeddings", "/v1/completions"];
  const logs: LogEntry[] = [];

  const now = Date.now();
  for (let i = 0; i < count; i++) {
    const ts = new Date(now - i * 3000 - Math.random() * 2000);
    const isError = Math.random() < 0.08;
    const isFailover = Math.random() < 0.05;
    const provider = providers[Math.floor(Math.random() * providers.length)];

    logs.push({
      id: `req_${String(i).padStart(6, "0")}`,
      timestamp: ts.toISOString(),
      method: "POST",
      path: paths[Math.floor(Math.random() * paths.length)],
      provider: isFailover ? providers[1] : provider,
      status: isError ? (Math.random() > 0.5 ? 429 : 500) : 200,
      duration_ms: Math.floor(800 + Math.random() * 3000),
      ttfb_ms: Math.floor(200 + Math.random() * 800),
      model: models[Math.floor(Math.random() * models.length)],
      streaming: Math.random() > 0.3,
      request_bytes: Math.floor(500 + Math.random() * 3000),
      response_bytes: Math.floor(1000 + Math.random() * 10000),
      failover_chain: isFailover
        ? [`${providers[0]}:429`, `${providers[1]}:200`]
        : null,
    });
  }
  return logs;
}

/**
 * Mock routing config.
 */
export const mockRoutingConfig: RoutingConfig = {
  strategy: "failover",
  max_retries: 3,
  cooldown_secs: 60,
  failure_threshold: 3,
  recovery_secs: 600,
};

/**
 * Mock model routing rules.
 */
export const mockModelRules: ModelRule[] = [
  { id: "rule-1", pattern: "qwen-*", target_provider: "ali-account-1" },
  { id: "rule-2", pattern: "glm-*", target_provider: "zhipu-account-1" },
];

/**
 * Mock TOML config string.
 */
export const mockConfigToml = `[server]
listen = "127.0.0.1:9090"
request_timeout_secs = 300
log_level = "info"

[routing]
strategy = "failover"
retry_on_failure = true
max_retries = 3

[health]
failure_threshold = 3
recovery_secs = 600
cooldown_secs = 60

[[providers]]
name = "ali-account-1"
base_url = "https://dashscope.aliyuncs.com/compatible-mode"
api_key = "sk-abcd1234xxxxxxxxxxxx"
priority = 1
weight = 50
models = ["qwen-plus", "qwen-turbo", "qwen-max"]

[[providers]]
name = "zhipu-account-1"
base_url = "https://open.bigmodel.cn/api/paas/v4"
api_key = "sk-efgh5678xxxxxxxxxxxx"
priority = 2
weight = 30
models = ["glm-4", "glm-4-flash", "glm-4-plus"]

[[providers]]
name = "ali-account-2"
base_url = "https://dashscope.aliyuncs.com/compatible-mode"
api_key = "sk-ijkl9012xxxxxxxxxxxx"
priority = 3
weight = 20
models = ["qwen-plus", "qwen-turbo"]

[[providers]]
name = "deepseek-1"
base_url = "https://api.deepseek.com"
api_key = "sk-mnop3456xxxxxxxxxxxx"
priority = 4
weight = 0
enabled = false
models = ["deepseek-chat", "deepseek-coder"]
`;
