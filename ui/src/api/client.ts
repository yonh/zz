import type {
  Provider,
  ProviderInput,
  RoutingConfig,
  ModelRule,
  SystemStats,
  TimeSeriesPoint,
  LogEntry,
} from "./types";

const API_BASE = "/zz/api";

async function apiFetch<T>(path: string, options?: RequestInit): Promise<T> {
  const resp = await fetch(`${API_BASE}${path}`, {
    ...options,
    headers: { "Content-Type": "application/json", ...options?.headers },
  });
  if (!resp.ok) throw new Error(`API error: ${resp.status}`);
  if (resp.status === 204) return undefined as T;
  return resp.json();
}

export const api = {
  // Providers
  getProviders: () =>
    apiFetch<{ providers: Provider[] }>("/providers"),
  getProvider: (name: string) =>
    apiFetch<Provider>(`/providers/${name}`),
  addProvider: (data: ProviderInput) =>
    apiFetch<Provider>("/providers", { method: "POST", body: JSON.stringify(data) }),
  updateProvider: (name: string, data: ProviderInput) =>
    apiFetch<Provider>(`/providers/${name}`, { method: "PUT", body: JSON.stringify(data) }),
  deleteProvider: (name: string) =>
    apiFetch<void>(`/providers/${name}`, { method: "DELETE" }),
  testProvider: (name: string) =>
    apiFetch<{ success: boolean; latency_ms: number }>(`/providers/${name}/test`, { method: "POST" }),
  enableProvider: (name: string) =>
    apiFetch<void>(`/providers/${name}/enable`, { method: "POST" }),
  disableProvider: (name: string) =>
    apiFetch<void>(`/providers/${name}/disable`, { method: "POST" }),
  resetProvider: (name: string) =>
    apiFetch<void>(`/providers/${name}/reset`, { method: "POST" }),

  // Routing
  getRouting: () =>
    apiFetch<RoutingConfig>("/routing"),
  updateRouting: (config: Partial<RoutingConfig>) =>
    apiFetch<void>("/routing", { method: "PUT", body: JSON.stringify(config) }),
  getRules: () =>
    apiFetch<{ rules: ModelRule[] }>("/routing/rules"),
  updateRules: (rules: ModelRule[]) =>
    apiFetch<void>("/routing/rules", { method: "PUT", body: JSON.stringify({ rules }) }),

  // Stats
  getStats: () =>
    apiFetch<SystemStats>("/stats"),
  getTimeseries: () =>
    apiFetch<{ data: TimeSeriesPoint[] }>("/stats/timeseries"),
  getLogs: (offset = 0, limit = 100) =>
    apiFetch<{ logs: LogEntry[]; total: number }>(`/logs?offset=${offset}&limit=${limit}`),

  // Config
  getConfig: () =>
    apiFetch<{ content: string }>("/config"),
  updateConfig: (content: string) =>
    apiFetch<void>("/config", { method: "PUT", body: JSON.stringify({ content }) }),
  validateConfig: (content: string) =>
    apiFetch<{ valid: boolean; errors?: string[] }>("/config/validate", { method: "POST", body: JSON.stringify({ content }) }),

  // System
  getHealth: () =>
    apiFetch<{ status: string }>("/health"),
  getVersion: () =>
    apiFetch<{ version: string; build: string }>("/version"),
};
