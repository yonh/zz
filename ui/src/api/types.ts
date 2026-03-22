/**
 * Provider health status.
 */
export type ProviderStatus = "healthy" | "cooldown" | "unhealthy" | "disabled";

/**
 * Routing strategy type.
 */
export type RoutingStrategy =
  | "failover"
  | "round-robin"
  | "weighted-random"
  | "quota-aware"
  | "manual";

/**
 * Upstream provider configuration and runtime state.
 */
export interface Provider {
  name: string;
  base_url: string;
  api_key: string;
  priority: number;
  weight: number;
  enabled: boolean;
  models: string[];
  status: ProviderStatus;
  cooldown_until: string | null;
  consecutive_failures: number;
  stats: ProviderStats;
  headers?: Record<string, string>;
  token_budget?: number;
}

/**
 * Per-provider statistics.
 */
export interface ProviderStats {
  total_requests: number;
  total_errors: number;
  error_rate: number;
  avg_latency_ms: number;
  latency_history: number[];
}

/**
 * Aggregated system statistics.
 */
export interface SystemStats {
  total_requests: number;
  requests_per_minute: number;
  active_providers: number;
  healthy_providers: number;
  total_providers: number;
  strategy: RoutingStrategy;
  uptime_secs: number;
}

/**
 * Token usage information from API response.
 */
export interface TokenUsage {
  prompt_tokens: number;
  completion_tokens: number;
  total_tokens: number;
  details?: {
    cached_tokens?: number;
    reasoning_tokens?: number;
    cache_read_tokens?: number;
    cache_write_tokens?: number;
    [key: string]: number | undefined;
  };
}

/**
 * A single request log entry.
 */
export interface LogEntry {
  id: string;
  timestamp: string;
  method: string;
  path: string;
  provider: string;
  status: number;
  duration_ms: number;
  ttfb_ms: number;
  model: string;
  streaming: boolean;
  request_bytes: number;
  response_bytes: number;
  failover_chain: string[] | null;
  token_usage?: TokenUsage;
}

/**
 * Routing configuration.
 */
export interface RoutingConfig {
  strategy: RoutingStrategy;
  max_retries: number;
  cooldown_secs: number;
  failure_threshold: number;
  recovery_secs: number;
  pinned_provider?: string;
}

/**
 * Model routing rule.
 */
export interface ModelRule {
  id: string;
  pattern: string;
  target_provider: string;
}

/**
 * Time series data point for charts.
 */
export interface TimeSeriesPoint {
  time: string;
  value: number;
}

/**
 * Traffic distribution entry for charts.
 */
export interface TrafficEntry {
  provider: string;
  requests: number;
  percentage: number;
  color: string;
}
