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
  ChevronDown,
  ChevronRight,
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
import { LogDetailPanel } from "@/components/LogDetailPanel";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { useAppStore } from "@/stores/store";
import { cn, formatDuration } from "@/lib/utils";
import { toast } from "sonner";

// Chart colors for light mode
const CHART_COLORS_LIGHT = [
  "hsl(142, 71%, 45%)",
  "hsl(217, 91%, 53%)",
  "hsl(278, 86%, 61%)",
  "hsl(31, 91%, 51%)",
  "hsl(0, 84%, 54%)",
];

// Chart colors for dark mode
const CHART_COLORS_DARK = [
  "hsl(142, 76%, 36%)",
  "hsl(217, 91%, 60%)",
  "hsl(278, 86%, 68%)",
  "hsl(31, 91%, 58%)",
  "hsl(0, 84%, 60%)",
];

// Theme-aware colors
const THEME_COLORS = {
  light: {
    tick: "hsl(215, 16%, 47%)",
    grid: "hsl(215, 16%, 88%)",
    tooltipBg: "hsl(0, 0%, 100%)",
    tooltipBorder: "hsl(215, 16%, 85%)",
    tooltipLabel: "hsl(222, 84%, 5%)",
  },
  dark: {
    tick: "hsl(215, 20%, 65%)",
    grid: "hsl(217, 32%, 25%)",
    tooltipBg: "hsl(222, 84%, 5%)",
    tooltipBorder: "hsl(217, 32%, 20%)",
    tooltipLabel: "hsl(210, 40%, 98%)",
  },
};

/**
 * Hook to detect dark mode
 */
function useDarkMode() {
  const [isDark, setIsDark] = useState(false);

  useEffect(() => {
    const checkDark = () => setIsDark(document.documentElement.classList.contains("dark"));
    checkDark();

    const observer = new MutationObserver(checkDark);
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ["class"] });

    return () => observer.disconnect();
  }, []);

  return isDark;
}

/**
 * Overview dashboard page with stats, charts, and activity feed.
 */
export default function Overview() {
  const systemStats = useAppStore((s) => s.systemStats);
  const logs = useAppStore((s) => s.logs);
  const providers = useAppStore((s) => s.providers);
  const setStrategy = useAppStore((s) => s.setStrategy);
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const isDark = useDarkMode();

  // Theme-aware colors
  const chartColors = isDark ? CHART_COLORS_DARK : CHART_COLORS_LIGHT;
  const themeColors = isDark ? THEME_COLORS.dark : THEME_COLORS.light;

  // Generate sample data for demonstration
  const requestRateData = useMemo(() => {
    const data = [];
    for (let i = 0; i < 12; i++) {
      const hour = new Date();
      hour.setHours(hour.getHours() - (11 - i));
      data.push({
        time: hour.toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit' }),
        value: Math.floor(Math.random() * 30) + 10
      });
    }
    return data;
  }, []);

  const recentLogs = logs.slice(0, 20);

  const strategyOptions = [
    { value: "failover", label: "Failover" },
    { value: "round-robin", label: "Round Robin" },
    { value: "weighted-random", label: "Weighted Random" },
    { value: "quota-aware", label: "Quota-Aware" },
    { value: "manual", label: "Manual / Fixed" },
  ];

  function handleStrategyChange(value: string) {
    setStrategy(value as typeof systemStats.strategy);
    toast.success(`Strategy changed to ${value}`);
  }

  // Track newly arriving log IDs to animate them
  const [newLogIds, setNewLogIds] = useState<Set<string>>(new Set());
  const prevLogCountRef = useRef(logs.length);

  useEffect(() => {
    if (logs.length > prevLogCountRef.current) {
      // New logs are prepended to the beginning
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
      color: chartColors[i % chartColors.length],
    }));
  }, [providers, chartColors]);

  return (
    <div className="flex flex-col flex-1 overflow-hidden gap-6">
      <div className="flex items-center justify-between shrink-0">
        <h1 className="text-2xl font-bold tracking-tight">Overview</h1>
      </div>

      {/* Stats Cards */}
      <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4 shrink-0">
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
            <Select
              value={systemStats.strategy}
              onValueChange={handleStrategyChange}
            >
              <SelectTrigger className="h-8 text-sm">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {strategyOptions.map((opt) => (
                  <SelectItem key={opt.value} value={opt.value}>
                    {opt.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <p className="text-xs text-muted-foreground mt-1">
              Uptime: {Math.floor(systemStats.uptime_secs / 3600)}h{" "}
              {Math.floor((systemStats.uptime_secs % 3600) / 60)}m
            </p>
          </CardContent>
        </Card>
      </div>

      {/* Charts Row */}
      <div className="grid gap-4 lg:grid-cols-7 shrink-0">
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
                <CartesianGrid
                  strokeDasharray="3 3"
                  stroke={themeColors.grid}
                  strokeOpacity={0.5}
                />
                <XAxis
                  dataKey="time"
                  tick={{ fill: themeColors.tick, fontSize: 11 }}
                  tickLine={false}
                  interval={9}
                />
                <YAxis
                  tick={{ fill: themeColors.tick, fontSize: 11 }}
                  tickLine={false}
                  axisLine={false}
                />
                <Tooltip
                  contentStyle={{
                    backgroundColor: themeColors.tooltipBg,
                    border: `1px solid ${themeColors.tooltipBorder}`,
                    borderRadius: "8px",
                    fontSize: "12px",
                    color: themeColors.tooltipLabel,
                  }}
                  labelStyle={{ color: themeColors.tooltipLabel }}
                  itemStyle={{ color: themeColors.tooltipLabel }}
                />
                <Line
                  type="monotone"
                  dataKey="value"
                  stroke={chartColors[0]}
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
                <CartesianGrid
                  strokeDasharray="3 3"
                  stroke={themeColors.grid}
                  horizontal={false}
                  strokeOpacity={0.5}
                />
                <XAxis
                  type="number"
                  tick={{ fill: themeColors.tick, fontSize: 11 }}
                  tickLine={false}
                />
                <YAxis
                  type="category"
                  dataKey="provider"
                  tick={{ fill: themeColors.tick, fontSize: 11 }}
                  tickLine={false}
                  axisLine={false}
                  width={100}
                />
                <Tooltip
                  contentStyle={{
                    backgroundColor: themeColors.tooltipBg,
                    border: `1px solid ${themeColors.tooltipBorder}`,
                    borderRadius: "8px",
                    fontSize: "12px",
                    color: themeColors.tooltipLabel,
                  }}
                  labelStyle={{ color: themeColors.tooltipLabel }}
                  itemStyle={{ color: themeColors.tooltipLabel }}
                  formatter={(value, _name, props) => [
                    `${Number(value).toLocaleString()} (${(props.payload as { percentage: number }).percentage}%)`,
                    "Requests",
                  ]}
                />
                <Bar dataKey="requests" radius={[0, 6, 6, 0]}>
                  {liveTrafficData.map((_entry, index) => (
                    <Cell
                      key={`cell-${index}`}
                      fill={chartColors[index % chartColors.length]}
                      fillOpacity={0.85}
                    />
                  ))}
                </Bar>
              </BarChart>
            </ResponsiveContainer>
          </CardContent>
        </Card>
      </div>

      {/* Activity Feed */}
      <Card className="flex-1 flex flex-col min-h-0">
        <CardHeader className="shrink-0">
          <CardTitle className="flex items-center gap-2">
            Recent Activity
            <Radio className="h-4 w-4 text-primary animate-pulse" />
            <span className="text-xs font-normal text-muted-foreground">Live</span>
          </CardTitle>
        </CardHeader>
        <CardContent className="flex-1 flex flex-col min-h-0">
          <div className="space-y-1 flex-1 overflow-y-auto min-h-0">
            {recentLogs.map((log) => {
              const isExpanded = expandedId === log.id;
              return (
                <div key={log.id}>
                  <div
                    className={cn(
                      "flex items-center gap-3 px-3 py-2 rounded-md hover:bg-accent/50 transition-all duration-500 text-sm cursor-pointer",
                      newLogIds.has(log.id) && "bg-primary/10 ring-1 ring-primary/20",
                      isExpanded && "bg-accent/30",
                    )}
                    onClick={() => setExpandedId(isExpanded ? null : log.id)}
                  >
                    {isExpanded ? (
                      <ChevronDown className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
                    ) : (
                      <ChevronRight className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
                    )}
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
                    <span className="font-mono text-xs w-24 shrink-0 truncate text-muted-foreground">
                      {log.model}
                    </span>
                    <span className="font-mono text-xs truncate flex-1">
                      {log.method} {log.path}
                    </span>
                    <span className="text-xs text-muted-foreground w-16 text-right shrink-0">
                      {formatDuration(log.duration_ms)}
                    </span>
                    <span className="text-xs text-muted-foreground w-14 text-right shrink-0 font-mono">
                      {log.token_usage ? log.token_usage.total_tokens.toLocaleString() : "-"}
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
                  {isExpanded && <LogDetailPanel log={log} />}
                </div>
              );
            })}
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
