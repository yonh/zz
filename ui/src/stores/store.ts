import { create } from "zustand";
import type {
  Provider,
  SystemStats,
  LogEntry,
  RoutingConfig,
  ModelRule,
  RoutingStrategy,
  ProviderStatus,
} from "@/api/types";
import {
  mockProviders,
  mockSystemStats,
  generateMockLogs,
  mockRoutingConfig,
  mockModelRules,
  mockConfigToml,
} from "@/api/mock";

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

  setStrategy: (strategy: RoutingStrategy) => void;
  toggleProvider: (name: string) => void;
  updateProviderPriority: (name: string, priority: number) => void;
  updateProviderWeight: (name: string, weight: number) => void;
  setRoutingConfig: (config: Partial<RoutingConfig>) => void;
  addModelRule: (rule: ModelRule) => void;
  removeModelRule: (id: string) => void;
  toggleDarkMode: () => void;
  addLog: (log: LogEntry) => void;
  reorderProviders: (orderedNames: string[]) => void;
  incrementStats: (success: boolean) => void;
  updateProviderStatus: (name: string, status: ProviderStatus, cooldownUntil: string | null) => void;
  updateProviderStats: (name: string, delta: { addRequest?: boolean; addError?: boolean; latency?: number }) => void;
  updateProvider: (name: string, updates: Partial<Provider>) => void;
}

export const useAppStore = create<AppState>((set) => ({
  providers: mockProviders,
  systemStats: mockSystemStats,
  logs: generateMockLogs(50),
  routingConfig: mockRoutingConfig,
  modelRules: mockModelRules,
  configToml: mockConfigToml,
  darkMode: getInitialDarkMode(),

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
    set((state) => ({
      logs: [log, ...state.logs].slice(0, 1000),
    })),

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
}));
