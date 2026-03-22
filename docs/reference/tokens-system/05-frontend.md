# ZZ - Token Statistics Frontend Implementation

## Version: 1.0.0

---

## 1. State Management Architecture

### 1.1 Store Structure

**File**: `ui/src/stores/tokenStore.ts`

```typescript
import { create } from 'zustand';
import { devtools, persist } from 'zustand/middleware';

// Types
export interface TokenStats {
  totalTokens: number;
  inputTokens: number;
  outputTokens: number;
  cachedTokens: number;
  totalCostUsd: number;
  requestCount: number;
  successCount: number;
  errorCount: number;
  avgDurationMs: number;
}

export interface TimeSeriesPoint {
  time: string;
  inputTokens: number;
  outputTokens: number;
  totalTokens: number;
  costUsd: number;
  requestCount: number;
}

export interface ProviderTokenStats extends TokenStats {
  provider: string;
  quota: QuotaInfo | null;
}

export interface QuotaInfo {
  provider: string;
  monthlyTokenBudget: number | null;
  monthlyCostBudgetUsd: number | null;
  tokensUsed: number;
  costUsedUsd: number;
  usagePercent: number;
  alertThreshold: number;
  resetDay: number;
  periodStart: string;
  daysUntilReset: number;
}

export interface ModelPricing {
  modelPattern: string;
  inputPricePer1k: number;
  outputPricePer1k: number;
  effectiveFrom: string;
  effectiveUntil: string | null;
}

// Store State
interface TokenState {
  // Summary data
  summary: {
    today: TokenStats | null;
    yesterday: TokenStats | null;
    thisWeek: TokenStats | null;
    thisMonth: TokenStats | null;
    lastMonth: TokenStats | null;
  } | null;
  
  // Time series data
  timeSeries: TimeSeriesPoint[];
  timeSeriesMeta: {
    start: string;
    end: string;
    granularity: string;
    loading: boolean;
  };
  
  // Provider stats
  providerStats: ProviderTokenStats[];
  providerStatsMeta: {
    period: string;
    loading: boolean;
  };
  
  // Model stats
  modelStats: ModelTokenStats[];
  modelStatsMeta: {
    period: string;
    loading: boolean;
  };
  
  // Quotas
  quotas: QuotaInfo[];
  quotasLoading: boolean;
  
  // Pricing
  pricing: {
    defaultPricing: { inputPricePer1k: number; outputPricePer1k: number };
    modelPricing: ModelPricing[];
  } | null;
  pricingLoading: boolean;
  
  // UI State
  selectedTimeRange: 'today' | 'week' | 'month' | 'custom';
  customTimeRange: { start: Date; end: Date } | null;
  selectedGranularity: 'minute' | 'hour' | 'day' | 'week' | 'month';
  selectedProvider: string | null;
  selectedModel: string | null;
  
  // Loading states
  summaryLoading: boolean;
  
  // Error states
  errors: {
    summary: string | null;
    timeSeries: string | null;
    quotas: string | null;
    pricing: string | null;
  };
  
  // Actions
  fetchSummary: () => Promise<void>;
  fetchTimeSeries: (params: TimeSeriesParams) => Promise<void>;
  fetchProviderStats: (period: string) => Promise<void>;
  fetchModelStats: (period: string) => Promise<void>;
  fetchQuotas: () => Promise<void>;
  updateQuota: (quota: QuotaUpdate) => Promise<void>;
  fetchPricing: () => Promise<void>;
  updatePricing: (pricing: PricingUpdate) => Promise<void>;
  setTimeRange: (range: TimeRange) => void;
  setGranularity: (granularity: string) => void;
  setSelectedProvider: (provider: string | null) => void;
  setSelectedModel: (model: string | null) => void;
  clearErrors: () => void;
}

// Store Implementation
export const useTokenStore = create<TokenState>()(
  devtools(
    (set, get) => ({
      // Initial state
      summary: null,
      timeSeries: [],
      timeSeriesMeta: {
        start: '',
        end: '',
        granularity: 'hour',
        loading: false,
      },
      providerStats: [],
      providerStatsMeta: { period: 'month', loading: false },
      modelStats: [],
      modelStatsMeta: { period: 'month', loading: false },
      quotas: [],
      quotasLoading: false,
      pricing: null,
      pricingLoading: false,
      selectedTimeRange: 'month',
      customTimeRange: null,
      selectedGranularity: 'hour',
      selectedProvider: null,
      selectedModel: null,
      summaryLoading: false,
      errors: {
        summary: null,
        timeSeries: null,
        quotas: null,
        pricing: null,
      },
      
      // Actions
      fetchSummary: async () => {
        set({ summaryLoading: true, errors: { ...get().errors, summary: null } });
        try {
          const response = await fetch('/zz/api/tokens/summary');
          if (!response.ok) throw new Error('Failed to fetch summary');
          const data = await response.json();
          set({ summary: data, summaryLoading: false });
        } catch (error) {
          set({
            summaryLoading: false,
            errors: { ...get().errors, summary: (error as Error).message }
          });
        }
      },
      
      fetchTimeSeries: async (params) => {
        set({ timeSeriesMeta: { ...get().timeSeriesMeta, loading: true } });
        try {
          const query = new URLSearchParams({
            start: params.start,
            end: params.end,
            granularity: params.granularity || 'hour',
            ...(params.provider && { provider: params.provider }),
            ...(params.model && { model: params.model }),
          });
          const response = await fetch(`/zz/api/tokens/timeseries?${query}`);
          if (!response.ok) throw new Error('Failed to fetch time series');
          const data = await response.json();
          set({
            timeSeries: data.data,
            timeSeriesMeta: {
              start: data.start,
              end: data.end,
              granularity: data.granularity,
              loading: false,
            },
          });
        } catch (error) {
          set({
            timeSeriesMeta: { ...get().timeSeriesMeta, loading: false },
            errors: { ...get().errors, timeSeries: (error as Error).message }
          });
        }
      },
      
      // ... other actions
    }),
    { name: 'token-store' }
  )
);
```

### 1.2 React Query Integration (Alternative)

For more complex caching and refetching:

```typescript
// ui/src/hooks/useTokenQueries.ts
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';

export const useTokenSummary = () => {
  return useQuery({
    queryKey: ['tokenSummary'],
    queryFn: async () => {
      const response = await fetch('/zz/api/tokens/summary');
      if (!response.ok) throw new Error('Failed to fetch summary');
      return response.json();
    },
    staleTime: 5 * 1000, // 5 seconds
    refetchInterval: 30 * 1000, // Auto-refresh every 30s
  });
};

export const useTimeSeries = (params: TimeSeriesParams) => {
  return useQuery({
    queryKey: ['timeSeries', params],
    queryFn: async () => {
      const query = new URLSearchParams({
        start: params.start,
        end: params.end,
        granularity: params.granularity,
      });
      const response = await fetch(`/zz/api/tokens/timeseries?${query}`);
      if (!response.ok) throw new Error('Failed to fetch time series');
      return response.json();
    },
    staleTime: 60 * 1000, // 1 minute
  });
};

export const useQuotas = () => {
  return useQuery({
    queryKey: ['quotas'],
    queryFn: async () => {
      const response = await fetch('/zz/api/quotas');
      if (!response.ok) throw new Error('Failed to fetch quotas');
      return response.json();
    },
  });
};

export const useUpdateQuota = () => {
  const queryClient = useQueryClient();
  
  return useMutation({
    mutationFn: async (quotas: QuotaUpdate[]) => {
      const response = await fetch('/zz/api/quotas', {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ quotas }),
      });
      if (!response.ok) throw new Error('Failed to update quota');
      return response.json();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['quotas'] });
      queryClient.invalidateQueries({ queryKey: ['providerStats'] });
    },
  });
};
```

---

## 2. Component Implementations

### 2.1 TokensPage

**File**: `ui/src/pages/Tokens.tsx`

```tsx
import React, { useEffect, useState } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Button } from '@/components/ui/button';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Skeleton } from '@/components/ui/skeleton';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Download, AlertCircle, TrendingUp, TrendingDown } from 'lucide-react';
import { useTokenSummary, useTimeSeries, useProviderStats, useModelStats } from '@/hooks/useTokenQueries';
import { TokenTrendChart } from '@/components/tokens/TokenTrendChart';
import { ProviderPieChart } from '@/components/tokens/ProviderPieChart';
import { ModelPieChart } from '@/components/tokens/ModelPieChart';
import { TokenDataTable } from '@/components/tokens/TokenDataTable';
import { StatsCard } from '@/components/tokens/StatsCard';
import { TimeRangePicker } from '@/components/tokens/TimeRangePicker';
import { exportAsCsv, exportAsJson } from '@/lib/export';

type TimeRange = 'today' | 'week' | 'month' | 'custom';

export const TokensPage: React.FC = () => {
  const [timeRange, setTimeRange] = useState<TimeRange>('month');
  const [granularity, setGranularity] = useState<'hour' | 'day' | 'week'>('hour');
  const [customRange, setCustomRange] = useState<{ start: Date; end: Date } | null>(null);
  const [exportFormat, setExportFormat] = useState<'csv' | 'json' | null>(null);
  
  // Queries
  const { data: summary, isLoading: summaryLoading, error: summaryError } = useTokenSummary();
  
  // Calculate time range for API
  const getTimeRangeParams = () => {
    const now = new Date();
    if (timeRange === 'custom' && customRange) {
      return { start: customRange.start.toISOString(), end: customRange.end.toISOString() };
    }
    const ranges = {
      today: { start: new Date(now.setHours(0, 0, 0, 0)), end: new Date() },
      week: { start: new Date(now.setDate(now.getDate() - 7)), end: new Date() },
      month: { start: new Date(now.setDate(now.getDate() - 30)), end: new Date() },
    };
    const range = ranges[timeRange];
    return { start: range.start.toISOString(), end: range.end.toISOString() };
  };
  
  const timeParams = getTimeRangeParams();
  const { data: timeSeries, isLoading: timeSeriesLoading } = useTimeSeries({
    start: timeParams.start,
    end: timeParams.end,
    granularity,
  });
  
  const { data: providerStats } = useProviderStats(timeRange);
  const { data: modelStats } = useModelStats(timeRange);
  
  // Auto-adjust granularity based on time range
  useEffect(() => {
    if (timeRange === 'today') setGranularity('hour');
    else if (timeRange === 'week') setGranularity('hour');
    else if (timeRange === 'month') setGranularity('day');
    else setGranularity('day');
  }, [timeRange]);
  
  // Export handler
  const handleExport = async (format: 'csv' | 'json') => {
    const params = new URLSearchParams({
      format,
      start: timeParams.start,
      end: timeParams.end,
    });
    
    const response = await fetch(`/zz/api/tokens/export?${params}`);
    if (format === 'csv') {
      const text = await response.text();
      downloadFile(text, `token_export_${new Date().toISOString().split('T')[0]}.csv`, 'text/csv');
    } else {
      const data = await response.json();
      downloadFile(JSON.stringify(data, null, 2), `token_export_${new Date().toISOString().split('T')[0]}.json`, 'application/json');
    }
  };
  
  // Get current period stats
  const currentStats = summary?.[timeRange === 'today' ? 'today' : timeRange === 'week' ? 'thisWeek' : 'thisMonth'];
  const previousStats = summary?.[timeRange === 'today' ? 'yesterday' : timeRange === 'week' ? 'lastWeek' : 'lastMonth'];
  
  // Calculate trends
  const calcTrend = (current?: number, previous?: number) => {
    if (!current || !previous) return null;
    const change = ((current - previous) / previous) * 100;
    return { value: Math.abs(change).toFixed(1), direction: change >= 0 ? 'up' : 'down' };
  };
  
  const tokenTrend = calcTrend(currentStats?.totalTokens, previousStats?.totalTokens);
  const costTrend = calcTrend(currentStats?.totalCostUsd, previousStats?.totalCostUsd);
  
  if (summaryError) {
    return (
      <div className="p-6">
        <Alert variant="destructive">
          <AlertCircle className="h-4 w-4" />
          <AlertDescription>
            Failed to load token statistics: {summaryError.message}
          </AlertDescription>
        </Alert>
      </div>
    );
  }
  
  return (
    <div className="p-6 space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold">Token Analytics</h1>
        <div className="flex items-center gap-4">
          <TimeRangePicker
            value={timeRange}
            onChange={setTimeRange}
            customRange={customRange}
            onCustomRangeChange={setCustomRange}
          />
          <Select value={granularity} onValueChange={(v) => setGranularity(v as any)}>
            <SelectTrigger className="w-32">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="hour">Hourly</SelectItem>
              <SelectItem value="day">Daily</SelectItem>
              <SelectItem value="week">Weekly</SelectItem>
            </SelectContent>
          </Select>
        </div>
      </div>
      
      {/* Stats Cards */}
      <div className="grid grid-cols-5 gap-4">
        <StatsCard
          title="Total Tokens"
          value={formatNumber(currentStats?.totalTokens)}
          trend={tokenTrend}
          icon={<TrendingUp className="h-4 w-4" />}
          loading={summaryLoading}
        />
        <StatsCard
          title="Input Tokens"
          value={formatNumber(currentStats?.inputTokens)}
          loading={summaryLoading}
        />
        <StatsCard
          title="Output Tokens"
          value={formatNumber(currentStats?.outputTokens)}
          loading={summaryLoading}
        />
        <StatsCard
          title="Cost"
          value={formatCurrency(currentStats?.totalCostUsd)}
          trend={costTrend}
          loading={summaryLoading}
        />
        <StatsCard
          title="Requests"
          value={formatNumber(currentStats?.requestCount)}
          loading={summaryLoading}
        />
      </div>
      
      {/* Trend Chart */}
      <Card>
        <CardHeader>
          <CardTitle>Token Consumption Trend</CardTitle>
        </CardHeader>
        <CardContent className="h-80">
          {timeSeriesLoading ? (
            <Skeleton className="w-full h-full" />
          ) : (
            <TokenTrendChart data={timeSeries?.data || []} granularity={granularity} />
          )}
        </CardContent>
      </Card>
      
      {/* Distribution Charts */}
      <div className="grid grid-cols-2 gap-6">
        <Card>
          <CardHeader>
            <CardTitle>By Provider</CardTitle>
          </CardHeader>
          <CardContent className="h-64">
            <ProviderPieChart data={providerStats?.providers || []} />
          </CardContent>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>By Model</CardTitle>
          </CardHeader>
          <CardContent className="h-64">
            <ModelPieChart data={modelStats?.models || []} />
          </CardContent>
        </Card>
      </div>
      
      {/* Data Table */}
      <Card>
        <CardHeader className="flex flex-row items-center justify-between">
          <CardTitle>Detailed Data</CardTitle>
          <div className="flex gap-2">
            <Button variant="outline" size="sm" onClick={() => handleExport('csv')}>
              <Download className="h-4 w-4 mr-2" />
              Export CSV
            </Button>
            <Button variant="outline" size="sm" onClick={() => handleExport('json')}>
              <Download className="h-4 w-4 mr-2" />
              Export JSON
            </Button>
          </div>
        </CardHeader>
        <CardContent>
          <TokenDataTable
            startTime={timeParams.start}
            endTime={timeParams.end}
          />
        </CardContent>
      </Card>
    </div>
  );
};

// Utility functions
function formatNumber(value?: number): string {
  if (value === undefined) return '-';
  if (value >= 1000000) return `${(value / 1000000).toFixed(1)}M`;
  if (value >= 1000) return `${(value / 1000).toFixed(1)}K`;
  return value.toString();
}

function formatCurrency(value?: number): string {
  if (value === undefined) return '-';
  return `$${value.toFixed(2)}`;
}

function downloadFile(content: string, filename: string, mimeType: string) {
  const blob = new Blob([content], { type: mimeType });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}
```

### 2.2 StatsCard Component

**File**: `ui/src/components/tokens/StatsCard.tsx`

```tsx
import React from 'react';
import { Card, CardContent } from '@/components/ui/card';
import { Skeleton } from '@/components/ui/skeleton';
import { TrendingUp, TrendingDown, Minus } from 'lucide-react';
import { cn } from '@/lib/utils';

interface Trend {
  value: string;
  direction: 'up' | 'down';
}

interface StatsCardProps {
  title: string;
  value: string;
  trend?: Trend | null;
  icon?: React.ReactNode;
  loading?: boolean;
  className?: string;
}

export const StatsCard: React.FC<StatsCardProps> = ({
  title,
  value,
  trend,
  icon,
  loading,
  className,
}) => {
  if (loading) {
    return (
      <Card className={className}>
        <CardContent className="p-4">
          <Skeleton className="h-4 w-20 mb-2" />
          <Skeleton className="h-8 w-24" />
        </CardContent>
      </Card>
    );
  }
  
  return (
    <Card className={className}>
      <CardContent className="p-4">
        <div className="flex items-center justify-between mb-1">
          <span className="text-sm text-muted-foreground">{title}</span>
          {icon && <span className="text-muted-foreground">{icon}</span>}
        </div>
        <div className="flex items-end gap-2">
          <span className="text-2xl font-bold">{value}</span>
          {trend && (
            <span className={cn(
              "flex items-center text-xs",
              trend.direction === 'up' ? 'text-green-500' : 'text-red-500'
            )}>
              {trend.direction === 'up' ? (
                <TrendingUp className="h-3 w-3 mr-0.5" />
              ) : (
                <TrendingDown className="h-3 w-3 mr-0.5" />
              )}
              {trend.value}%
            </span>
          )}
        </div>
      </CardContent>
    </Card>
  );
};
```

### 2.3 TokenTrendChart Component

**File**: `ui/src/components/tokens/TokenTrendChart.tsx`

```tsx
import React from 'react';
import {
  LineChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  Legend,
  ResponsiveContainer,
} from 'recharts';
import { TimeSeriesPoint } from '@/stores/tokenStore';

interface TokenTrendChartProps {
  data: TimeSeriesPoint[];
  granularity: 'hour' | 'day' | 'week' | 'month';
}

export const TokenTrendChart: React.FC<TokenTrendChartProps> = ({
  data,
  granularity,
}) => {
  const formatXAxis = (tickItem: string) => {
    const date = new Date(tickItem);
    switch (granularity) {
      case 'hour':
        return date.toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit' });
      case 'day':
        return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
      case 'week':
        return `Week ${getWeekNumber(date)}`;
      case 'month':
        return date.toLocaleDateString('en-US', { month: 'short', year: 'numeric' });
      default:
        return tickItem;
    }
  };
  
  const formatYAxis = (value: number) => {
    if (value >= 1000000) return `${(value / 1000000).toFixed(1)}M`;
    if (value >= 1000) return `${(value / 1000).toFixed(0)}K`;
    return value.toString();
  };
  
  if (data.length === 0) {
    return (
      <div className="flex items-center justify-center h-full text-muted-foreground">
        No data available for selected time range
      </div>
    );
  }
  
  return (
    <ResponsiveContainer width="100%" height="100%">
      <LineChart data={data} margin={{ top: 5, right: 30, left: 20, bottom: 5 }}>
        <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" />
        <XAxis
          dataKey="time"
          tickFormatter={formatXAxis}
          stroke="hsl(var(--muted-foreground))"
          tick={{ fill: 'hsl(var(--muted-foreground))', fontSize: 12 }}
        />
        <YAxis
          yAxisId="tokens"
          tickFormatter={formatYAxis}
          stroke="hsl(var(--muted-foreground))"
          tick={{ fill: 'hsl(var(--muted-foreground))', fontSize: 12 }}
        />
        <YAxis
          yAxisId="cost"
          orientation="right"
          tickFormatter={(v) => `$${v.toFixed(2)}`}
          stroke="hsl(var(--muted-foreground))"
          tick={{ fill: 'hsl(var(--muted-foreground))', fontSize: 12 }}
        />
        <Tooltip
          contentStyle={{
            backgroundColor: 'hsl(var(--card))',
            border: '1px solid hsl(var(--border))',
            borderRadius: '8px',
          }}
          labelFormatter={(label) => new Date(label).toLocaleString()}
          formatter={(value: number, name: string) => {
            if (name === 'costUsd') return [`$${value.toFixed(4)}`, 'Cost'];
            return [formatYAxis(value), name];
          }}
        />
        <Legend />
        <Line
          yAxisId="tokens"
          type="monotone"
          dataKey="inputTokens"
          name="Input Tokens"
          stroke="#3b82f6"
          strokeWidth={2}
          dot={false}
        />
        <Line
          yAxisId="tokens"
          type="monotone"
          dataKey="outputTokens"
          name="Output Tokens"
          stroke="#10b981"
          strokeWidth={2}
          dot={false}
        />
        <Line
          yAxisId="cost"
          type="monotone"
          dataKey="costUsd"
          name="Cost ($)"
          stroke="#f59e0b"
          strokeWidth={2}
          dot={false}
          strokeDasharray="5 5"
        />
      </LineChart>
    </ResponsiveContainer>
  );
};

function getWeekNumber(date: Date): number {
  const firstDayOfYear = new Date(date.getFullYear(), 0, 1);
  const pastDaysOfYear = (date.getTime() - firstDayOfYear.getTime()) / 86400000;
  return Math.ceil((pastDaysOfYear + firstDayOfYear.getDay() + 1) / 7);
}
```

### 2.4 QuotasPage

**File**: `ui/src/pages/Quotas.tsx`

```tsx
import React, { useState } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Slider } from '@/components/ui/slider';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Progress } from '@/components/ui/progress';
import { Skeleton } from '@/components/ui/skeleton';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { useQuotas, useUpdateQuota } from '@/hooks/useTokenQueries';
import { useToast } from '@/hooks/useToast';
import { AlertCircle, Check, Save } from 'lucide-react';

interface QuotaFormData {
  provider: string;
  monthlyTokenBudget: number | null;
  monthlyCostBudgetUsd: number | null;
  alertThreshold: number;
  resetDay: number;
}

export const QuotasPage: React.FC = () => {
  const { data, isLoading, error } = useQuotas();
  const updateQuota = useUpdateQuota();
  const { toast } = useToast();
  
  const [formData, setFormData] = useState<Record<string, QuotaFormData>>({});
  const [hasChanges, setHasChanges] = useState<Record<string, boolean>>({});
  
  // Initialize form data from API response
  React.useEffect(() => {
    if (data?.quotas) {
      const initial: Record<string, QuotaFormData> = {};
      data.quotas.forEach(q => {
        initial[q.provider] = {
          provider: q.provider,
          monthlyTokenBudget: q.monthlyTokenBudget,
          monthlyCostBudgetUsd: q.monthlyCostBudgetUsd,
          alertThreshold: q.alertThreshold,
          resetDay: q.resetDay,
        };
      });
      setFormData(initial);
    }
  }, [data]);
  
  const handleFieldChange = (provider: string, field: keyof QuotaFormData, value: any) => {
    setFormData(prev => ({
      ...prev,
      [provider]: { ...prev[provider], [field]: value },
    }));
    setHasChanges(prev => ({ ...prev, [provider]: true }));
  };
  
  const handleSave = async (provider: string) => {
    const quotaData = formData[provider];
    if (!quotaData) return;
    
    // Validation
    const errors: string[] = [];
    if (quotaData.monthlyTokenBudget !== null && quotaData.monthlyTokenBudget <= 0) {
      errors.push('Token budget must be greater than 0');
    }
    if (quotaData.monthlyCostBudgetUsd !== null && quotaData.monthlyCostBudgetUsd <= 0) {
      errors.push('Cost budget must be greater than 0');
    }
    if (quotaData.alertThreshold < 0.5 || quotaData.alertThreshold > 1) {
      errors.push('Alert threshold must be between 50% and 100%');
    }
    if (quotaData.resetDay < 1 || quotaData.resetDay > 28) {
      errors.push('Reset day must be between 1 and 28');
    }
    
    if (errors.length > 0) {
      toast({
        title: 'Validation Error',
        description: errors.join(', '),
        variant: 'destructive',
      });
      return;
    }
    
    try {
      await updateQuota.mutateAsync([quotaData]);
      setHasChanges(prev => ({ ...prev, [provider]: false }));
      toast({
        title: 'Quota Updated',
        description: `Quota for ${provider} has been saved.`,
      });
    } catch (error) {
      toast({
        title: 'Error',
        description: `Failed to update quota: ${(error as Error).message}`,
        variant: 'destructive',
      });
    }
  };
  
  if (isLoading) {
    return (
      <div className="p-6 space-y-6">
        {[1, 2, 3].map(i => (
          <Card key={i}>
            <CardContent className="p-6">
              <Skeleton className="h-40 w-full" />
            </CardContent>
          </Card>
        ))}
      </div>
    );
  }
  
  if (error) {
    return (
      <div className="p-6">
        <Alert variant="destructive">
          <AlertCircle className="h-4 w-4" />
          <AlertDescription>Failed to load quotas: {error.message}</AlertDescription>
        </Alert>
      </div>
    );
  }
  
  return (
    <div className="p-6 space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold">Quota Management</h1>
      </div>
      
      {data?.quotas.map(quota => {
        const formQuota = formData[quota.provider];
        if (!formQuota) return null;
        
        const usagePercent = quota.currentUsage?.usagePercent || 0;
        const isOverThreshold = usagePercent >= formQuota.alertThreshold * 100;
        const isOverBudget = usagePercent >= 100;
        
        return (
          <Card key={quota.provider} className={isOverBudget ? 'border-red-500' : isOverThreshold ? 'border-amber-500' : ''}>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                {quota.provider}
                {hasChanges[quota.provider] && (
                  <span className="text-xs text-muted-foreground">(unsaved changes)</span>
                )}
              </CardTitle>
            </CardHeader>
            <CardContent className="space-y-6">
              {/* Token Budget Section */}
              <div className="grid grid-cols-2 gap-6">
                <div className="space-y-4">
                  <div>
                    <Label>Token Budget (monthly)</Label>
                    <div className="flex items-center gap-2 mt-1">
                      <Input
                        type="number"
                        placeholder="Unlimited"
                        value={formQuota.monthlyTokenBudget ?? ''}
                        onChange={(e) => handleFieldChange(
                          quota.provider,
                          'monthlyTokenBudget',
                          e.target.value ? parseInt(e.target.value) : null
                        )}
                      />
                      <span className="text-sm text-muted-foreground">tokens</span>
                    </div>
                  </div>
                  
                  {/* Token Progress */}
                  {formQuota.monthlyTokenBudget && (
                    <div className="space-y-2">
                      <div className="flex justify-between text-sm">
                        <span>Used: {formatNumber(quota.currentUsage?.tokensUsed || 0)}</span>
                        <span>Remaining: {formatNumber(formQuota.monthlyTokenBudget - (quota.currentUsage?.tokensUsed || 0))}</span>
                      </div>
                      <Progress
                        value={Math.min(usagePercent, 100)}
                        className={isOverBudget ? 'bg-red-100' : isOverThreshold ? 'bg-amber-100' : ''}
                      />
                    </div>
                  )}
                </div>
                
                <div className="space-y-4">
                  <div>
                    <Label>Cost Budget (monthly)</Label>
                    <div className="flex items-center gap-2 mt-1">
                      <span className="text-sm">$</span>
                      <Input
                        type="number"
                        step="0.01"
                        placeholder="Unlimited"
                        value={formQuota.monthlyCostBudgetUsd ?? ''}
                        onChange={(e) => handleFieldChange(
                          quota.provider,
                          'monthlyCostBudgetUsd',
                          e.target.value ? parseFloat(e.target.value) : null
                        )}
                      />
                    </div>
                  </div>
                  
                  {/* Cost Progress */}
                  {formQuota.monthlyCostBudgetUsd && (
                    <div className="space-y-2">
                      <div className="flex justify-between text-sm">
                        <span>Used: ${quota.currentUsage?.costUsedUsd.toFixed(2) || '0.00'}</span>
                        <span>Remaining: ${(formQuota.monthlyCostBudgetUsd - (quota.currentUsage?.costUsedUsd || 0)).toFixed(2)}</span>
                      </div>
                      <Progress
                        value={Math.min((quota.currentUsage?.costUsedUsd || 0) / formQuota.monthlyCostBudgetUsd * 100, 100)}
                      />
                    </div>
                  )}
                </div>
              </div>
              
              {/* Settings Row */}
              <div className="grid grid-cols-3 gap-4 pt-4 border-t">
                <div>
                  <Label>Alert Threshold</Label>
                  <div className="flex items-center gap-2 mt-1">
                    <Slider
                      value={[formQuota.alertThreshold * 100]}
                      min={50}
                      max={100}
                      step={5}
                      onValueChange={([v]) => handleFieldChange(quota.provider, 'alertThreshold', v / 100)}
                      className="flex-1"
                    />
                    <span className="w-12 text-sm text-right">{Math.round(formQuota.alertThreshold * 100)}%</span>
                  </div>
                </div>
                
                <div>
                  <Label>Reset Day</Label>
                  <Select
                    value={formQuota.resetDay.toString()}
                    onValueChange={(v) => handleFieldChange(quota.provider, 'resetDay', parseInt(v))}
                  >
                    <SelectTrigger>
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {Array.from({ length: 28 }, (_, i) => i + 1).map(day => (
                        <SelectItem key={day} value={day.toString()}>
                          {day}{getOrdinalSuffix(day)} of each month
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </div>
                
                <div className="flex items-end">
                  <Button
                    onClick={() => handleSave(quota.provider)}
                    disabled={!hasChanges[quota.provider]}
                    className="w-full"
                  >
                    <Save className="h-4 w-4 mr-2" />
                    Save Changes
                  </Button>
                </div>
              </div>
              
              {/* Status Footer */}
              <div className="flex items-center justify-between text-sm text-muted-foreground pt-4 border-t">
                <span>Days until reset: {quota.currentUsage?.daysUntilReset || '-'}</span>
                <span>Period started: {new Date(quota.currentUsage?.periodStart || '').toLocaleDateString()}</span>
              </div>
            </CardContent>
          </Card>
        );
      })}
    </div>
  );
};

function getOrdinalSuffix(n: number): string {
  const s = ['th', 'st', 'nd', 'rd'];
  const v = n % 100;
  return s[(v - 20) % 10] || s[v] || s[0];
}
```

---

## 3. WebSocket Integration

### 3.1 Token Update Hook

**File**: `ui/src/hooks/useTokenUpdates.ts`

```typescript
import { useEffect } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { useWebSocket } from './useWebSocket';
import { useToast } from './useToast';

export const useTokenUpdates = () => {
  const { lastMessage } = useWebSocket();
  const queryClient = useQueryClient();
  const { toast } = useToast();
  
  useEffect(() => {
    if (!lastMessage) return;
    
    try {
      const message = JSON.parse(lastMessage.data);
      
      switch (message.type) {
        case 'token_update':
          // Invalidate token queries for fresh data
          queryClient.invalidateQueries({ queryKey: ['tokenSummary'] });
          queryClient.invalidateQueries({ queryKey: ['timeSeries'] });
          queryClient.invalidateQueries({ queryKey: ['providerStats'] });
          break;
          
        case 'quota_alert':
          toast({
            title: 'Quota Alert',
            description: message.data.message,
            variant: 'warning',
            action: {
              label: 'View',
              onClick: () => window.location.href = '/quotas',
            },
          });
          queryClient.invalidateQueries({ queryKey: ['quotas'] });
          break;
          
        case 'quota_exceeded':
          toast({
            title: 'Quota Exceeded',
            description: message.data.message,
            variant: 'destructive',
          });
          queryClient.invalidateQueries({ queryKey: ['quotas'] });
          break;
      }
    } catch (error) {
      console.error('Failed to parse WebSocket message:', error);
    }
  }, [lastMessage, queryClient, toast]);
};
```

---

## 4. File Structure

```
ui/src/
├── pages/
│   ├── Tokens.tsx           # Main analytics page
│   ├── Quotas.tsx           # Quota management
│   └── Pricing.tsx          # Pricing configuration
├── components/
│   ├── tokens/
│   │   ├── StatsCard.tsx           # Stat display card
│   │   ├── TokenTrendChart.tsx     # Line chart for trends
│   │   ├── ProviderPieChart.tsx    # Provider distribution
│   │   ├── ModelPieChart.tsx       # Model distribution
│   │   ├── TokenDataTable.tsx      # Data table with pagination
│   │   ├── TimeRangePicker.tsx     # Time range selector
│   │   └── QuotaProgress.tsx       # Quota progress bar
│   └── layout/
│       └── Layout.tsx       # (existing - add new nav items)
├── hooks/
│   ├── useTokenQueries.ts   # React Query hooks
│   └── useTokenUpdates.ts   # WebSocket integration
├── stores/
│   └── tokenStore.ts        # Zustand store (alternative)
├── lib/
│   ├── export.ts            # CSV/JSON export utilities
│   └── formatters.ts        # Number/date formatting
└── api/
    └── types.ts             # TypeScript interfaces
```

---

**Document Version**: 1.0
**Last Updated**: 2026-03-22