import type {
  Provider,
  ProviderInput,
  RoutingConfig,
  ModelRule,
  SystemStats,
  TimeSeriesPoint,
  LogEntry,
  RequestJournalEntry,
  RequestJournalListResponse,
  RequestJournalQuery,
  RequestJournalStatus,
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

function buildQueryString(params: Record<string, string | number | undefined>): string {
  const parts: string[] = [];
  for (const [key, value] of Object.entries(params)) {
    if (value !== undefined) {
      parts.push(`${encodeURIComponent(key)}=${encodeURIComponent(String(value))}`);
    }
  }
  return parts.length > 0 ? `?${parts.join("&")}` : "";
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

  getRequestJournalStatus: () =>
    apiFetch<RequestJournalStatus>("/request-journal/status"),
  getRequestJournal: (query: RequestJournalQuery & { offset?: number; limit?: number }) => {
    const qs = buildQueryString({
      client: query.client,
      provider: query.provider,
      model: query.model,
      status: query.status,
      path: query.path,
      date: query.date,
      offset: query.offset,
      limit: query.limit,
    });
    return apiFetch<RequestJournalListResponse>(`/request-journal${qs}`);
  },
  getRequestJournalEntry: (id: string) =>
    apiFetch<RequestJournalEntry>(`/request-journal/${id}`),
  exportRequestJournal: (query: RequestJournalQuery) => {
    const qs = buildQueryString({
      client: query.client,
      provider: query.provider,
      model: query.model,
      status: query.status,
      path: query.path,
      date: query.date,
    });
    window.open(`${API_BASE}/request-journal/export${qs}`, "_blank");
  },
};
