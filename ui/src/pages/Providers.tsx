import {
  Server,
  Plus,
  Power,
  PowerOff,
  Pencil,
  Wifi,
  Clock,
  AlertTriangle,
  CheckCircle2,
  XCircle,
  Ban,
  Eye,
  EyeOff,
  GripVertical,
  Loader2,
  X,
  Save,
} from "lucide-react";
import { useState, useCallback } from "react";
import {
  DndContext,
  closestCenter,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
  type DragEndEvent,
} from "@dnd-kit/core";
import {
  arrayMove,
  SortableContext,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import {
  LineChart,
  Line,
  ResponsiveContainer,
} from "recharts";
import { toast } from "sonner";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useAppStore } from "@/stores/store";
import type { Provider, ProviderStatus } from "@/api/types";
import { cn } from "@/lib/utils";

/**
 * Map provider status to badge variant and icon.
 */
function getStatusDisplay(status: ProviderStatus) {
  switch (status) {
    case "healthy":
      return { variant: "success" as const, icon: CheckCircle2, label: "Healthy" };
    case "cooldown":
      return { variant: "warning" as const, icon: Clock, label: "Cooldown" };
    case "unhealthy":
      return { variant: "danger" as const, icon: XCircle, label: "Unhealthy" };
    case "disabled":
      return { variant: "secondary" as const, icon: Ban, label: "Disabled" };
  }
}

/**
 * Edit modal for a provider.
 */
function EditProviderModal({
  provider,
  onClose,
}: {
  provider: Provider;
  onClose: () => void;
}) {
  const updateProvider = useAppStore((s) => s.updateProvider);
  const [form, setForm] = useState({
    base_url: provider.base_url,
    api_key: provider.api_key,
    weight: provider.weight,
    models: provider.models.join(", "),
  });

  function handleSave() {
    updateProvider(provider.name, {
      base_url: form.base_url,
      api_key: form.api_key,
      weight: form.weight,
      models: form.models.split(",").map((m) => m.trim()).filter(Boolean),
    });
    toast.success(`Provider "${provider.name}" updated`);
    onClose();
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm" onClick={onClose}>
      <div className="bg-card border rounded-lg shadow-lg w-full max-w-md p-6 space-y-4" onClick={(e) => e.stopPropagation()}>
        <div className="flex items-center justify-between">
          <h3 className="text-lg font-semibold">Edit {provider.name}</h3>
          <button onClick={onClose} className="text-muted-foreground hover:text-foreground">
            <X className="h-4 w-4" />
          </button>
        </div>
        <div className="space-y-3">
          <div className="space-y-1">
            <label className="text-sm font-medium">Base URL</label>
            <Input value={form.base_url} onChange={(e) => setForm({ ...form, base_url: e.target.value })} />
          </div>
          <div className="space-y-1">
            <label className="text-sm font-medium">API Key</label>
            <Input value={form.api_key} onChange={(e) => setForm({ ...form, api_key: e.target.value })} type="password" />
          </div>
          <div className="space-y-1">
            <label className="text-sm font-medium">Weight</label>
            <Input type="number" value={form.weight} onChange={(e) => setForm({ ...form, weight: Number(e.target.value) })} min={0} />
          </div>
          <div className="space-y-1">
            <label className="text-sm font-medium">Models (comma separated)</label>
            <Input value={form.models} onChange={(e) => setForm({ ...form, models: e.target.value })} placeholder="qwen-plus, qwen-turbo" />
          </div>
        </div>
        <div className="flex justify-end gap-2 pt-2">
          <Button variant="outline" size="sm" onClick={onClose}>Cancel</Button>
          <Button size="sm" onClick={handleSave} className="gap-1.5">
            <Save className="h-3.5 w-3.5" /> Save
          </Button>
        </div>
      </div>
    </div>
  );
}

/**
 * Sortable provider card using @dnd-kit.
 */
function SortableProviderCard({ provider }: { provider: Provider }) {
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: provider.name });

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
    zIndex: isDragging ? 50 : undefined,
    opacity: isDragging ? 0.8 : undefined,
  };

  return (
    <div ref={setNodeRef} style={style}>
      <ProviderCard
        provider={provider}
        dragHandleProps={{ ...attributes, ...listeners }}
        isDragging={isDragging}
      />
    </div>
  );
}

/**
 * Single provider card component with drag handle.
 */
function ProviderCard({
  provider,
  dragHandleProps,
  isDragging,
}: {
  provider: Provider;
  dragHandleProps?: Record<string, unknown>;
  isDragging?: boolean;
}) {
  const toggleProvider = useAppStore((s) => s.toggleProvider);
  const [showKey, setShowKey] = useState(false);
  const [testing, setTesting] = useState(false);
  const [editOpen, setEditOpen] = useState(false);

  const statusDisplay = getStatusDisplay(provider.status);
  const StatusIcon = statusDisplay.icon;
  const isDisabled = provider.status === "disabled";

  const sparklineData = provider.stats.latency_history.map((v, i) => ({
    idx: i,
    value: v,
  }));

  const maskedKey = provider.api_key.slice(0, 5) + "****" + provider.api_key.slice(-4);

  /**
   * Simulate a test connection with loading state and result toast.
   */
  const handleTestConnection = useCallback(() => {
    setTesting(true);
    const latency = 300 + Math.floor(Math.random() * 1200);
    setTimeout(() => {
      setTesting(false);
      const success = Math.random() > 0.15;
      if (success) {
        toast.success(`${provider.name}: Connection OK (${latency}ms)`);
      } else {
        toast.error(`${provider.name}: Connection failed - timeout`);
      }
    }, latency);
  }, [provider.name]);

  /**
   * Toggle enable/disable with toast feedback.
   */
  const handleToggle = useCallback(() => {
    toggleProvider(provider.name);
    if (provider.enabled) {
      toast.info(`Provider "${provider.name}" disabled`);
    } else {
      toast.success(`Provider "${provider.name}" enabled`);
    }
  }, [toggleProvider, provider.name, provider.enabled]);

  return (
    <>
      <Card className={cn(
        "transition-all duration-200",
        isDisabled && "opacity-60",
        isDragging && "shadow-xl ring-2 ring-primary/20",
      )}>
        <CardHeader className="flex flex-row items-start justify-between space-y-0 pb-3">
          <div className="flex items-start gap-2">
            <button
              className="mt-1 cursor-grab active:cursor-grabbing touch-none text-muted-foreground hover:text-foreground transition-colors"
              {...dragHandleProps}
              title="Drag to reorder"
            >
              <GripVertical className="h-5 w-5" />
            </button>
            <div className="space-y-1">
              <CardTitle className="flex items-center gap-2 text-base">
                <Server className="h-4 w-4" />
                {provider.name}
              </CardTitle>
              <div className="flex items-center gap-2 mt-1">
                <Badge variant={statusDisplay.variant}>
                  <StatusIcon className="h-3 w-3 mr-1" />
                  {statusDisplay.label}
                </Badge>
                <span className="text-xs text-muted-foreground">
                  P{provider.priority}
                </span>
                <span className="text-xs text-muted-foreground">
                  W{provider.weight}
                </span>
              </div>
            </div>
          </div>
        </CardHeader>

        <CardContent className="space-y-4">
          {/* Provider details */}
          <div className="space-y-2 text-sm">
            <div className="flex items-center justify-between">
              <span className="text-muted-foreground">Base URL</span>
              <span className="font-mono text-xs truncate max-w-[300px]">
                {provider.base_url}
              </span>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-muted-foreground">API Key</span>
              <div className="flex items-center gap-1">
                <span className="font-mono text-xs">
                  {showKey ? provider.api_key : maskedKey}
                </span>
                <button
                  onClick={() => setShowKey(!showKey)}
                  className="text-muted-foreground hover:text-foreground transition-colors"
                >
                  {showKey ? <EyeOff className="h-3.5 w-3.5" /> : <Eye className="h-3.5 w-3.5" />}
                </button>
              </div>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-muted-foreground">Models</span>
              <div className="flex gap-1 flex-wrap justify-end">
                {provider.models.map((m) => (
                  <Badge key={m} variant="outline" className="text-xs">
                    {m}
                  </Badge>
                ))}
              </div>
            </div>
          </div>

          {/* Stats row */}
          <div className="grid grid-cols-4 gap-3 pt-2 border-t">
            <div className="text-center">
              <div className="text-lg font-bold">
                {provider.stats.total_requests.toLocaleString()}
              </div>
              <div className="text-xs text-muted-foreground">Requests</div>
            </div>
            <div className="text-center">
              <div className="text-lg font-bold">
                {provider.stats.total_errors}
              </div>
              <div className="text-xs text-muted-foreground">Errors</div>
            </div>
            <div className="text-center">
              <div className="text-lg font-bold">
                {provider.stats.error_rate.toFixed(1)}%
              </div>
              <div className="text-xs text-muted-foreground">Error Rate</div>
            </div>
            <div className="text-center">
              <div className="text-lg font-bold">
                {provider.stats.avg_latency_ms}
              </div>
              <div className="text-xs text-muted-foreground">Avg ms</div>
            </div>
          </div>

          {/* Latency sparkline */}
          {sparklineData.length > 0 && (
            <div className="h-12">
              <ResponsiveContainer width="100%" height="100%">
                <LineChart data={sparklineData}>
                  <Line
                    type="monotone"
                    dataKey="value"
                    stroke={
                      provider.status === "healthy"
                        ? "hsl(160, 60%, 45%)"
                        : provider.status === "cooldown"
                          ? "hsl(43, 74%, 66%)"
                          : "hsl(0, 84%, 60%)"
                    }
                    strokeWidth={1.5}
                    dot={false}
                  />
                </LineChart>
              </ResponsiveContainer>
            </div>
          )}

          {/* Cooldown info */}
          {provider.status === "cooldown" && provider.cooldown_until && (
            <div className="flex items-center gap-2 p-2 rounded-md bg-amber-500/10 text-amber-600 dark:text-amber-400 text-xs animate-pulse">
              <AlertTriangle className="h-3.5 w-3.5 shrink-0" />
              <span>
                Quota exceeded. Recovers at{" "}
                {new Date(provider.cooldown_until).toLocaleTimeString()}
              </span>
            </div>
          )}

          {/* Actions */}
          <div className="flex items-center gap-2 pt-2 border-t">
            <Button
              variant={provider.enabled ? "outline" : "default"}
              size="sm"
              onClick={handleToggle}
              className="gap-1.5"
            >
              {provider.enabled ? (
                <>
                  <PowerOff className="h-3.5 w-3.5" /> Disable
                </>
              ) : (
                <>
                  <Power className="h-3.5 w-3.5" /> Enable
                </>
              )}
            </Button>
            <Button variant="outline" size="sm" className="gap-1.5" onClick={() => setEditOpen(true)}>
              <Pencil className="h-3.5 w-3.5" /> Edit
            </Button>
            <Button
              variant="outline"
              size="sm"
              className="gap-1.5"
              onClick={handleTestConnection}
              disabled={testing || isDisabled}
            >
              {testing ? (
                <>
                  <Loader2 className="h-3.5 w-3.5 animate-spin" /> Testing...
                </>
              ) : (
                <>
                  <Wifi className="h-3.5 w-3.5" /> Test
                </>
              )}
            </Button>
          </div>
        </CardContent>
      </Card>

      {editOpen && (
        <EditProviderModal provider={provider} onClose={() => setEditOpen(false)} />
      )}
    </>
  );
}

/**
 * Providers management page with drag-and-drop reordering.
 */
export default function Providers() {
  const providers = useAppStore((s) => s.providers);
  const reorderProviders = useAppStore((s) => s.reorderProviders);

  const sorted = [...providers].sort((a, b) => a.priority - b.priority);

  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 8 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  );

  /**
   * Handle drag end to reorder providers.
   */
  function handleDragEnd(event: DragEndEvent) {
    const { active, over } = event;
    if (!over || active.id === over.id) return;
    const oldIndex = sorted.findIndex((p) => p.name === active.id);
    const newIndex = sorted.findIndex((p) => p.name === over.id);
    const reordered = arrayMove(sorted, oldIndex, newIndex);
    reorderProviders(reordered.map((p) => p.name));
    toast.info("Provider priority updated");
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold tracking-tight">Providers</h1>
        <Button className="gap-2">
          <Plus className="h-4 w-4" /> Add Provider
        </Button>
      </div>

      <p className="text-sm text-muted-foreground">
        Drag providers to reorder priority. Higher position = higher priority.
      </p>

      <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={handleDragEnd}>
        <SortableContext items={sorted.map((p) => p.name)} strategy={verticalListSortingStrategy}>
          <div className="grid gap-4 lg:grid-cols-2">
            {sorted.map((provider) => (
              <SortableProviderCard key={provider.name} provider={provider} />
            ))}
          </div>
        </SortableContext>
      </DndContext>
    </div>
  );
}
