import { useState, useMemo, useRef, useEffect } from "react";
import {
  Search,
  Filter,
  ChevronDown,
  ChevronRight,
  ArrowRight,
  Download,
  Pause,
  Play,
} from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { useAppStore } from "@/stores/store";
import type { LogEntry } from "@/api/types";
import { cn } from "@/lib/utils";
import { toast } from "sonner";

/**
 * Expandable log detail row.
 */
function LogDetailPanel({ log }: { log: LogEntry }) {
  return (
    <div className="px-4 py-3 bg-muted/30 border-t text-xs space-y-2">
      <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
        <div>
          <span className="text-muted-foreground">Request ID</span>
          <div className="font-mono mt-0.5">{log.id}</div>
        </div>
        <div>
          <span className="text-muted-foreground">Duration</span>
          <div className="font-mono mt-0.5">
            {log.duration_ms}ms (TTFB: {log.ttfb_ms}ms)
          </div>
        </div>
        <div>
          <span className="text-muted-foreground">Model</span>
          <div className="font-mono mt-0.5">{log.model}</div>
        </div>
        <div>
          <span className="text-muted-foreground">Streaming</span>
          <div className="mt-0.5">{log.streaming ? "Yes" : "No"}</div>
        </div>
        <div>
          <span className="text-muted-foreground">Request Size</span>
          <div className="font-mono mt-0.5">
            {(log.request_bytes / 1024).toFixed(1)} KB
          </div>
        </div>
        <div>
          <span className="text-muted-foreground">Response Size</span>
          <div className="font-mono mt-0.5">
            {(log.response_bytes / 1024).toFixed(1)} KB
          </div>
        </div>
        {log.failover_chain && (
          <div className="col-span-2">
            <span className="text-muted-foreground">Failover Chain</span>
            <div className="flex items-center gap-1 mt-0.5 font-mono">
              {log.failover_chain.map((step, i) => (
                <span key={i} className="flex items-center gap-1">
                  {i > 0 && <ArrowRight className="h-3 w-3 text-amber-500" />}
                  <Badge
                    variant={step.includes("200") ? "success" : "warning"}
                    className="text-[10px]"
                  >
                    {step}
                  </Badge>
                </span>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

/**
 * Logs page with filterable real-time log table.
 */
export default function Logs() {
  const logs = useAppStore((s) => s.logs);
  const providers = useAppStore((s) => s.providers);

  const [statusFilter, setStatusFilter] = useState("all");
  const [providerFilter, setProviderFilter] = useState("all");
  const [searchQuery, setSearchQuery] = useState("");
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [autoScroll, setAutoScroll] = useState(true);
  const scrollContainerRef = useRef<HTMLDivElement>(null);

  const filteredLogs = useMemo(() => {
    return logs.filter((log) => {
      if (statusFilter === "2xx" && (log.status < 200 || log.status >= 300)) return false;
      if (statusFilter === "4xx" && (log.status < 400 || log.status >= 500)) return false;
      if (statusFilter === "5xx" && log.status < 500) return false;
      if (statusFilter === "error" && log.status < 400) return false;
      if (providerFilter !== "all" && log.provider !== providerFilter) return false;
      if (searchQuery) {
        const q = searchQuery.toLowerCase();
        return (
          log.path.toLowerCase().includes(q) ||
          log.model.toLowerCase().includes(q) ||
          log.provider.toLowerCase().includes(q) ||
          log.id.toLowerCase().includes(q)
        );
      }
      return true;
    });
  }, [logs, statusFilter, providerFilter, searchQuery]);

  const statusOptions = [
    { value: "all", label: "All Status" },
    { value: "2xx", label: "2xx Success" },
    { value: "4xx", label: "4xx Client Error" },
    { value: "5xx", label: "5xx Server Error" },
    { value: "error", label: "All Errors" },
  ];

  const providerOptions = [
    { value: "all", label: "All Providers" },
    ...providers.map((p) => ({ value: p.name, label: p.name })),
  ];

  // Auto-scroll to top when new logs arrive and autoScroll is enabled
  useEffect(() => {
    if (autoScroll && scrollContainerRef.current) {
      scrollContainerRef.current.scrollTop = 0;
    }
  }, [logs, autoScroll]);

  /**
   * Export filtered logs as JSON file download.
   */
  function handleExport() {
    const data = JSON.stringify(filteredLogs, null, 2);
    const blob = new Blob([data], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `zz-logs-${new Date().toISOString().slice(0, 19)}.json`;
    a.click();
    URL.revokeObjectURL(url);
    toast.success(`Exported ${filteredLogs.length} log entries`);
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold tracking-tight">Logs</h1>
        <div className="flex items-center gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={() => setAutoScroll(!autoScroll)}
            className="gap-1.5"
          >
            {autoScroll ? (
              <>
                <Pause className="h-3.5 w-3.5" /> Pause
              </>
            ) : (
              <>
                <Play className="h-3.5 w-3.5" /> Resume
              </>
            )}
          </Button>
          <Button variant="outline" size="sm" className="gap-1.5" onClick={handleExport}>
            <Download className="h-3.5 w-3.5" /> Export
          </Button>
        </div>
      </div>

      {/* Filters */}
      <Card>
        <CardContent className="pt-4 pb-4">
          <div className="flex items-center gap-3">
            <Filter className="h-4 w-4 text-muted-foreground shrink-0" />
            <Select
              value={statusFilter}
              onChange={(e) => setStatusFilter(e.target.value)}
              options={statusOptions}
              className="w-40 h-9"
            />
            <Select
              value={providerFilter}
              onChange={(e) => setProviderFilter(e.target.value)}
              options={providerOptions}
              className="w-44 h-9"
            />
            <div className="relative flex-1">
              <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-muted-foreground" />
              <Input
                placeholder="Search by path, model, provider, id..."
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                className="pl-9 h-9"
              />
            </div>
            <span className="text-xs text-muted-foreground shrink-0">
              {filteredLogs.length} / {logs.length}
            </span>
          </div>
        </CardContent>
      </Card>

      {/* Log Table */}
      <Card>
        <CardHeader className="pb-0">
          <CardTitle>Request Log</CardTitle>
        </CardHeader>
        <CardContent className="pt-4">
          <div ref={scrollContainerRef} className="rounded-md border max-h-[600px] overflow-y-auto">
            {/* Header */}
            <div className="grid grid-cols-[24px_90px_56px_120px_1fr_70px_70px] gap-2 px-3 py-2 border-b bg-muted/50 text-xs font-medium text-muted-foreground sticky top-0 z-10">
              <span />
              <span>Time</span>
              <span>Status</span>
              <span>Provider</span>
              <span>Path</span>
              <span className="text-right">Duration</span>
              <span className="text-right">Model</span>
            </div>

            {/* Rows */}
            {filteredLogs.map((log) => {
              const isExpanded = expandedId === log.id;
              return (
                <div key={log.id}>
                  <div
                    className={cn(
                      "grid grid-cols-[24px_90px_56px_120px_1fr_70px_70px] gap-2 px-3 py-2 border-b text-sm items-center cursor-pointer hover:bg-accent/30 transition-colors",
                      isExpanded && "bg-accent/20",
                      log.failover_chain && "bg-amber-500/5"
                    )}
                    onClick={() => setExpandedId(isExpanded ? null : log.id)}
                  >
                    {isExpanded ? (
                      <ChevronDown className="h-3.5 w-3.5 text-muted-foreground" />
                    ) : (
                      <ChevronRight className="h-3.5 w-3.5 text-muted-foreground" />
                    )}
                    <span className="font-mono text-xs text-muted-foreground">
                      {new Date(log.timestamp).toLocaleTimeString()}
                    </span>
                    <Badge
                      variant={
                        log.status < 300
                          ? "success"
                          : log.status === 429
                            ? "warning"
                            : "danger"
                      }
                      className="text-[10px] justify-center"
                    >
                      {log.status}
                    </Badge>
                    <span className="font-mono text-xs truncate">
                      {log.provider}
                    </span>
                    <span className="font-mono text-xs truncate">
                      {log.method} {log.path}
                    </span>
                    <span className="font-mono text-xs text-right text-muted-foreground">
                      {log.duration_ms}ms
                    </span>
                    <span className="font-mono text-xs text-right truncate text-muted-foreground">
                      {log.model}
                    </span>
                  </div>
                  {isExpanded && <LogDetailPanel log={log} />}
                </div>
              );
            })}

            {filteredLogs.length === 0 && (
              <div className="px-4 py-8 text-center text-sm text-muted-foreground">
                No logs matching current filters.
              </div>
            )}
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
