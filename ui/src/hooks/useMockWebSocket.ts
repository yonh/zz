import { useEffect, useRef } from "react";
import { useAppStore } from "@/stores/store";
import type { LogEntry, ProviderStatus } from "@/api/types";

const PROVIDERS = ["ali-account-1", "zhipu-account-1", "ali-account-2"];
const MODELS = ["qwen-plus", "qwen-turbo", "glm-4", "glm-4-flash", "qwen-max"];
const PATHS = ["/v1/chat/completions", "/v1/embeddings", "/v1/completions"];

let logCounter = 1000;

/**
 * Generate a single random log entry simulating a real request.
 */
function generateRandomLog(): LogEntry {
  const provider = PROVIDERS[Math.floor(Math.random() * PROVIDERS.length)];
  const isError = Math.random() < 0.06;
  const isFailover = Math.random() < 0.04;

  logCounter++;
  return {
    id: `req_live_${logCounter}`,
    timestamp: new Date().toISOString(),
    method: "POST",
    path: PATHS[Math.floor(Math.random() * PATHS.length)],
    provider: isFailover ? PROVIDERS[1] : provider,
    status: isError ? (Math.random() > 0.5 ? 429 : 500) : 200,
    duration_ms: Math.floor(600 + Math.random() * 3500),
    ttfb_ms: Math.floor(150 + Math.random() * 900),
    model: MODELS[Math.floor(Math.random() * MODELS.length)],
    streaming: Math.random() > 0.25,
    request_bytes: Math.floor(400 + Math.random() * 4000),
    response_bytes: Math.floor(800 + Math.random() * 12000),
    failover_chain: isFailover
      ? [`${PROVIDERS[0]}:429`, `${PROVIDERS[1]}:200`]
      : null,
  };
}

/**
 * Hook that simulates a WebSocket connection pushing real-time data.
 * Pushes new logs, updates stats, and occasionally triggers provider state changes.
 */
export function useMockWebSocket() {
  const addLog = useAppStore((s) => s.addLog);
  const incrementStats = useAppStore((s) => s.incrementStats);
  const updateProviderStatus = useAppStore((s) => s.updateProviderStatus);
  const updateProviderStats = useAppStore((s) => s.updateProviderStats);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const stateIntervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  useEffect(() => {
    // Push new log every 2-5 seconds
    function scheduleNextLog() {
      const delay = 2000 + Math.random() * 3000;
      intervalRef.current = setTimeout(() => {
        const log = generateRandomLog();
        addLog(log);

        // Update per-provider stats
        updateProviderStats(log.provider, {
          addRequest: true,
          addError: log.status >= 400,
          latency: log.duration_ms,
        });

        // Update global stats
        incrementStats(log.status < 400);

        scheduleNextLog();
      }, delay) as unknown as ReturnType<typeof setInterval>;
    }

    scheduleNextLog();

    // Occasionally toggle provider states (every 15-30s)
    stateIntervalRef.current = setInterval(() => {
      const roll = Math.random();
      if (roll < 0.3) {
        // Simulate a provider going into cooldown
        const target = PROVIDERS[Math.floor(Math.random() * PROVIDERS.length)];
        const cooldownUntil = new Date(Date.now() + 60000).toISOString();
        updateProviderStatus(target, "cooldown", cooldownUntil);

        // Recover after 10-20 seconds (simulated)
        setTimeout(() => {
          updateProviderStatus(target, "healthy", null);
        }, 10000 + Math.random() * 10000);
      }
    }, 15000 + Math.random() * 15000);

    return () => {
      if (intervalRef.current) clearTimeout(intervalRef.current as unknown as number);
      if (stateIntervalRef.current) clearInterval(stateIntervalRef.current);
    };
  }, [addLog, incrementStats, updateProviderStatus, updateProviderStats]);
}
