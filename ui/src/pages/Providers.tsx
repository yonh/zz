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
  GripVertical,
  Loader2,
  X,
  Save,
} from "lucide-react";
import { useState, useCallback, useEffect } from "react";
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
import type { Provider, ProviderInput, ProviderStatus } from "@/api/types";
import { api } from "@/api/client";
import { cn } from "@/lib/utils";

/**
 * Live cooldown countdown timer.
 */
function CooldownCountdown({ until }: { until: string }) {
  const [remaining, setRemaining] = useState("");

  useEffect(() => {
    function update() {
      const diff = new Date(until).getTime() - Date.now();
      if (diff <= 0) {
        setRemaining("Recovering...");
        return;
      }
      const secs = Math.floor(diff / 1000);
      const mins = Math.floor(secs / 60);
      setRemaining(mins > 0 ? `${mins}m ${secs % 60}s` : `${secs}s`);
    }
    update();
    const timer = setInterval(update, 1000);
    return () => clearInterval(timer);
  }, [until]);

  return <span>Quota exceeded. Recovers in {remaining}</span>;
}

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
 * Add modal for a new provider.
 */
function AddProviderModal({ onClose }: { onClose: () => void }) {
  const providers = useAppStore((s) => s.providers);
  const addProvider = useAppStore((s) => s.addProvider);
  const [form, setForm] = useState({
    name: "",
    base_url: "",
    api_key: "",
    priority: providers.length + 1,
    weight: 50,
    models: "",
  });
  const [error, setError] = useState("");

  async function handleSave() {
    if (!form.name.trim()) {
      setError("Name is required");
      return;
    }
    if (providers.some((p) => p.name === form.name.trim())) {
      setError("Provider name already exists");
      return;
    }
    if (!form.base_url.trim()) {
      setError("Base URL is required");
      return;
    }
    try {
      const created = await api.addProvider({
        name: form.name.trim(),
        base_url: form.base_url.trim(),
        api_key: form.api_key,
        priority: form.priority,
        weight: form.weight,
        models: form.models.split(",").map((m) => m.trim()).filter(Boolean),
      });
      addProvider(created);
      toast.success(`Provider "${form.name.trim()}" added`);
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to add provider");
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm" onClick={onClose}>
      <div className="bg-card border rounded-lg shadow-lg w-full max-w-md p-6 space-y-4" onClick={(e) => e.stopPropagation()}>
        <div className="flex items-center justify-between">
          <h3 className="text-lg font-semibold">Add Provider</h3>
          <button onClick={onClose} className="text-muted-foreground hover:text-foreground">
            <X className="h-4 w-4" />
          </button>
        </div>
        {error && (
          <div className="text-sm text-destructive bg-destructive/10 p-2 rounded">
            {error}
          </div>
        )}
        <div className="space-y-3">
          <div className="space-y-1">
            <label className="text-sm font-medium">Name *</label>
            <Input
              value={form.name}
              onChange={(e) => { setForm({ ...form, name: e.target.value }); setError(""); }}
              placeholder="my-provider"
            />
          </div>
          <div className="space-y-1">
            <label className="text-sm font-medium">Base URL *</label>
            <Input
              value={form.base_url}
              onChange={(e) => setForm({ ...form, base_url: e.target.value })}
              placeholder="https://api.example.com/v1"
            />
          </div>
          <div className="space-y-1">
            <label className="text-sm font-medium">API Key</label>
            <Input
              value={form.api_key}
              onChange={(e) => setForm({ ...form, api_key: e.target.value })}
              type="password"
              placeholder="sk-..."
            />
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div className="space-y-1">
              <label className="text-sm font-medium">Priority</label>
              <Input
                type="number"
                value={form.priority}
                onChange={(e) => setForm({ ...form, priority: Number(e.target.value) })}
                min={1}
              />
            </div>
            <div className="space-y-1">
              <label className="text-sm font-medium">Weight</label>
              <Input
                type="number"
                value={form.weight}
                onChange={(e) => setForm({ ...form, weight: Number(e.target.value) })}
                min={0}
              />
            </div>
          </div>
          <div className="space-y-1">
            <label className="text-sm font-medium">Models (comma separated)</label>
            <Input
              value={form.models}
              onChange={(e) => setForm({ ...form, models: e.target.value })}
              placeholder="gpt-4, gpt-3.5-turbo"
            />
          </div>
        </div>
        <div className="flex justify-end gap-2 pt-2">
          <Button variant="outline" size="sm" onClick={onClose}>Cancel</Button>
          <Button size="sm" onClick={handleSave} className="gap-1.5">
            <Plus className="h-3.5 w-3.5" /> Add
          </Button>
        </div>
      </div>
    </div>
  );
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
    api_key: "", // Leave empty to keep existing key
    priority: provider.priority,
    weight: provider.weight,
    models: provider.models.join(", "),
  });
  const [error, setError] = useState("");

  async function handleSave() {
    try {
      const updated = await api.updateProvider(provider.name, {
        base_url: form.base_url,
        api_key: form.api_key || undefined,
        priority: form.priority,
        weight: form.weight,
        models: form.models.split(",").map((m) => m.trim()).filter(Boolean),
      });
      updateProvider(provider.name, updated);
      toast.success(`Provider "${provider.name}" updated`);
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to update provider");
    }
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
        {error && (
          <div className="text-sm text-destructive bg-destructive/10 p-2 rounded">
            {error}
          </div>
        )}
        <div className="space-y-3">
          <div className="space-y-1">
            <label className="text-sm font-medium">Base URL</label>
            <Input value={form.base_url} onChange={(e) => setForm({ ...form, base_url: e.target.value })} />
          </div>
          <div className="space-y-1">
            <label className="text-sm font-medium">API Key</label>
            <Input
              value={form.api_key}
              onChange={(e) => setForm({ ...form, api_key: e.target.value })}
              type="password"
              placeholder={`Current: ${provider.api_key_masked}`}
            />
            <p className="text-xs text-muted-foreground">Leave empty to keep existing key</p>
          </div>
          <div className="space-y-1">
            <label className="text-sm font-medium">Priority</label>
            <Input type="number" value={form.priority} onChange={(e) => setForm({ ...form, priority: Number(e.target.value) })} min={1} />
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
  const [testing, setTesting] = useState(false);
  const [editOpen, setEditOpen] = useState(false);

  const statusDisplay = getStatusDisplay(provider.status);
  const StatusIcon = statusDisplay.icon;
  const isDisabled = provider.status === "disabled";

  const sparklineData = provider.stats.latency_history.map((v, i) => ({
    idx: i,
    value: v,
  }));

  /**
   * Test connection to provider via backend API.
   */
  const handleTestConnection = useCallback(async () => {
    setTesting(true);
    try {
      const result = await api.testProvider(provider.name);
      if (result.success) {
        toast.success(`${provider.name}: Connection OK (${result.latency_ms}ms)`);
      } else {
        toast.error(`${provider.name}: Connection failed`);
      }
    } catch (err) {
      toast.error(`${provider.name}: Test failed - ${err instanceof Error ? err.message : "unknown error"}`);
    } finally {
      setTesting(false);
    }
  }, [provider.name]);

  /**
   * Toggle enable/disable with toast feedback.
   */
  const handleToggle = useCallback(async () => {
    const willEnable = !provider.enabled;
    try {
      if (willEnable) {
        await api.enableProvider(provider.name);
      } else {
        await api.disableProvider(provider.name);
      }
      toggleProvider(provider.name);
      toast.success(`${provider.name} ${willEnable ? "enabled" : "disabled"}`);
    } catch (err) {
      toast.error(`Failed to ${willEnable ? "enable" : "disable"} provider: ${err instanceof Error ? err.message : "unknown error"}`);
    }
  }, [provider.name, provider.enabled, toggleProvider]);

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
              <span className="font-mono text-xs">
                {provider.api_key_masked}
              </span>
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
              <CooldownCountdown until={provider.cooldown_until} />
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
  const [addModalOpen, setAddModalOpen] = useState(false);

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
        <Button className="gap-2" onClick={() => setAddModalOpen(true)}>
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

      {addModalOpen && <AddProviderModal onClose={() => setAddModalOpen(false)} />}
    </div>
  );
}
