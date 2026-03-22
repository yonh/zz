# UI Design Specification

## Overview

This document defines the design system and UI specifications for the zz proxy dashboard, based on the Routing and Providers page patterns.

## Design Principles

1. **Consistency** - Unified visual language across all pages
2. **Simplicity** - Clean, minimal design without excessive decoration
3. **Semantic Colors** - Use CSS variables, avoid hardcoded colors
4. **Clear Hierarchy** - Proper spacing and typography scales

---

## Layout System

### Page Container

```tsx
<div className="space-y-6">
  {/* Page Header */}
  <div className="flex items-center justify-between">
    <h1 className="text-2xl font-bold tracking-tight">Page Title</h1>
    {/* Optional action buttons */}
  </div>

  {/* Content Cards */}
  <Card>...</Card>
</div>
```

### Grid Layouts

```tsx
// Stats cards - 4 columns on large screens
<div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">

// Two column layout
<div className="grid gap-4 lg:grid-cols-2">

// Strategy selector - 5 columns
<div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-5">

// Charts row - 4:3 ratio
<div className="grid gap-4 lg:grid-cols-7">
  <Card className="lg:col-span-4">...</Card>
  <Card className="lg:col-span-3">...</Card>
</div>
```

---

## Card Component

### Standard Card

```tsx
<Card>
  <CardHeader>
    <CardTitle>Section Title</CardTitle>
  </CardHeader>
  <CardContent>
    {/* Content */}
  </CardContent>
</Card>
```

### Card with Icon Title

```tsx
<Card>
  <CardHeader>
    <CardTitle className="flex items-center gap-2">
      <Icon className="h-4 w-4" />
      Title Text
    </CardTitle>
  </CardHeader>
  <CardContent>
    {/* Content */}
  </CardContent>
</Card>
```

### Stats Card (Recommended Style)

```tsx
// Simple, clean stats card without border decoration
<Card>
  <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
    <CardTitle className="text-sm font-medium">Metric Name</CardTitle>
    <Icon className="h-4 w-4 text-muted-foreground" />
  </CardHeader>
  <CardContent>
    <div className="text-2xl font-bold">{value}</div>
    <p className="text-xs text-muted-foreground">{description}</p>
  </CardContent>
</Card>
```

**Note:** Avoid `border-l-4` decoration. Use simple, clean cards instead.

---

## Typography

### Headings

| Element | Classes |
|---------|---------|
| Page Title | `text-2xl font-bold tracking-tight` |
| Card Title | `text-sm font-semibold` |
| Section Label | `text-sm font-medium` |
| Small Label | `text-xs font-medium` |

### Body Text

| Element | Classes |
|---------|---------|
| Description | `text-sm text-muted-foreground` |
| Hint Text | `text-xs text-muted-foreground` |
| Mono/Data | `font-mono text-xs` |
| Mono Value | `font-mono text-sm` |

---

## Spacing Scale

| Level | Value | Usage |
|-------|-------|-------|
| xs | `gap-1.5` | Icon + text inline |
| sm | `gap-2` | Button internal, small groups |
| md | `gap-3` | Form fields, list items |
| lg | `gap-4` | Card grids, section groups |
| xl | `space-y-6` | Page sections |

---

## Color System

### CSS Variables (Preferred)

```css
--color-foreground
--color-muted-foreground
--color-card
--color-border
--color-primary
--color-accent
--color-destructive
```

### Semantic Status Colors (via Badge)

| Status | Badge Variant | Use Case |
|--------|---------------|----------|
| Success | `variant="success"` | Healthy, OK, Active |
| Warning | `variant="warning"` | Cooldown, Rate limited |
| Danger | `variant="danger"` | Error, Unhealthy |
| Default | `variant="secondary"` | Disabled, Inactive |
| Outline | `variant="outline"` | Tags, Labels |

### Avoid

- Hardcoded Tailwind colors like `text-emerald-600`, `text-blue-600`
- Custom color values outside CSS variables

---

## Table Design

### Standard Table

```tsx
<div className="rounded-md border">
  {/* Header */}
  <div className="grid grid-cols-[...] gap-4 px-4 py-2 border-b bg-muted/50 text-sm font-medium text-muted-foreground">
    <span>Column 1</span>
    <span>Column 2</span>
    ...
  </div>

  {/* Rows */}
  {items.map((item) => (
    <div
      key={item.id}
      className="grid grid-cols-[...] gap-4 px-4 py-3 border-b last:border-0 text-sm hover:bg-accent/30 transition-colors"
    >
      ...
    </div>
  ))}
</div>
```

### Grid Column Templates

```tsx
// Standard data table
grid-cols-[auto_1fr_100px_100px_120px]

// Log table
grid-cols-[24px_90px_56px_120px_100px_1fr_70px]
```

---

## Interactive Elements

### Buttons

```tsx
// Primary action
<Button className="gap-2">
  <Icon className="h-4 w-4" /> Label
</Button>

// Secondary action
<Button variant="outline" size="sm" className="gap-1.5">
  <Icon className="h-3.5 w-3.5" /> Label
</Button>

// Destructive action
<Button variant="ghost" className="text-destructive hover:text-destructive">
  <Icon className="h-3.5 w-3.5" />
</Button>
```

### Selectable Cards

```tsx
<button
  className={cn(
    "flex flex-col items-start gap-2 rounded-lg border p-4 text-left transition-all",
    isActive
      ? "border-primary bg-primary/5 shadow-sm"
      : "border-border hover:bg-accent/50"
  )}
>
  ...
</button>
```

---

## Charts

### Line Chart

```tsx
<ResponsiveContainer width="100%" height={250}>
  <LineChart data={data}>
    <CartesianGrid
      strokeDasharray="3 3"
      className="stroke-border"
      strokeOpacity={0.3}
    />
    <XAxis
      dataKey="time"
      tick={{ fill: "hsl(var(--color-muted-foreground))", fontSize: 11 }}
      tickLine={false}
    />
    <YAxis
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
    />
    <Line
      type="monotone"
      dataKey="value"
      stroke="hsl(var(--color-primary))"
      strokeWidth={2}
      dot={false}
    />
  </LineChart>
</ResponsiveContainer>
```

### Chart Colors

Use CSS variables:
```tsx
const CHART_COLORS = [
  "hsl(var(--color-chart-1))",
  "hsl(var(--color-chart-2))",
  "hsl(var(--color-chart-3))",
  "hsl(var(--color-chart-4))",
  "hsl(var(--color-chart-5))",
];
```

---

## Badge Component

```tsx
// Status badges
<Badge variant="success">Healthy</Badge>
<Badge variant="warning">Cooldown</Badge>
<Badge variant="danger">Error</Badge>

// Compact status
<Badge variant="success" className="text-[10px] justify-center">
  200
</Badge>

// Tag style
<Badge variant="outline" className="text-xs">
  model-name
</Badge>
```

---

## Form Elements

### Input Field

```tsx
<div className="space-y-2">
  <label className="text-sm font-medium">Label</label>
  <Input placeholder="Placeholder" className="h-9" />
</div>
```

### Select

```tsx
<div className="space-y-2">
  <label className="text-sm font-medium">Label</label>
  <Select
    value={value}
    onChange={(e) => setValue(e.target.value)}
    options={options}
    className="h-9"
  />
</div>
```

---

## Alert/Info Boxes

```tsx
// Warning alert
<div className="flex items-center gap-2 p-3 rounded-md bg-amber-500/10 text-amber-600 dark:text-amber-400 text-sm">
  <AlertTriangle className="h-4 w-4" />
  Warning message here
</div>

// Info box
<div className="text-xs text-muted-foreground italic">
  Default: follow global strategy
</div>
```

---

## Overview Page Recommendations

Based on analysis, the Overview page should be updated to:

1. **Remove** `border-l-4` decoration from stats cards
2. **Use** CSS variables instead of hardcoded colors
3. **Simplify** stats card design to match Routing page style
4. **Unify** chart colors with CSS variables
5. **Keep** Recent Activity inline expansion pattern (matches Logs page)

### Recommended Stats Card Style

```tsx
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
```

---

## Component Checklist

When creating new pages or components:

- [ ] Use `space-y-6` for page-level spacing
- [ ] Use standard Card + CardHeader + CardContent structure
- [ ] Avoid hardcoded colors, use CSS variables
- [ ] Use Badge variants for status indicators
- [ ] Apply `text-muted-foreground` for secondary text
- [ ] Use `font-mono` for data/code values
- [ ] Apply proper hover states: `hover:bg-accent/50` or `hover:bg-accent/30`
- [ ] Use `gap-2` for button internal spacing
- [ ] Keep table headers in `bg-muted/50`
