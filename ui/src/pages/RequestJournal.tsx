import { useState, useEffect, useMemo } from "react";
import {
  Search,
  Filter,
  ChevronDown,
  ChevronRight,
  Download,
  Copy,
  ExternalLink,
  X,
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
} from "@/api/types";
import { cn } from "@/lib/utils";
import { toast } from "sonner";
import { AlertCircle } from "lucide-react";

function RequestDetailModal({
  entry,
  onClose,
}: {
  entry: RequestJournalEntry;
  onClose: () => void;
}) {
  const [bodyPrettified, setBodyPrettified] = useState(true);

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

  function copyBody() {
    navigator.clipboard.writeText(bodyText);
    toast.success("Request body copied to clipboard");
  }

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4">
      <Card className="w-full max-w-4xl max-h-[90vh] flex flex-col">
        <CardHeader className="flex-shrink-0 border-b">
          <div className="flex items-center justify-between">
            <CardTitle className="text-lg">Request Details</CardTitle>
            <Button variant="ghost" size="icon" onClick={onClose}>
              <X className="h-4 w-4" />
            </Button>
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
              <div className="mt-0.5">{entry.timestamp}</div>
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
        </CardContent>
      </Card>
    </div>
  );
}

export default function RequestJournal() {
  const [entries, setEntries] = useState<RequestJournalSummary[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);
  const [selectedEntry, setSelectedEntry] = useState<RequestJournalEntry | null>(null);
  const [journalEnabled, setJournalEnabled] = useState<boolean | null>(null);

  const [offset, setOffset] = useState(0);
  const limit = 50;

  const [clientFilter, setClientFilter] = useState("all");
  const [providerFilter, setProviderFilter] = useState("all");
  const [statusFilter, setStatusFilter] = useState("all");
  const [searchQuery, setSearchQuery] = useState("");
  const [dateFilter, setDateFilter] = useState("");

  const clients = useMemo(() => {
    const unique = new Set(entries.map((e) => e.client_name));
    return ["all", ...unique];
  }, [entries]);

  const providers = useMemo(() => {
    const unique = new Set(entries.map((e) => e.provider).filter(Boolean));
    return ["all", ...unique];
  }, [entries]);

  async function fetchEntries() {
    setLoading(true);
    try {
      const query: RequestJournalQuery & { offset: number; limit: number } = {
        offset,
        limit,
      };
      if (clientFilter !== "all") query.client = clientFilter;
      if (providerFilter !== "all") query.provider = providerFilter;
      if (statusFilter !== "all") query.status = parseInt(statusFilter);
      if (searchQuery) query.path = searchQuery;
      if (dateFilter) query.date = dateFilter;

      const response = await api.getRequestJournal(query);
      setEntries(response.entries);
      setTotal(response.total);
      if (response.enabled !== undefined) {
        setJournalEnabled(response.enabled);
      }
    } catch (error) {
      toast.error("Failed to fetch request journal");
    } finally {
      setLoading(false);
    }
  }

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
  }, []);

  useEffect(() => {
    fetchEntries();
  }, [offset, clientFilter, providerFilter, statusFilter, dateFilter]);

  useEffect(() => {
    const timer = setTimeout(() => {
      if (offset === 0) fetchEntries();
      else setOffset(0);
    }, 300);
    return () => clearTimeout(timer);
  }, [searchQuery]);

  async function handleRowClick(entry: RequestJournalSummary) {
    try {
      const fullEntry = await api.getRequestJournalEntry(entry.id);
      setSelectedEntry(fullEntry);
    } catch {
      toast.error("Failed to fetch entry details");
    }
  }

  function handleExport() {
    const query: RequestJournalQuery = {};
    if (clientFilter !== "all") query.client = clientFilter;
    if (providerFilter !== "all") query.provider = providerFilter;
    if (statusFilter !== "all") query.status = parseInt(statusFilter);
    if (searchQuery) query.path = searchQuery;
    if (dateFilter) query.date = dateFilter;
    api.exportRequestJournal(query);
  }

  const totalPages = Math.ceil(total / limit);

  return (
    <div className="flex flex-col flex-1 overflow-hidden gap-6">
      <div className="flex items-center justify-between shrink-0">
        <h1 className="text-2xl font-bold tracking-tight">Request Journal</h1>
        <Button variant="outline" size="sm" className="gap-1.5" onClick={handleExport}>
          <Download className="h-3.5 w-3.5" /> Export
        </Button>
      </div>

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

      <Card className="shrink-0">
        <CardContent className="pt-4 pb-4">
          <div className="flex items-center gap-3 flex-wrap">
            <Filter className="h-4 w-4 text-muted-foreground shrink-0" />
            <Select value={clientFilter} onValueChange={setClientFilter}>
              <SelectTrigger className="w-32 h-9">
                <SelectValue placeholder="Client" />
              </SelectTrigger>
              <SelectContent>
                {clients.map((c) => (
                  <SelectItem key={c} value={c}>
                    {c === "all" ? "All Clients" : c}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <Select value={providerFilter} onValueChange={setProviderFilter}>
              <SelectTrigger className="w-36 h-9">
                <SelectValue placeholder="Provider" />
              </SelectTrigger>
              <SelectContent>
                {providers.map((p) => (
                  <SelectItem key={p} value={p}>
                    {p === "all" ? "All Providers" : p}
                  </SelectItem>
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
            <Input
              type="date"
              value={dateFilter}
              onChange={(e) => setDateFilter(e.target.value)}
              className="w-36 h-9"
            />
            <div className="relative flex-1 min-w-48">
              <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-muted-foreground" />
              <Input
                placeholder="Search by path..."
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                className="pl-9 h-9"
              />
            </div>
            <span className="text-xs text-muted-foreground shrink-0">
              {total} entries
            </span>
          </div>
        </CardContent>
      </Card>

      <Card className="flex-1 flex flex-col min-h-0">
        <CardContent className="pt-4 flex-1 flex flex-col min-h-0">
          {loading ? (
            <div className="flex-1 flex items-center justify-center">
              <div className="h-6 w-6 animate-spin rounded-full border-2 border-muted border-t-primary" />
            </div>
          ) : (
            <div className="rounded-md border flex-1 overflow-y-auto min-h-0">
              <div className="grid grid-cols-[90px_80px_100px_100px_1fr_60px_70px] gap-2 px-3 py-2 border-b bg-muted/50 text-xs font-medium text-muted-foreground sticky top-0 z-10">
                <span>Time</span>
                <span>Client</span>
                <span>Provider</span>
                <span>Model</span>
                <span>Path</span>
                <span className="text-right">Status</span>
                <span className="text-right">Size</span>
              </div>

              {entries.map((entry) => (
                <div
                  key={entry.id}
                  className="grid grid-cols-[90px_80px_100px_100px_1fr_60px_70px] gap-2 px-3 py-2 border-b text-sm items-center cursor-pointer hover:bg-accent/30 transition-colors"
                  onClick={() => handleRowClick(entry)}
                >
                  <span className="font-mono text-xs text-muted-foreground">
                    {new Date(entry.timestamp).toLocaleTimeString()}
                  </span>
                  <span className="text-xs truncate">{entry.client_name}</span>
                  <span className="font-mono text-xs truncate">{entry.provider || "-"}</span>
                  <span className="font-mono text-xs truncate text-muted-foreground">
                    {entry.model || "-"}
                  </span>
                  <span className="font-mono text-xs truncate">
                    {entry.method} {entry.path}
                  </span>
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
                  <span className="font-mono text-xs text-right text-muted-foreground">
                    {(entry.request_bytes / 1024).toFixed(0)}K
                  </span>
                </div>
              ))}

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

          {totalPages > 1 && (
            <div className="flex items-center justify-between pt-4 shrink-0">
              <span className="text-xs text-muted-foreground">
                Page {offset / limit + 1} of {totalPages}
              </span>
              <div className="flex gap-2">
                <Button
                  variant="outline"
                  size="sm"
                  disabled={offset === 0}
                  onClick={() => setOffset(Math.max(0, offset - limit))}
                >
                  Previous
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  disabled={offset + limit >= total}
                  onClick={() => setOffset(offset + limit)}
                >
                  Next
                </Button>
              </div>
            </div>
          )}
        </CardContent>
      </Card>

      {selectedEntry && (
        <RequestDetailModal entry={selectedEntry} onClose={() => setSelectedEntry(null)} />
      )}
    </div>
  );
}
