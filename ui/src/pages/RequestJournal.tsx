import { useState, useEffect, useCallback } from "react";
import {
  Search,
  Filter,
  Download,
  Copy,
  ExternalLink,
  X,
  RefreshCw,
  XCircle,
  ChevronLeft,
  ChevronRight,
} from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { api } from "@/api/client";
import type {
  RequestJournalEntry,
  RequestJournalSummary,
  RequestJournalQuery,
  RequestTiming,
} from "@/api/types";
import { toast } from "sonner";
import { AlertCircle } from "lucide-react";

function TimingBreakdown({ timing }: { timing: RequestTiming }) {
  const hasRetries = timing.retry_count > 0;

  return (
    <div className="text-sm space-y-2">
      <div className="flex items-center gap-2">
        <span className="text-muted-foreground">Timing Breakdown</span>
        {!timing.completed && (
          <Badge variant="warning" className="text-[10px]">Incomplete</Badge>
        )}
      </div>
      <div className="bg-muted/50 rounded p-2 space-y-1.5">
        <div className="flex justify-between text-xs">
          <span className="text-muted-foreground">Model Parsing</span>
          <span className="font-mono">{timing.parse_model_ms} ms</span>
        </div>
        <div className="flex justify-between text-xs">
          <span className="text-muted-foreground">Provider Selection</span>
          <span className="font-mono">{timing.select_provider_ms} ms</span>
        </div>
        <div className="flex justify-between text-xs">
          <span className="text-muted-foreground">Upstream Total</span>
          <span className="font-mono font-medium">{timing.upstream_total_ms} ms</span>
        </div>
        <div className="flex justify-between text-xs">
          <span className="text-muted-foreground">Upstream TTFB</span>
          <span className="font-mono">{timing.upstream_ttfb_ms} ms</span>
        </div>
        <div className="flex justify-between text-xs">
          <span className="text-muted-foreground">Available Providers</span>
          <span className="font-mono">{timing.available_providers}</span>
        </div>
        <div className="flex justify-between text-xs">
          <span className="text-muted-foreground">Selection Reason</span>
          <Badge variant="outline" className="text-[10px] font-mono">
            {timing.selection_reason}
          </Badge>
        </div>
        {hasRetries && (
          <div className="pt-1 border-t">
            <div className="text-xs text-muted-foreground mb-1">
              Retry Breakdown ({timing.retry_count} retries)
            </div>
            {timing.retry_providers.map((prov, i) => {
              const duration = timing.retry_durations_ms[i];
              const error = timing.retry_errors?.[i];
              if (duration === undefined) return null;
              const isFailed = i < timing.retry_count;
              return (
                <div key={i} className="flex justify-between text-xs pl-2">
                  <span className="font-mono">
                    {i + 1}. {prov}
                    {isFailed && error && error !== "ok"
                      ? ` (${error})`
                      : isFailed
                        ? " (failed)"
                        : " (success)"}
                  </span>
                  <span className="font-mono">{duration} ms</span>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}

function RequestDetailModal({
  entry,
  onClose,
  onPrev,
  onNext,
  hasPrev,
  hasNext,
}: {
  entry: RequestJournalEntry;
  onClose: () => void;
  onPrev?: () => void;
  onNext?: () => void;
  hasPrev?: boolean;
  hasNext?: boolean;
}) {
  const [bodyPrettified, setBodyPrettified] = useState(true);
  const [respBodyPrettified, setRespBodyPrettified] = useState(true);
  const [respTab, setRespTab] = useState<"assembled" | "source">("assembled");

  const bodyText = entry.request_body_text || "";
  let displayBody = bodyText;
  if (bodyPrettified && bodyText) {
    try {
      const parsed = JSON.parse(bodyText);
      displayBody = JSON.stringify(parsed, null, 2);
    } catch {
      displayBody = bodyText;
    }
  }

  const isStreamingWithSse = entry.streaming && entry.sse_raw_body;
  const responseText = entry.response_body_text || entry.upstream_response_body || "";
  const assembledText = responseText;
  const sourceText = entry.sse_raw_body || "";

  const activeRespText = isStreamingWithSse
    ? (respTab === "source" ? sourceText : assembledText)
    : responseText;
  let displayRespBody = activeRespText;
  if (respBodyPrettified && activeRespText) {
    try {
      const parsed = JSON.parse(activeRespText);
      displayRespBody = JSON.stringify(parsed, null, 2);
    } catch {
      displayRespBody = activeRespText;
    }
  }

  const hasRespBody = !!(assembledText || sourceText || entry.response_body_base64);

  function copyBody() {
    navigator.clipboard.writeText(bodyText);
    toast.success("Request body copied to clipboard");
  }

  function copyRespBody() {
    navigator.clipboard.writeText(activeRespText);
    toast.success("Response body copied to clipboard");
  }

  // Keyboard support
  useEffect(() => {
    function handleKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
      if (e.key === "ArrowLeft" && hasPrev && onPrev) onPrev();
      if (e.key === "ArrowRight" && hasNext && onNext) onNext();
    }
    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  }, [onClose, onPrev, onNext, hasPrev, hasNext]);

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4">
      <Card className="w-full max-w-4xl max-h-[90vh] flex flex-col">
        <CardHeader className="flex-shrink-0 border-b">
          <div className="flex items-center justify-between">
            <CardTitle className="text-lg">Request Details</CardTitle>
            <div className="flex items-center gap-1">
              {hasPrev && (
                <Button variant="ghost" size="icon" onClick={onPrev} title="Previous (←)">
                  <ChevronLeft className="h-4 w-4" />
                </Button>
              )}
              {hasNext && (
                <Button variant="ghost" size="icon" onClick={onNext} title="Next (→)">
                  <ChevronRight className="h-4 w-4" />
                </Button>
              )}
              <Button variant="ghost" size="icon" onClick={onClose} title="Close (Esc)">
                <X className="h-4 w-4" />
              </Button>
            </div>
          </div>
        </CardHeader>
        <CardContent className="flex-1 overflow-y-auto pt-4 space-y-4">
          <div className="grid grid-cols-2 md:grid-cols-4 gap-4 text-sm">
            <div>
              <span className="text-muted-foreground">ID</span>
              <div className="font-mono mt-0.5">{entry.id}</div>
            </div>
            <div>
              <span className="text-muted-foreground">Timestamp</span>
              <div className="mt-0.5">{formatTimestamp(entry.timestamp)}</div>
            </div>
            <div>
              <span className="text-muted-foreground">Client</span>
              <div className="mt-0.5">{entry.client_name}</div>
            </div>
            <div>
              <span className="text-muted-foreground">Status</span>
              <Badge
                variant={entry.status < 300 ? "success" : entry.status === 503 ? "warning" : "danger"}
                className="mt-0.5"
              >
                {entry.status}
              </Badge>
            </div>
            <div>
              <span className="text-muted-foreground">Provider</span>
              <div className="font-mono mt-0.5">{entry.provider || "-"}</div>
            </div>
            <div>
              <span className="text-muted-foreground">Model</span>
              <div className="font-mono mt-0.5">{entry.model || "-"}</div>
            </div>
            <div>
              <span className="text-muted-foreground">Streaming</span>
              <div className="mt-0.5">{entry.streaming ? "Yes" : "No"}</div>
            </div>
            <div>
              <span className="text-muted-foreground">Request Size</span>
              <div className="font-mono mt-0.5">
                {(entry.request_bytes / 1024).toFixed(1)} KB
              </div>
            </div>
          </div>

          {entry.timing && (
            <TimingBreakdown timing={entry.timing} />
          )}

          {!entry.timing && (
            <div className="text-sm">
              <span className="text-muted-foreground">Timing Breakdown</span>
              <div className="mt-0.5 text-xs text-muted-foreground italic">
                Not collected
              </div>
            </div>
          )}

          {entry.upstream_url && (
            <div className="text-sm">
              <span className="text-muted-foreground">Upstream URL</span>
              <div className="font-mono mt-0.5 text-xs break-all bg-muted/50 p-2 rounded">
                {entry.upstream_url}
              </div>
            </div>
          )}

          {entry.failover_chain && entry.failover_chain.length > 0 && (
            <div className="text-sm">
              <span className="text-muted-foreground">Failover Chain</span>
              <div className="flex items-center gap-1 mt-0.5 font-mono flex-wrap">
                {entry.failover_chain.map((step, i) => (
                  <span key={i} className="flex items-center gap-1">
                    {i > 0 && <ExternalLink className="h-3 w-3 text-amber-500" />}
                    <Badge
                      variant={step.includes(":200") ? "success" : step.includes(":err") ? "danger" : "warning"}
                      className="text-[10px]"
                    >
                      {step}
                    </Badge>
                  </span>
                ))}
              </div>
            </div>
          )}

          {entry.error && (
            <div className="text-sm">
              <span className="text-muted-foreground text-destructive">Error</span>
              <div className="mt-0.5 bg-destructive/10 text-destructive p-2 rounded text-xs font-mono">
                {entry.error}
              </div>
            </div>
          )}

          {entry.upstream_error_body && (
            <div className="text-sm">
              <span className="text-muted-foreground">Upstream Error Body</span>
              <div className="mt-0.5 bg-muted/50 rounded p-2 overflow-x-auto max-h-60">
                <pre className="font-mono text-xs whitespace-pre-wrap break-all">
                  {entry.upstream_error_body}
                </pre>
              </div>
            </div>
          )}

          <div className="text-sm">
            <span className="text-muted-foreground">Request Headers</span>
            <div className="mt-0.5 bg-muted/50 rounded p-2 overflow-x-auto">
              <pre className="font-mono text-xs">
                {Object.entries(entry.request_headers)
                  .map(([k, v]) => `${k}: ${v}`)
                  .join("\n")}
              </pre>
            </div>
          </div>

          {(entry.request_body_text || entry.request_body_base64) && (
            <div className="text-sm">
              <div className="flex items-center justify-between mb-0.5">
                <span className="text-muted-foreground">Request Body</span>
                <div className="flex items-center gap-2">
                  {entry.request_body_text && (
                    <Button
                      variant="ghost"
                      size="sm"
                      className="h-6 text-xs"
                      onClick={() => setBodyPrettified(!bodyPrettified)}
                    >
                      {bodyPrettified ? "Raw" : "Prettify"}
                    </Button>
                  )}
                  <Button variant="ghost" size="sm" className="h-6" onClick={copyBody}>
                    <Copy className="h-3 w-3" />
                  </Button>
                </div>
              </div>
              <div className="mt-0.5 bg-muted/50 rounded p-2 overflow-x-auto max-h-80">
                <pre className="font-mono text-xs whitespace-pre-wrap break-all">
                  {entry.request_body_base64
                    ? `[Binary data, base64: ${entry.request_body_base64.slice(0, 50)}...]`
                    : displayBody}
                </pre>
              </div>
            </div>
          )}

          {hasRespBody && (
            <div className="text-sm">
              <div className="flex items-center justify-between mb-1">
                <span className="text-muted-foreground">Response Body</span>
                <div className="flex items-center gap-2">
                  <Button
                    variant="ghost"
                    size="sm"
                    className="h-6 text-xs"
                    onClick={() => setRespBodyPrettified(!respBodyPrettified)}
                  >
                    {respBodyPrettified ? "Raw" : "Prettify"}
                  </Button>
                  <Button variant="ghost" size="sm" className="h-6" onClick={copyRespBody}>
                    <Copy className="h-3 w-3" />
                  </Button>
                </div>
              </div>
              {isStreamingWithSse && (
              <div className="flex border-b mb-0">
                <button
                  className={`px-3 py-1.5 text-xs font-medium border-b-2 transition-colors ${
                    respTab === "assembled"
                      ? "border-primary text-primary"
                      : "border-transparent text-muted-foreground hover:text-foreground"
                  }`}
                  onClick={() => setRespTab("assembled")}
                >
                  Assembled
                </button>
                <button
                  className={`px-3 py-1.5 text-xs font-medium border-b-2 transition-colors ${
                    respTab === "source"
                      ? "border-primary text-primary"
                      : "border-transparent text-muted-foreground hover:text-foreground"
                  }`}
                  onClick={() => setRespTab("source")}
                >
                  Raw SSE
                </button>
              </div>
              )}
              <div className="bg-muted/50 rounded-b p-2 overflow-x-auto max-h-80">
                <pre className="font-mono text-xs whitespace-pre-wrap break-all">
                  {entry.response_body_base64 && respTab === "assembled"
                    ? `[Binary data, base64: ${entry.response_body_base64.slice(0, 50)}...]`
                    : displayRespBody}
                </pre>
              </div>
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}

/** Format a timestamp string for display. Shows date only if not today. */
function formatTimestamp(ts: string): string {
  const d = new Date(ts);
  const now = new Date();
  const isToday = d.toDateString() === now.toDateString();
  if (isToday) {
    return d.toLocaleTimeString();
  }
  const month = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  const hours = String(d.getHours()).padStart(2, "0");
  const mins = String(d.getMinutes()).padStart(2, "0");
  return `${month}/${day} ${hours}:${mins}`;
}

/** Format timestamp with relative time for recent entries */
function formatTimestampRelative(ts: string): string {
  const d = new Date(ts);
  const now = new Date();
  const diffMs = now.getTime() - d.getTime();
  const diffMin = Math.floor(diffMs / 60000);
  const diffHour = Math.floor(diffMs / 3600000);

  if (diffMin < 1) return "just now";
  if (diffMin < 60) return `${diffMin}m ago`;
  if (diffHour < 24 && d.toDateString() === now.toDateString()) {
    return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  }
  return formatTimestamp(ts);
}

type SearchScope = "path" | "model" | "all";

export default function RequestJournal() {
  const [entries, setEntries] = useState<RequestJournalSummary[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);
  const [selectedEntry, setSelectedEntry] = useState<RequestJournalEntry | null>(null);
  const [selectedIndex, setSelectedIndex] = useState(-1);
  const [journalEnabled, setJournalEnabled] = useState<boolean | null>(null);

  const [offset, setOffset] = useState(0);
  const [pageSize, setPageSize] = useState(50);

  // Filters
  const [clientFilter, setClientFilter] = useState("all");
  const [providerFilter, setProviderFilter] = useState("all");
  const [modelFilter, setModelFilter] = useState("all");
  const [statusFilter, setStatusFilter] = useState("all");
  const [searchQuery, setSearchQuery] = useState("");
  const [searchScope, setSearchScope] = useState<SearchScope>("all");
  const [failedOnly, setFailedOnly] = useState(false);
  const [slowOnly, setSlowOnly] = useState(false);
  const [dateFilter, setDateFilter] = useState("");

  // Facets from backend
  const [facetClients, setFacetClients] = useState<string[]>([]);
  const [facetProviders, setFacetProviders] = useState<string[]>([]);
  const [facetModels, setFacetModels] = useState<string[]>([]);

  // Auto-refresh
  const [autoRefresh, setAutoRefresh] = useState(true);
  const [lastRefreshed, setLastRefreshed] = useState<Date | null>(null);

  const hasActiveFilters = clientFilter !== "all"
    || providerFilter !== "all"
    || modelFilter !== "all"
    || statusFilter !== "all"
    || searchQuery !== ""
    || failedOnly
    || slowOnly
    || dateFilter !== "";

  const fetchEntries = useCallback(async () => {
    setLoading(true);
    try {
      const query: RequestJournalQuery & { offset: number; limit: number } = {
        offset,
        limit: pageSize,
      };
      if (clientFilter !== "all") query.client = clientFilter;
      if (providerFilter !== "all") query.provider = providerFilter;
      if (modelFilter !== "all") query.model = modelFilter;
      if (statusFilter !== "all") query.status = parseInt(statusFilter);
      if (searchQuery) {
        if (searchScope === "path") {
          query.path = searchQuery;
        } else if (searchScope === "model") {
          query.model = searchQuery;
        } else {
          query.path = searchQuery;
        }
      }
      if (dateFilter) query.date = dateFilter;
      if (failedOnly) query.failed = true;
      if (slowOnly) query.slow = true;

      const response = await api.getRequestJournal(query);
      setEntries(response.entries);
      setTotal(response.total);
      if (response.enabled !== undefined) {
        setJournalEnabled(response.enabled);
      }
      setLastRefreshed(new Date());
    } catch {
      toast.error("Failed to fetch request journal");
    } finally {
      setLoading(false);
    }
  }, [offset, pageSize, clientFilter, providerFilter, modelFilter, statusFilter, searchQuery, searchScope, dateFilter, failedOnly, slowOnly]);

  const fetchFacets = useCallback(async () => {
    try {
      const facets = await api.getRequestJournalFacets();
      setFacetClients(facets.clients);
      setFacetProviders(facets.providers);
      setFacetModels(facets.models);
    } catch {
      // Non-critical: facets are for filter convenience
    }
  }, []);

  // Check status on mount
  useEffect(() => {
    async function checkStatus() {
      try {
        const status = await api.getRequestJournalStatus();
        setJournalEnabled(status.enabled);
      } catch {
        setJournalEnabled(false);
      }
    }
    checkStatus();
    fetchFacets();
  }, [fetchFacets]);

  // Fetch entries when filters change
  useEffect(() => {
    fetchEntries();
  }, [fetchEntries]);

  // Debounced search
  useEffect(() => {
    const timer = setTimeout(() => {
      if (offset === 0) fetchEntries();
      else setOffset(0);
    }, 300);
    return () => clearTimeout(timer);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [searchQuery]);

  // Auto-refresh every 10 seconds
  useEffect(() => {
    if (!autoRefresh) return;
    const interval = setInterval(() => {
      fetchEntries();
      fetchFacets();
    }, 10000);
    return () => clearInterval(interval);
  }, [autoRefresh, fetchEntries, fetchFacets]);

  // Reset offset when filters change
  useEffect(() => {
    setOffset(0);
  }, [clientFilter, providerFilter, modelFilter, statusFilter, dateFilter, failedOnly, slowOnly]);

  async function handleRowClick(entry: RequestJournalSummary, index: number) {
    try {
      const fullEntry = await api.getRequestJournalEntry(entry.id);
      setSelectedEntry(fullEntry);
      setSelectedIndex(index);
    } catch {
      toast.error("Failed to fetch entry details");
    }
  }

  async function navigateEntry(direction: "prev" | "next") {
    const newIndex = direction === "prev" ? selectedIndex - 1 : selectedIndex + 1;
    if (newIndex < 0 || newIndex >= entries.length) return;
    const target = entries[newIndex];
    try {
      const fullEntry = await api.getRequestJournalEntry(target.id);
      setSelectedEntry(fullEntry);
      setSelectedIndex(newIndex);
    } catch {
      toast.error("Failed to fetch entry details");
    }
  }

  function handleExport() {
    const query: RequestJournalQuery = {};
    if (clientFilter !== "all") query.client = clientFilter;
    if (providerFilter !== "all") query.provider = providerFilter;
    if (modelFilter !== "all") query.model = modelFilter;
    if (statusFilter !== "all") query.status = parseInt(statusFilter);
    if (searchQuery) query.path = searchQuery;
    if (dateFilter) query.date = dateFilter;
    if (failedOnly) query.failed = true;
    if (slowOnly) query.slow = true;
    api.exportRequestJournal(query);
    toast.info("Export started — downloading...");
  }

  function clearFilters() {
    setClientFilter("all");
    setProviderFilter("all");
    setModelFilter("all");
    setStatusFilter("all");
    setSearchQuery("");
    setSearchScope("all");
    setFailedOnly(false);
    setSlowOnly(false);
    setDateFilter("");
    setOffset(0);
  }

  const totalPages = Math.ceil(total / pageSize);

  return (
    <div className="flex flex-col flex-1 overflow-hidden gap-4">
      {/* Header */}
      <div className="flex items-center justify-between shrink-0">
        <h1 className="text-2xl font-bold tracking-tight">Request Journal</h1>
        <div className="flex items-center gap-2">
          {lastRefreshed && (
            <span className="text-xs text-muted-foreground">
              Updated {formatTimestampRelative(lastRefreshed.toISOString())}
            </span>
          )}
          <Button
            variant="outline"
            size="sm"
            className="gap-1.5"
            onClick={() => { fetchEntries(); fetchFacets(); }}
          >
            <RefreshCw className={`h-3.5 w-3.5 ${loading ? "animate-spin" : ""}`} />
          </Button>
          <Button
            variant={autoRefresh ? "secondary" : "outline"}
            size="sm"
            className="text-xs"
            onClick={() => setAutoRefresh(!autoRefresh)}
          >
            {autoRefresh ? "Auto: ON" : "Auto: OFF"}
          </Button>
          <Button variant="outline" size="sm" className="gap-1.5" onClick={handleExport}>
            <Download className="h-3.5 w-3.5" /> Export
          </Button>
        </div>
      </div>

      {/* Disabled warning */}
      {journalEnabled === false && (
        <Card className="shrink-0 bg-yellow-50 border-yellow-200">
          <CardContent className="pt-4 pb-4">
            <div className="flex items-start gap-3">
              <AlertCircle className="h-5 w-5 text-yellow-600 shrink-0 mt-0.5" />
              <div className="space-y-2">
                <p className="text-sm font-medium text-yellow-800">
                  Request Journal is disabled
                </p>
                <p className="text-xs text-yellow-700">
                  Enable it in config.toml to capture LLM requests:
                </p>
                <pre className="text-xs bg-yellow-100 p-2 rounded text-yellow-900">
{`[observability.request_journal]
enabled = true
storage_dir = "logs/request-journal"`}
                </pre>
              </div>
            </div>
          </CardContent>
        </Card>
      )}

      {/* Filters */}
      <Card className="shrink-0">
        <CardContent className="pt-4 pb-4">
          <div className="flex items-center gap-3 flex-wrap">
            <Filter className="h-4 w-4 text-muted-foreground shrink-0" />
            <Select value={clientFilter} onValueChange={setClientFilter}>
              <SelectTrigger className="w-32 h-9">
                <SelectValue placeholder="Client" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">All Clients</SelectItem>
                {facetClients.map((c) => (
                  <SelectItem key={c} value={c}>{c}</SelectItem>
                ))}
              </SelectContent>
            </Select>
            <Select value={providerFilter} onValueChange={setProviderFilter}>
              <SelectTrigger className="w-36 h-9">
                <SelectValue placeholder="Provider" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">All Providers</SelectItem>
                {facetProviders.map((p) => (
                  <SelectItem key={p} value={p}>{p}</SelectItem>
                ))}
              </SelectContent>
            </Select>
            <Select value={modelFilter} onValueChange={setModelFilter}>
              <SelectTrigger className="w-40 h-9">
                <SelectValue placeholder="Model" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">All Models</SelectItem>
                {facetModels.map((m) => (
                  <SelectItem key={m} value={m}>{m}</SelectItem>
                ))}
              </SelectContent>
            </Select>
            <Select value={statusFilter} onValueChange={setStatusFilter}>
              <SelectTrigger className="w-28 h-9">
                <SelectValue placeholder="Status" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">All Status</SelectItem>
                <SelectItem value="200">200 OK</SelectItem>
                <SelectItem value="400">400 Bad Request</SelectItem>
                <SelectItem value="401">401 Unauthorized</SelectItem>
                <SelectItem value="429">429 Rate Limited</SelectItem>
                <SelectItem value="500">500 Server Error</SelectItem>
                <SelectItem value="503">503 Failed</SelectItem>
              </SelectContent>
            </Select>
            <Button
              variant={failedOnly ? "destructive" : "outline"}
              size="sm"
              className="h-9 gap-1.5"
              onClick={() => setFailedOnly(!failedOnly)}
            >
              Failed
            </Button>
            <Button
              variant={slowOnly ? "secondary" : "outline"}
              size="sm"
              className="h-9 gap-1.5"
              onClick={() => setSlowOnly(!slowOnly)}
            >
              Slow
            </Button>
            <Input
              type="date"
              value={dateFilter}
              onChange={(e) => setDateFilter(e.target.value)}
              className="w-36 h-9"
            />
            {/* Search with scope selector */}
            <div className="relative flex-1 min-w-48 flex items-center gap-1">
              <Select value={searchScope} onValueChange={(v) => setSearchScope(v as SearchScope)}>
                <SelectTrigger className="w-20 h-9 text-xs shrink-0">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="all">All</SelectItem>
                  <SelectItem value="path">Path</SelectItem>
                  <SelectItem value="model">Model</SelectItem>
                </SelectContent>
              </Select>
              <div className="relative flex-1">
                <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-muted-foreground" />
                <Input
                  placeholder="Search..."
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  className="pl-9 h-9"
                />
              </div>
            </div>
            {hasActiveFilters && (
              <Button
                variant="ghost"
                size="sm"
                className="h-9 gap-1 text-muted-foreground hover:text-foreground"
                onClick={clearFilters}
              >
                <XCircle className="h-3.5 w-3.5" /> Clear
              </Button>
            )}
            <span className="text-xs text-muted-foreground shrink-0">
              {total} entries
            </span>
          </div>
        </CardContent>
      </Card>

      {/* Table */}
      <Card className="flex-1 flex flex-col min-h-0">
        <CardContent className="pt-4 flex-1 flex flex-col min-h-0">
          {loading && entries.length === 0 ? (
            <div className="flex-1 flex items-center justify-center">
              <div className="h-6 w-6 animate-spin rounded-full border-2 border-muted border-t-primary" />
            </div>
          ) : (
            <div className="rounded-md border flex-1 overflow-y-auto min-h-0">
              <table className="w-full text-sm">
                <thead className="bg-muted/50 sticky top-0 z-10">
                  <tr>
                    <th className="text-left px-3 py-2 text-xs font-medium text-muted-foreground w-[90px]">Time</th>
                    <th className="text-left px-3 py-2 text-xs font-medium text-muted-foreground w-[80px]">Client</th>
                    <th className="text-left px-3 py-2 text-xs font-medium text-muted-foreground w-[100px]">Provider</th>
                    <th className="text-left px-3 py-2 text-xs font-medium text-muted-foreground w-[130px]">Model</th>
                    <th className="text-left px-3 py-2 text-xs font-medium text-muted-foreground">Path</th>
                    <th className="text-left px-3 py-2 text-xs font-medium text-muted-foreground w-[50px] text-center">Stream</th>
                    <th className="text-right px-3 py-2 text-xs font-medium text-muted-foreground w-[60px]">Status</th>
                    <th className="text-right px-3 py-2 text-xs font-medium text-muted-foreground w-[70px]">Latency</th>
                    <th className="text-right px-3 py-2 text-xs font-medium text-muted-foreground w-[70px]">Size</th>
                  </tr>
                </thead>
                <tbody>
                  {entries.map((entry, idx) => (
                    <tr
                      key={entry.id}
                      className={`border-b cursor-pointer hover:bg-accent/30 transition-colors ${
                        selectedIndex === idx ? "bg-accent/20" : ""
                      }`}
                      onClick={() => handleRowClick(entry, idx)}
                    >
                      <td className="px-3 py-2">
                        <span className="font-mono text-xs text-muted-foreground">
                          {formatTimestamp(entry.timestamp)}
                        </span>
                      </td>
                      <td className="px-3 py-2">
                        <span className="text-xs truncate block">{entry.client_name}</span>
                      </td>
                      <td className="px-3 py-2">
                        <span className="font-mono text-xs truncate block">{entry.provider || "-"}</span>
                      </td>
                      <td className="px-3 py-2">
                        <span className="font-mono text-xs truncate block text-muted-foreground">
                          {entry.model || "-"}
                        </span>
                      </td>
                      <td className="px-3 py-2">
                        <span className="font-mono text-xs truncate block">
                          {entry.method} {entry.path}
                        </span>
                      </td>
                      <td className="px-3 py-2 text-center">
                        {entry.streaming ? (
                          <Badge variant="outline" className="text-[9px] px-1 py-0">SSE</Badge>
                        ) : (
                          <span className="text-[10px] text-muted-foreground">-</span>
                        )}
                      </td>
                      <td className="px-3 py-2 text-right">
                        <Badge
                          variant={
                            entry.status < 300
                              ? "success"
                              : entry.status === 503
                                ? "warning"
                                : "danger"
                          }
                          className="text-[10px] justify-center"
                        >
                          {entry.status}
                        </Badge>
                      </td>
                      <td className="px-3 py-2 text-right">
                        {entry.timing_summary ? (
                          <Badge
                            variant={
                              entry.timing_summary.upstream_total_ms > 3000
                                ? "danger"
                                : entry.timing_summary.upstream_total_ms > 1000
                                  ? "warning"
                                  : "success"
                            }
                            className="text-[10px] justify-center"
                          >
                            {entry.timing_summary.upstream_total_ms > 1000
                              ? `${(entry.timing_summary.upstream_total_ms / 1000).toFixed(1)}s`
                              : `${entry.timing_summary.upstream_total_ms}ms`}
                            {!entry.timing_summary.completed && " \u26A0"}
                          </Badge>
                        ) : (
                          <span className="text-xs text-muted-foreground">-</span>
                        )}
                      </td>
                      <td className="px-3 py-2 text-right">
                        <span className="font-mono text-xs text-muted-foreground">
                          {formatBytes(entry.request_bytes, entry.response_bytes)}
                        </span>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>

              {entries.length === 0 && (
                <div className="px-4 py-8 text-center text-sm text-muted-foreground">
                  {journalEnabled === false ? (
                    <div className="space-y-3">
                      <div className="flex items-center justify-center gap-2 text-yellow-600">
                        <AlertCircle className="h-4 w-4" />
                        <span className="font-medium">Request Journal is disabled</span>
                      </div>
                      <p className="text-xs text-muted-foreground">
                        Enable it in config.toml:
                      </p>
                      <pre className="text-xs bg-muted/50 p-2 rounded inline-block text-left">
{`[observability.request_journal]
enabled = true`}
                      </pre>
                    </div>
                  ) : (
                    <div className="space-y-2">
                      <p>No journal entries found.</p>
                      <p className="text-xs">
                        Send some LLM requests to see them captured here.
                      </p>
                    </div>
                  )}
                </div>
              )}
            </div>
          )}

          {/* Pagination */}
          {totalPages > 1 && (
            <div className="flex items-center justify-between pt-4 shrink-0">
              <div className="flex items-center gap-2">
                <span className="text-xs text-muted-foreground">
                  Page {offset / pageSize + 1} of {totalPages}
                </span>
                <Select
                  value={String(pageSize)}
                  onValueChange={(v) => {
                    setPageSize(Number(v));
                    setOffset(0);
                  }}
                >
                  <SelectTrigger className="w-20 h-7 text-xs">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="25">25/page</SelectItem>
                    <SelectItem value="50">50/page</SelectItem>
                    <SelectItem value="100">100/page</SelectItem>
                  </SelectContent>
                </Select>
              </div>
              <div className="flex items-center gap-1">
                <Button
                  variant="outline"
                  size="sm"
                  disabled={offset === 0}
                  onClick={() => setOffset(0)}
                  className="h-8 px-2"
                >
                  First
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  disabled={offset === 0}
                  onClick={() => setOffset(Math.max(0, offset - pageSize))}
                >
                  <ChevronLeft className="h-4 w-4" />
                </Button>
                {generatePageNumbers(offset / pageSize + 1, totalPages).map((page) => (
                  <Button
                    key={page}
                    variant={page === offset / pageSize + 1 ? "secondary" : "outline"}
                    size="sm"
                    className="h-8 w-8 px-0"
                    onClick={() => setOffset((page - 1) * pageSize)}
                  >
                    {page}
                  </Button>
                ))}
                <Button
                  variant="outline"
                  size="sm"
                  disabled={offset + pageSize >= total}
                  onClick={() => setOffset(offset + pageSize)}
                >
                  <ChevronRight className="h-4 w-4" />
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  disabled={offset + pageSize >= total}
                  onClick={() => setOffset((totalPages - 1) * pageSize)}
                  className="h-8 px-2"
                >
                  Last
                </Button>
              </div>
            </div>
          )}
        </CardContent>
      </Card>

      {/* Detail Modal */}
      {selectedEntry && (
        <RequestDetailModal
          entry={selectedEntry}
          onClose={() => { setSelectedEntry(null); setSelectedIndex(-1); }}
          onPrev={selectedIndex > 0 ? () => navigateEntry("prev") : undefined}
          onNext={selectedIndex < entries.length - 1 ? () => navigateEntry("next") : undefined}
          hasPrev={selectedIndex > 0}
          hasNext={selectedIndex < entries.length - 1}
        />
      )}
    </div>
  );
}

/** Format request/response bytes as compact string */
function formatBytes(reqBytes: number, respBytes: number): string {
  const req = reqBytes > 0 ? `${(reqBytes / 1024).toFixed(0)}K` : "0";
  const resp = respBytes > 0 ? `${(respBytes / 1024).toFixed(0)}K` : "0";
  return `${req}/${resp}`;
}

/** Generate page numbers to display in pagination (current ± 2) */
function generatePageNumbers(current: number, total: number): number[] {
  const pages: number[] = [];
  const start = Math.max(1, Math.min(current - 2, total - 4));
  const end = Math.min(total, start + 4);
  for (let i = start; i <= end; i++) {
    pages.push(i);
  }
  return pages;
}
