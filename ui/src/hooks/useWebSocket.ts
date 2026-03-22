import { useEffect, useRef } from "react";
import { useAppStore } from "@/stores/store";
import type { LogEntry, SystemStats } from "@/api/types";

export function useWebSocket() {
  const addLog = useAppStore((s) => s.addLog);
  const updateProviderStatus = useAppStore((s) => s.updateProviderStatus);
  const setSystemStats = useAppStore((s) => s.setSystemStats);
  const wsRef = useRef<WebSocket | null>(null);
  const reconnectTimeout = useRef<number>(1000);
  const mountedRef = useRef(false);

  useEffect(() => {
    // Skip double-mount in StrictMode
    if (mountedRef.current) return;
    mountedRef.current = true;

    function connect() {
      const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
      const ws = new WebSocket(`${protocol}//${window.location.host}/zz/ws`);
      wsRef.current = ws;

      ws.onopen = () => {
        reconnectTimeout.current = 1000;
      };

      ws.onclose = () => {
        if (mountedRef.current) {
          setTimeout(connect, reconnectTimeout.current);
          reconnectTimeout.current = Math.min(reconnectTimeout.current * 2, 30000);
        }
      };

      ws.onmessage = (event) => {
        const msg = JSON.parse(event.data);
        switch (msg.type) {
          case "log":
            addLog(msg.data as LogEntry);
            break;
          case "provider_state":
            updateProviderStatus(
              msg.data.name,
              msg.data.status,
              msg.data.cooldown_until
            );
            break;
          case "stats":
            setSystemStats(msg.data as SystemStats);
            break;
        }
      };
    }

    connect();
    return () => {
      mountedRef.current = false;
      wsRef.current?.close();
    };
  }, [addLog, updateProviderStatus, setSystemStats]);
}
