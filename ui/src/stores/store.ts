import { create } from "zustand";
import type {
  Provider,
  ProviderInput,
  SystemStats,
  LogEntry,
  RoutingConfig,
  ModelRule,
  RoutingStrategy,
  ProviderStatus,
} from "@/api/types";
import { api } from "@/api/client";

/**
 * Empty default values for initial state before API data loads.
 */
const defaultSystemStats: SystemStats = {
  total_requests: 0,
  requests_per_minute: 0,
  active_providers: 0,
  healthy_providers: 0,
  total_providers: 0,
  strategy: "failover",
  uptime_secs: 0,
  tokens: {
    prompt: 0,
    completion: 0,
    total: 0,
  },
};

const defaultRoutingConfig: RoutingConfig = {
  strategy: "failover",
  max_retries: 3,
  cooldown_secs: 60,
  failure_threshold: 3,
  recovery_secs: 600,
};

/**
 * Read persisted dark mode preference from localStorage.
 */
function getInitialDarkMode(): boolean {
  try {
    const stored = localStorage.getItem("zz-dark-mode");
    if (stored !== null) return stored === "true";
  } catch { /* noop */ }
  return true;
}

/**
 * Global application state store.
 */
interface AppState {
  providers: Provider[];
  systemStats: SystemStats;
  logs: LogEntry[];
  routingConfig: RoutingConfig;
  modelRules: ModelRule[];
  configToml: string;
  darkMode: boolean;
  loading: boolean;
  error: string | null;

  setStrategy: (strategy: RoutingStrategy) => void;
  toggleProvider: (name: string) => void;
  updateProviderPriority: (name: string, priority: number) => void;
  updateProviderWeight: (name: string, weight: number) => void;
  setRoutingConfig: (config: Partial<RoutingConfig>) => void;
  setPinnedProvider: (name: string) => void;
  addModelRule: (rule: ModelRule) => void;
  removeModelRule: (id: string) => void;
  toggleDarkMode: () => void;
  addLog: (log: LogEntry) => void;
  reorderProviders: (orderedNames: string[]) => void;
  incrementStats: (success: boolean) => void;
  updateProviderStatus: (name: string, status: ProviderStatus, cooldownUntil: string | null) => void;
  updateProviderStats: (name: string, delta: { addRequest?: boolean; addError?: boolean; latency?: number }) => void;
  updateProvider: (name: string, updates: ProviderInput) => void;
  addProvider: (provider: ProviderInput) => void;
  removeProvider: (name: string) => void;
  setSystemStats: (stats: SystemStats) => void;
  setLogs: (logs: LogEntry[]) => void;
  initFromApi: () => Promise<void>;
}

export const useAppStore = create<AppState>((set) => ({
  providers: [],
  systemStats: defaultSystemStats,
  logs: [],
  routingConfig: defaultRoutingConfig,
  modelRules: [],
  configToml: "",
  darkMode: getInitialDarkMode(),
  loading: true,
  error: null,

  setStrategy: (strategy) =>
    set((state) => ({
      routingConfig: { ...state.routingConfig, strategy },
      systemStats: { ...state.systemStats, strategy },
    })),

  toggleProvider: (name) =>
    set((state) => ({
      providers: state.providers.map((p) =>
        p.name === name
          ? {
              ...p,
              enabled: !p.enabled,
              status: !p.enabled ? "healthy" : "disabled",
            }
          : p
      ),
    })),

  updateProviderPriority: (name, priority) =>
    set((state) => ({
      providers: state.providers.map((p) =>
        p.name === name ? { ...p, priority } : p
      ),
    })),

  updateProviderWeight: (name, weight) =>
    set((state) => ({
      providers: state.providers.map((p) =>
        p.name === name ? { ...p, weight } : p
      ),
    })),

  setRoutingConfig: (config) =>
    set((state) => ({
      routingConfig: { ...state.routingConfig, ...config },
    })),

  setPinnedProvider: (name) =>
    set((state) => ({
      routingConfig: { ...state.routingConfig, pinned_provider: name },
    })),

  addModelRule: (rule) =>
    set((state) => ({
      modelRules: [...state.modelRules, rule],
    })),

  removeModelRule: (id) =>
    set((state) => ({
      modelRules: state.modelRules.filter((r) => r.id !== id),
    })),

  toggleDarkMode: () =>
    set((state) => {
      const next = !state.darkMode;
      if (next) {
        document.documentElement.classList.add("dark");
      } else {
        document.documentElement.classList.remove("dark");
      }
      try { localStorage.setItem("zz-dark-mode", String(next)); } catch { /* noop */ }
      return { darkMode: next };
    }),

  addLog: (log) =>
    set((state) => {
      // Skip if log with same ID already exists
      if (state.logs.some((l) => l.id === log.id)) {
        return state;
      }
      return { logs: [log, ...state.logs].slice(0, 1000) };
    }),

  reorderProviders: (orderedNames) =>
    set((state) => ({
      providers: orderedNames.map((name, idx) => {
        const p = state.providers.find((pr) => pr.name === name)!;
        return { ...p, priority: idx + 1 };
      }),
    })),

  incrementStats: (success) =>
    set((state) => ({
      systemStats: {
        ...state.systemStats,
        total_requests: state.systemStats.total_requests + 1,
        requests_per_minute: state.systemStats.requests_per_minute + (Math.random() * 0.4 - 0.2),
        healthy_providers: state.providers.filter((p) => p.status === "healthy").length,
        active_providers: state.providers.filter((p) => p.enabled).length,
        ...(success ? {} : {}),
      },
    })),

  updateProviderStatus: (name, status, cooldownUntil) =>
    set((state) => ({
      providers: state.providers.map((p) =>
        p.name === name
          ? {
              ...p,
              status,
              cooldown_until: cooldownUntil,
              consecutive_failures: status === "healthy" ? 0 : p.consecutive_failures,
            }
          : p
      ),
    })),

  updateProviderStats: (name, delta) =>
    set((state) => ({
      providers: state.providers.map((p) => {
        if (p.name !== name) return p;
        const newReqs = p.stats.total_requests + (delta.addRequest ? 1 : 0);
        const newErrs = p.stats.total_errors + (delta.addError ? 1 : 0);
        const newLatencyHistory = delta.latency
          ? [...p.stats.latency_history.slice(-11), delta.latency]
          : p.stats.latency_history;
        const avgLat = delta.latency
          ? Math.round((p.stats.avg_latency_ms * 0.9) + (delta.latency * 0.1))
          : p.stats.avg_latency_ms;
        return {
          ...p,
          stats: {
            total_requests: newReqs,
            total_errors: newErrs,
            error_rate: newReqs > 0 ? (newErrs / newReqs) * 100 : 0,
            avg_latency_ms: avgLat,
            latency_history: newLatencyHistory,
            prompt_tokens: p.stats.prompt_tokens,
            completion_tokens: p.stats.completion_tokens,
            total_tokens: p.stats.total_tokens,
          },
        };
      }),
    })),

  updateProvider: (name, updates) =>
    set((state) => ({
      providers: state.providers.map((p) =>
        p.name === name ? { ...p, ...updates } : p
      ),
    })),

  addProvider: (provider) =>
    set((state) => {
      const newProvider: Provider = {
        name: provider.name || "",
        base_url: provider.base_url || "",
        api_key_masked: "****",
        priority: provider.priority || 1,
        weight: provider.weight || 50,
        enabled: provider.enabled ?? true,
        models: provider.models || [],
        status: "healthy",
        cooldown_until: null,
        consecutive_failures: 0,
        stats: {
          total_requests: 0,
          total_errors: 0,
          error_rate: 0,
          avg_latency_ms: 0,
          latency_history: [],
          prompt_tokens: 0,
          completion_tokens: 0,
          total_tokens: 0,
        },
        headers: provider.headers,
        token_budget: provider.token_budget,
      };
      return {
        providers: [...state.providers, newProvider],
        systemStats: {
          ...state.systemStats,
          total_providers: state.systemStats.total_providers + 1,
          active_providers: newProvider.enabled
            ? state.systemStats.active_providers + 1
            : state.systemStats.active_providers,
        },
      };
    }),

  removeProvider: (name) =>
    set((state) => ({
      providers: state.providers.filter((p) => p.name !== name),
      systemStats: {
        ...state.systemStats,
        total_providers: state.systemStats.total_providers - 1,
        active_providers: state.providers.find((p) => p.name === name)?.enabled
          ? state.systemStats.active_providers - 1
          : state.systemStats.active_providers,
      },
    })),

  setSystemStats: (stats) => set((state) => ({
    systemStats: {
      ...defaultSystemStats,
      ...state.systemStats,
      ...stats,
      tokens: {
        ...defaultSystemStats.tokens,
        ...(state.systemStats.tokens || {}),
        ...(stats.tokens || {}),
      },
    },
  })),

  setLogs: (logs) => set({ logs }),

  initFromApi: async () => {
    set({ loading: true, error: null });
    try {
      const [providersRes, stats, routing, rulesRes, configRes, logsRes] = await Promise.all([
        api.getProviders(),
        api.getStats(),
        api.getRouting(),
        api.getRules(),
        api.getConfig(),
        api.getLogs(0, 100),
      ]);
      set({
        providers: providersRes.providers,
        systemStats: {
          ...defaultSystemStats,
          ...stats,
          tokens: {
            ...defaultSystemStats.tokens,
            ...(stats.tokens || {}),
          },
        },
        routingConfig: routing,
        modelRules: rulesRes.rules,
        configToml: configRes.content,
        logs: logsRes.logs,
        loading: false,
      });
    } catch (err) {
      const message = err instanceof Error ? err.message : "Failed to connect to backend";
      console.error("Failed to initialize from API:", err);
      set({ loading: false, error: message });
    }
  },
}));
