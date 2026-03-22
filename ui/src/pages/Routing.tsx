import { useState } from "react";
import {
  ArrowRightLeft,
  RefreshCw,
  Shuffle,
  Gauge,
  Pin,
  Trash2,
  Plus,
  GripVertical,
  Save,
  ToggleLeft,
  ToggleRight,
  AlertTriangle,
} from "lucide-react";
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
import { toast } from "sonner";
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
import { useAppStore } from "@/stores/store";
import type { RoutingStrategy, Provider } from "@/api/types";
import { cn } from "@/lib/utils";

/**
 * Strategy option definition for the selector cards.
 */
interface StrategyOption {
  id: RoutingStrategy;
  label: string;
  description: string;
  icon: React.ElementType;
}

const strategyOptions: StrategyOption[] = [
  {
    id: "failover",
    label: "Failover",
    description: "Use providers in priority order, auto switch on failure",
    icon: ArrowRightLeft,
  },
  {
    id: "round-robin",
    label: "Round Robin",
    description: "Distribute requests evenly across all healthy providers",
    icon: RefreshCw,
  },
  {
    id: "weighted-random",
    label: "Weighted Random",
    description: "Random selection weighted by weight values",
    icon: Shuffle,
  },
  {
    id: "quota-aware",
    label: "Quota-Aware",
    description: "Track token usage and switch at threshold",
    icon: Gauge,
  },
  {
    id: "manual",
    label: "Manual / Fixed",
    description: "Always use a specific pinned provider",
    icon: Pin,
  },
];

/**
 * Failover strategy settings.
 */
function FailoverSettings() {
  const routingConfig = useAppStore((s) => s.routingConfig);
  const setRoutingConfig = useAppStore((s) => s.setRoutingConfig);
  const [localConfig, setLocalConfig] = useState({
    max_retries: routingConfig.max_retries,
    cooldown_secs: routingConfig.cooldown_secs,
    failure_threshold: routingConfig.failure_threshold,
    recovery_secs: routingConfig.recovery_secs,
  });

  return (
    <div className="space-y-4">
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <div className="space-y-2">
          <label className="text-sm font-medium">Max retries per request</label>
          <Input
            type="number"
            value={localConfig.max_retries}
            onChange={(e) => setLocalConfig({ ...localConfig, max_retries: Number(e.target.value) })}
            min={0}
            max={10}
          />
        </div>
        <div className="space-y-2">
          <label className="text-sm font-medium">Cooldown after quota error (s)</label>
          <Input
            type="number"
            value={localConfig.cooldown_secs}
            onChange={(e) => setLocalConfig({ ...localConfig, cooldown_secs: Number(e.target.value) })}
            min={0}
          />
        </div>
        <div className="space-y-2">
          <label className="text-sm font-medium">Failure threshold</label>
          <Input
            type="number"
            value={localConfig.failure_threshold}
            onChange={(e) => setLocalConfig({ ...localConfig, failure_threshold: Number(e.target.value) })}
            min={1}
            max={20}
          />
        </div>
        <div className="space-y-2">
          <label className="text-sm font-medium">Recovery check interval (s)</label>
          <Input
            type="number"
            value={localConfig.recovery_secs}
            onChange={(e) => setLocalConfig({ ...localConfig, recovery_secs: Number(e.target.value) })}
            min={0}
          />
        </div>
      </div>
      <Button
        onClick={() => { setRoutingConfig(localConfig); toast.success("Failover settings applied"); }}
        className="gap-2"
      >
        <Save className="h-4 w-4" /> Apply Changes
      </Button>
    </div>
  );
}

/**
 * Round Robin strategy settings - toggle providers on/off in rotation.
 */
function RoundRobinSettings() {
  const providers = useAppStore((s) => s.providers);
  const toggleProvider = useAppStore((s) => s.toggleProvider);
  const enabledProviders = providers.filter((p) => p.enabled);

  return (
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground">
        Toggle providers to include in the round-robin rotation. Only enabled providers receive traffic.
      </p>
      <div className="space-y-2">
        {providers.map((p) => (
          <div key={p.name} className="flex items-center justify-between p-3 rounded-md border">
            <div className="flex items-center gap-3">
              <Badge variant={p.status === "healthy" ? "success" : p.status === "cooldown" ? "warning" : "secondary"}>
                {p.status}
              </Badge>
              <span className="font-medium">{p.name}</span>
            </div>
            <button
              onClick={() => toggleProvider(p.name)}
              className="flex items-center gap-2 text-sm"
            >
              {p.enabled ? (
                <>
                  <ToggleRight className="h-5 w-5 text-emerald-500" />
                  <span className="text-muted-foreground">In rotation</span>
                </>
              ) : (
                <>
                  <ToggleLeft className="h-5 w-5 text-muted-foreground" />
                  <span className="text-muted-foreground">Excluded</span>
                </>
              )}
            </button>
          </div>
        ))}
      </div>
      <p className="text-xs text-muted-foreground">
        {enabledProviders.length} of {providers.length} providers in rotation.
      </p>
    </div>
  );
}

/**
 * Weighted Random strategy settings - weight sliders per provider.
 */
function WeightedRandomSettings() {
  const providers = useAppStore((s) => s.providers);
  const updateProviderWeight = useAppStore((s) => s.updateProviderWeight);
  const [localWeights, setLocalWeights] = useState<Record<string, number>>(() => {
    const weights: Record<string, number> = {};
    providers.forEach((p) => { weights[p.name] = p.weight; });
    return weights;
  });

  const totalWeight = Object.values(localWeights).reduce((a, b) => a + b, 0);

  function handleApply() {
    Object.entries(localWeights).forEach(([name, weight]) => {
      updateProviderWeight(name, weight);
    });
    toast.success("Weights updated");
  }

  return (
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground">
        Adjust weight values to control traffic distribution. Higher weight = more traffic.
      </p>
      <div className="space-y-3">
        {providers.map((p) => {
          const weight = localWeights[p.name] ?? p.weight;
          const percentage = totalWeight > 0 ? Math.round((weight / totalWeight) * 100) : 0;
          return (
            <div key={p.name} className="space-y-2">
              <div className="flex items-center justify-between">
                <span className="font-medium text-sm">{p.name}</span>
                <div className="flex items-center gap-2">
                  <span className="text-xs text-muted-foreground w-12 text-right">{percentage}%</span>
                  <Input
                    type="number"
                    value={weight}
                    onChange={(e) => setLocalWeights({ ...localWeights, [p.name]: Number(e.target.value) })}
                    min={0}
                    max={100}
                    className="w-20 h-8 text-sm"
                  />
                </div>
              </div>
              <input
                type="range"
                min={0}
                max={100}
                value={weight}
                onChange={(e) => setLocalWeights({ ...localWeights, [p.name]: Number(e.target.value) })}
                className="w-full h-2 bg-muted rounded-lg appearance-none cursor-pointer"
              />
            </div>
          );
        })}
      </div>
      <Button onClick={handleApply} className="gap-2">
        <Save className="h-4 w-4" /> Apply Weights
      </Button>
    </div>
  );
}

/**
 * Quota-Aware strategy settings - token budget per provider.
 */
function QuotaAwareSettings() {
  const providers = useAppStore((s) => s.providers);
  const updateProvider = useAppStore((s) => s.updateProvider);
  const [budgets, setBudgets] = useState<Record<string, number>>(() => {
    const b: Record<string, number> = {};
    providers.forEach((p) => { b[p.name] = p.token_budget ?? 100000; });
    return b;
  });
  const [threshold, setThreshold] = useState(90);

  function handleApply() {
    Object.entries(budgets).forEach(([name, budget]) => {
      updateProvider(name, { token_budget: budget });
    });
    toast.success("Token budgets updated");
  }

  return (
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground">
        Set token budgets per provider. The proxy will switch providers when usage reaches the threshold.
      </p>
      <div className="space-y-3">
        {providers.map((p) => (
          <div key={p.name} className="flex items-center justify-between gap-4 p-3 rounded-md border">
            <div className="flex items-center gap-3">
              <Badge variant={p.status === "healthy" ? "success" : "warning"}>
                {p.status}
              </Badge>
              <span className="font-medium">{p.name}</span>
            </div>
            <div className="flex items-center gap-2">
              <span className="text-xs text-muted-foreground">Budget:</span>
              <Input
                type="number"
                value={budgets[p.name] ?? 100000}
                onChange={(e) => setBudgets({ ...budgets, [p.name]: Number(e.target.value) })}
                min={0}
                className="w-28 h-8 text-sm"
              />
              <span className="text-xs text-muted-foreground">tokens</span>
            </div>
          </div>
        ))}
      </div>
      <div className="flex items-center gap-4">
        <label className="text-sm font-medium">Switch threshold:</label>
        <Input
          type="number"
          value={threshold}
          onChange={(e) => setThreshold(Number(e.target.value))}
          min={50}
          max={100}
          className="w-20 h-8"
        />
        <span className="text-sm text-muted-foreground">% of budget</span>
      </div>
      <Button onClick={handleApply} className="gap-2">
        <Save className="h-4 w-4" /> Apply Budgets
      </Button>
    </div>
  );
}

/**
 * Manual / Fixed strategy settings - pin to a single provider.
 */
function ManualSettings() {
  const providers = useAppStore((s) => s.providers);
  const routingConfig = useAppStore((s) => s.routingConfig);
  const setPinnedProvider = useAppStore((s) => s.setPinnedProvider);
  const [selected, setSelected] = useState(routingConfig.pinned_provider ?? providers[0]?.name ?? "");

  function handlePin() {
    setPinnedProvider(selected);
    toast.success(`Pinned to ${selected}`);
  }

  const selectedProvider = providers.find((p) => p.name === selected);
  const isUnhealthy = selectedProvider && selectedProvider.status !== "healthy";

  return (
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground">
        Pin all traffic to a specific provider. Useful for testing or when you want predictable routing.
      </p>
      <div className="flex items-end gap-4">
        <div className="space-y-2 flex-1 max-w-xs">
          <label className="text-sm font-medium">Pinned Provider</label>
          <Select value={selected} onValueChange={setSelected}>
            <SelectTrigger className="h-9">
              <SelectValue placeholder="Select provider..." />
            </SelectTrigger>
            <SelectContent>
              {providers.map((p) => (
                <SelectItem key={p.name} value={p.name}>
                  {p.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <Button onClick={handlePin} className="gap-2">
          <Pin className="h-4 w-4" /> Pin Provider
        </Button>
      </div>
      {isUnhealthy && (
        <div className="flex items-center gap-2 p-3 rounded-md bg-amber-500/10 text-amber-600 dark:text-amber-400 text-sm">
          <AlertTriangle className="h-4 w-4" />
          Warning: Selected provider is currently {selectedProvider?.status}.
        </div>
      )}
      {routingConfig.pinned_provider && (
        <p className="text-xs text-muted-foreground">
          Currently pinned to: <strong>{routingConfig.pinned_provider}</strong>
        </p>
      )}
    </div>
  );
}

/**
 * Dynamic strategy settings component.
 */
function StrategySettings({ strategy }: { strategy: RoutingStrategy }) {
  switch (strategy) {
    case "failover":
      return <FailoverSettings />;
    case "round-robin":
      return <RoundRobinSettings />;
    case "weighted-random":
      return <WeightedRandomSettings />;
    case "quota-aware":
      return <QuotaAwareSettings />;
    case "manual":
      return <ManualSettings />;
    default:
      return null;
  }
}

/**
 * Sortable row for the provider priority table.
 */
function SortablePriorityRow({ provider: p }: { provider: Provider }) {
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: p.name });

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
    zIndex: isDragging ? 50 : undefined,
  };

  const status = p.status === "healthy"
    ? { variant: "success" as const, label: "Healthy" }
    : p.status === "cooldown"
      ? { variant: "warning" as const, label: "Cooldown" }
      : { variant: "danger" as const, label: "Unhealthy" };

  return (
    <div
      ref={setNodeRef}
      style={style}
      className={cn(
        "grid grid-cols-[auto_1fr_100px_100px_120px] gap-4 px-4 py-3 border-b last:border-0 items-center text-sm transition-colors",
        isDragging ? "bg-accent/50 shadow-md" : "hover:bg-accent/30",
      )}
    >
      <button
        className="cursor-grab active:cursor-grabbing touch-none text-muted-foreground hover:text-foreground"
        {...attributes}
        {...listeners}
      >
        <GripVertical className="h-4 w-4" />
      </button>
      <div>
        <span className="font-medium">{p.name}</span>
        <span className="text-xs text-muted-foreground ml-2">
          {p.models.slice(0, 2).join(", ")}
          {p.models.length > 2 && ` +${p.models.length - 2}`}
        </span>
      </div>
      <div className="text-center font-mono">{p.priority}</div>
      <div className="text-center font-mono">{p.weight}</div>
      <div className="text-center">
        <Badge variant={status.variant}>{status.label}</Badge>
      </div>
    </div>
  );
}

/**
 * Routing strategy and rules management page.
 */
export default function Routing() {
  const routingConfig = useAppStore((s) => s.routingConfig);
  const providers = useAppStore((s) => s.providers);
  const modelRules = useAppStore((s) => s.modelRules);
  const setStrategy = useAppStore((s) => s.setStrategy);
  const removeModelRule = useAppStore((s) => s.removeModelRule);
  const addModelRule = useAppStore((s) => s.addModelRule);
  const reorderProviders = useAppStore((s) => s.reorderProviders);

  const [newRulePattern, setNewRulePattern] = useState("");
  const [newRuleTarget, setNewRuleTarget] = useState("");

  const enabledProviders = providers.filter((p) => p.enabled);
  const sortedEnabled = [...enabledProviders].sort((a, b) => a.priority - b.priority);

  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 5 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  );

  /**
   * Handle adding a new model routing rule.
   */
  function handleAddRule() {
    if (!newRulePattern || !newRuleTarget) return;
    addModelRule({
      id: `rule-${Date.now()}`,
      pattern: newRulePattern,
      target_provider: newRuleTarget,
    });
    setNewRulePattern("");
    setNewRuleTarget("");
    toast.success("Model routing rule added");
  }

  /**
   * Handle drag end to reorder provider priority in the table.
   */
  function handlePriorityDragEnd(event: DragEndEvent) {
    const { active, over } = event;
    if (!over || active.id === over.id) return;
    const oldIndex = sortedEnabled.findIndex((p) => p.name === active.id);
    const newIndex = sortedEnabled.findIndex((p) => p.name === over.id);
    const reordered = arrayMove(sortedEnabled, oldIndex, newIndex);
    // Preserve disabled providers in their original order
    const disabledProviders = providers.filter((p) => !p.enabled);
    reorderProviders([...reordered.map((p) => p.name), ...disabledProviders.map((p) => p.name)]);
    toast.info("Provider priority updated");
  }

  // Get strategy label for settings card title
  const strategyLabel = strategyOptions.find((o) => o.id === routingConfig.strategy)?.label ?? "Strategy";

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold tracking-tight">Routing</h1>
      </div>

      {/* Strategy Selector */}
      <Card>
        <CardHeader>
          <CardTitle>Select Strategy</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-5">
            {strategyOptions.map((option) => {
              const isActive = routingConfig.strategy === option.id;
              const Icon = option.icon;
              return (
                <button
                  key={option.id}
                  onClick={() => { setStrategy(option.id); toast.success(`Strategy changed to ${option.label}`); }}
                  className={cn(
                    "relative flex flex-col items-start gap-2 rounded-lg border p-4 text-left transition-all hover:border-foreground/20",
                    isActive
                      ? "border-primary bg-primary/5 shadow-sm"
                      : "border-border hover:bg-accent/50"
                  )}
                >
                  <div className="flex items-center gap-2">
                    <Icon
                      className={cn(
                        "h-5 w-5",
                        isActive ? "text-primary" : "text-muted-foreground"
                      )}
                    />
                    <span
                      className={cn(
                        "font-semibold text-sm",
                        isActive ? "text-primary" : "text-foreground"
                      )}
                    >
                      {option.label}
                    </span>
                  </div>
                  <p className="text-xs text-muted-foreground leading-relaxed">
                    {option.description}
                  </p>
                  {isActive && (
                    <Badge variant="default" className="absolute top-2 right-2 text-[10px] px-1.5 py-0">
                      Active
                    </Badge>
                  )}
                </button>
              );
            })}
          </div>
        </CardContent>
      </Card>

      {/* Strategy Settings - Dynamic based on selected strategy */}
      <Card>
        <CardHeader>
          <CardTitle>{strategyLabel} Settings</CardTitle>
        </CardHeader>
        <CardContent>
          <StrategySettings strategy={routingConfig.strategy} />
        </CardContent>
      </Card>

      {/* Provider Priority / Weight Table with Drag-and-Drop */}
      <Card>
        <CardHeader>
          <CardTitle>Provider Priority & Weight</CardTitle>
        </CardHeader>
        <CardContent>
          <p className="text-sm text-muted-foreground mb-3">
            Drag rows to reorder priority. Top = highest priority.
          </p>
          <div className="rounded-md border">
            <div className="grid grid-cols-[auto_1fr_100px_100px_120px] gap-4 px-4 py-2 border-b bg-muted/50 text-sm font-medium text-muted-foreground">
              <span className="w-6" />
              <span>Provider</span>
              <span className="text-center">Priority</span>
              <span className="text-center">Weight</span>
              <span className="text-center">Status</span>
            </div>
            <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={handlePriorityDragEnd}>
              <SortableContext items={sortedEnabled.map((p) => p.name)} strategy={verticalListSortingStrategy}>
                {sortedEnabled.map((p) => (
                  <SortablePriorityRow key={p.name} provider={p} />
                ))}
              </SortableContext>
            </DndContext>
          </div>
        </CardContent>
      </Card>

      {/* Model Routing Rules */}
      <Card>
        <CardHeader>
          <CardTitle>Model Routing Rules</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <p className="text-sm text-muted-foreground">
            Override global strategy for specific models. Patterns use glob format (e.g., <code className="text-xs bg-muted px-1 py-0.5 rounded">qwen-*</code>).
          </p>

          <div className="space-y-2">
            {modelRules.map((rule) => (
              <div
                key={rule.id}
                className="flex items-center gap-3 rounded-md border px-4 py-2"
              >
                <span className="font-mono text-sm flex-1">
                  model = "{rule.pattern}"
                </span>
                <span className="text-muted-foreground text-sm">→</span>
                <Badge variant="outline">{rule.target_provider}</Badge>
                <Button
                  variant="ghost"
                  size="icon"
                  className="h-8 w-8 text-destructive hover:text-destructive"
                  onClick={() => removeModelRule(rule.id)}
                >
                  <Trash2 className="h-3.5 w-3.5" />
                </Button>
              </div>
            ))}

            <div className="text-xs text-muted-foreground italic">
              Default: follow global strategy
            </div>
          </div>

          <div className="flex items-end gap-3 pt-2 border-t">
            <div className="space-y-1 flex-1">
              <label className="text-xs font-medium">Pattern</label>
              <Input
                placeholder="e.g. qwen-*"
                value={newRulePattern}
                onChange={(e) => setNewRulePattern(e.target.value)}
                className="h-9"
              />
            </div>
            <div className="space-y-1 flex-1">
              <label className="text-xs font-medium">Target Provider</label>
              <Select value={newRuleTarget} onValueChange={setNewRuleTarget}>
                <SelectTrigger className="h-9">
                  <SelectValue placeholder="Select provider..." />
                </SelectTrigger>
                <SelectContent>
                  {enabledProviders.map((p) => (
                    <SelectItem key={p.name} value={p.name}>
                      {p.name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <Button
              onClick={handleAddRule}
              size="sm"
              className="gap-1.5 h-9"
              disabled={!newRulePattern || !newRuleTarget}
            >
              <Plus className="h-3.5 w-3.5" /> Add Rule
            </Button>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
