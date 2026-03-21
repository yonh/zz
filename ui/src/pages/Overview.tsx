import { useMemo, useState, useEffect, useRef } from "react";
import {
  Activity,
  Server,
  ShieldCheck,
  ArrowRightLeft,
  TrendingUp,
  AlertTriangle,
  CheckCircle2,
  Radio,
} from "lucide-react";
import {
  LineChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  BarChart,
  Bar,
  Cell,
} from "recharts";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { useAppStore } from "@/stores/store";
import { generateRequestRateData } from "@/api/mock";
import { cn } from "@/lib/utils";

const CHART_COLORS = [
  "hsl(12, 76%, 61%)",
  "hsl(173, 58%, 39%)",
  "hsl(197, 37%, 24%)",
  "hsl(43, 74%, 66%)",
  "hsl(27, 87%, 67%)",
];

/**
 * Overview dashboard page with stats, charts, and activity feed.
 */
export default function Overview() {
  const systemStats = useAppStore((s) => s.systemStats);
  const logs = useAppStore((s) => s.logs);
  const providers = useAppStore((s) => s.providers);
  const requestRateData = useMemo(() => generateRequestRateData(), []);

  const recentLogs = logs.slice(0, 20);

  // Track newly arriving log IDs to animate them
  const [newLogIds, setNewLogIds] = useState<Set<string>>(new Set());
  const prevLogCountRef = useRef(logs.length);

  useEffect(() => {
    if (logs.length > prevLogCountRef.current) {
      const newIds = new Set(logs.slice(0, logs.length - prevLogCountRef.current).map((l) => l.id));
      setNewLogIds(newIds);
      const timer = setTimeout(() => setNewLogIds(new Set()), 1500);
      prevLogCountRef.current = logs.length;
      return () => clearTimeout(timer);
    }
    prevLogCountRef.current = logs.length;
  }, [logs]);

  // Live traffic distribution derived from provider stats
  const liveTrafficData = useMemo(() => {
    const active = providers.filter((p) => p.enabled && p.stats.total_requests > 0);
    const total = active.reduce((sum, p) => sum + p.stats.total_requests, 0);
    return active.map((p, i) => ({
      provider: p.name,
      requests: p.stats.total_requests,
      percentage: total > 0 ? Math.round((p.stats.total_requests / total) * 1000) / 10 : 0,
      color: CHART_COLORS[i % CHART_COLORS.length],
    }));
  }, [providers]);

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold tracking-tight">Overview</h1>
      </div>

      {/* Stats Cards */}
      <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Total Requests</CardTitle>
            <Activity className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">
              {systemStats.total_requests.toLocaleString()}
            </div>
            <p className="text-xs text-muted-foreground">
              {systemStats.requests_per_minute.toFixed(1)} req/min
            </p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Active Providers</CardTitle>
            <Server className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">
              {systemStats.active_providers}/{systemStats.total_providers}
            </div>
            <p className="text-xs text-muted-foreground">
              {systemStats.total_providers - systemStats.active_providers} disabled
            </p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Healthy Providers</CardTitle>
            <ShieldCheck className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">
              {systemStats.healthy_providers}/{systemStats.total_providers}
            </div>
            <p className="text-xs text-muted-foreground">
              {systemStats.active_providers - systemStats.healthy_providers} in cooldown
            </p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Strategy</CardTitle>
            <ArrowRightLeft className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold capitalize">
              {systemStats.strategy.replace("-", " ")}
            </div>
            <p className="text-xs text-muted-foreground">
              Uptime: {Math.floor(systemStats.uptime_secs / 3600)}h{" "}
              {Math.floor((systemStats.uptime_secs % 3600) / 60)}m
            </p>
          </CardContent>
        </Card>
      </div>

      {/* Charts Row */}
      <div className="grid gap-4 lg:grid-cols-7">
        <Card className="lg:col-span-4">
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <TrendingUp className="h-4 w-4" />
              Request Rate (Last 1h)
            </CardTitle>
          </CardHeader>
          <CardContent>
            <ResponsiveContainer width="100%" height={250}>
              <LineChart data={requestRateData}>
                <CartesianGrid strokeDasharray="3 3" className="stroke-border" />
                <XAxis
                  dataKey="time"
                  className="text-xs"
                  tick={{ fill: "hsl(var(--color-muted-foreground))", fontSize: 11 }}
                  tickLine={false}
                  interval={9}
                />
                <YAxis
                  className="text-xs"
                  tick={{ fill: "hsl(var(--color-muted-foreground))", fontSize: 11 }}
                  tickLine={false}
                  axisLine={false}
                />
                <Tooltip
                  contentStyle={{
                    backgroundColor: "hsl(var(--color-card))",
                    border: "1px solid hsl(var(--color-border))",
                    borderRadius: "8px",
                    fontSize: "12px",
                  }}
                  labelStyle={{ color: "hsl(var(--color-foreground))" }}
                />
                <Line
                  type="monotone"
                  dataKey="value"
                  stroke="hsl(12, 76%, 61%)"
                  strokeWidth={2}
                  dot={false}
                  name="Requests/min"
                />
              </LineChart>
            </ResponsiveContainer>
          </CardContent>
        </Card>

        <Card className="lg:col-span-3">
          <CardHeader>
            <CardTitle>Traffic Distribution</CardTitle>
          </CardHeader>
          <CardContent>
            <ResponsiveContainer width="100%" height={250}>
              <BarChart data={liveTrafficData} layout="vertical">
                <CartesianGrid strokeDasharray="3 3" className="stroke-border" horizontal={false} />
                <XAxis
                  type="number"
                  tick={{ fill: "hsl(var(--color-muted-foreground))", fontSize: 11 }}
                  tickLine={false}
                />
                <YAxis
                  type="category"
                  dataKey="provider"
                  tick={{ fill: "hsl(var(--color-muted-foreground))", fontSize: 11 }}
                  tickLine={false}
                  axisLine={false}
                  width={100}
                />
                <Tooltip
                  contentStyle={{
                    backgroundColor: "hsl(var(--color-card))",
                    border: "1px solid hsl(var(--color-border))",
                    borderRadius: "8px",
                    fontSize: "12px",
                  }}
                  formatter={(value, _name, props) => [
                    `${Number(value).toLocaleString()} (${(props.payload as { percentage: number }).percentage}%)`,
                    "Requests",
                  ]}
                />
                <Bar dataKey="requests" radius={[0, 4, 4, 0]}>
                  {liveTrafficData.map((_entry, index) => (
                    <Cell key={index} fill={CHART_COLORS[index % CHART_COLORS.length]} />
                  ))}
                </Bar>
              </BarChart>
            </ResponsiveContainer>
          </CardContent>
        </Card>
      </div>

      {/* Activity Feed */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            Recent Activity
            <Radio className="h-4 w-4 text-emerald-500 animate-pulse" />
            <span className="text-xs font-normal text-muted-foreground">Live</span>
          </CardTitle>
        </CardHeader>
        <CardContent>
          <div className="space-y-2 max-h-[400px] overflow-y-auto">
            {recentLogs.map((log) => (
              <div
                key={log.id}
                className={cn(
                  "flex items-center gap-3 px-3 py-2 rounded-md hover:bg-accent/50 transition-all duration-500 text-sm",
                  newLogIds.has(log.id) && "bg-primary/10 ring-1 ring-primary/20",
                )}
              >
                <span className="text-xs text-muted-foreground font-mono w-20 shrink-0">
                  {new Date(log.timestamp).toLocaleTimeString()}
                </span>
                <Badge
                  variant={
                    log.status >= 200 && log.status < 300
                      ? "success"
                      : log.status === 429
                        ? "warning"
                        : "danger"
                  }
                  className="w-12 justify-center"
                >
                  {log.status}
                </Badge>
                <span className="text-muted-foreground font-mono text-xs w-28 shrink-0 truncate">
                  {log.provider}
                </span>
                <span className="font-mono text-xs truncate flex-1">
                  {log.method} {log.path}
                </span>
                <span className="text-xs text-muted-foreground w-16 text-right shrink-0">
                  {log.duration_ms}ms
                </span>
                {log.failover_chain && (
                  <div className="flex items-center gap-1">
                    <AlertTriangle className="h-3 w-3 text-amber-500" />
                    <span className="text-xs text-amber-500">failover</span>
                  </div>
                )}
                {!log.failover_chain && log.status < 300 && (
                  <CheckCircle2
                    className={cn("h-3.5 w-3.5 text-emerald-500 shrink-0")}
                  />
                )}
              </div>
            ))}
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
