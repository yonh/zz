# ZZ - Web Dashboard UI Spec

## Overview

A web-based control panel (similar to Clash Dashboard / Yacd) for managing and monitoring the ZZ proxy. Served directly by the Rust backend as embedded static files. Provides real-time traffic visualization, provider management, and routing strategy control.

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Framework | React 18 + TypeScript |
| Styling | TailwindCSS 4 |
| Components | shadcn/ui |
| Icons | Lucide React |
| Charts | Recharts |
| State | Zustand |
| Real-time | WebSocket (for live stats) |
| HTTP Client | fetch API |
| Build | Vite |
| Embedding | Rust `include_dir` or `rust-embed` to embed built assets into binary |

## Architecture

```
┌─────────────────────────────────────────────┐
│                ZZ Binary                     │
│                                              │
│  ┌──────────────┐   ┌────────────────────┐  │
│  │ Proxy Engine  │   │  Admin API Server  │  │
│  │ (port 9090)   │   │  /zz/api/*         │  │
│  └──────────────┘   │  /zz/ws (WebSocket) │  │
│                      │  /zz/ui/* (static)  │  │
│                      └────────────────────┘  │
└─────────────────────────────────────────────┘
         ▲                      ▲
         │ API Traffic          │ Browser
         │                      │
    Coding Tools          Web Dashboard
```

Single binary, single port. The dashboard is served at `http://127.0.0.1:9090/zz/ui/`.

## Pages & Layout

### Global Layout

```
┌─────────────────────────────────────────────────────────┐
│  ◉ ZZ   │ Overview │ Providers │ Routing │ Logs │ Config │
├─────────┴───────────────────────────────────────────────┤
│                                                          │
│                    [Page Content]                         │
│                                                          │
└──────────────────────────────────────────────────────────┘
```

- **Top nav bar**: Logo + horizontal tabs
- **Dark/Light mode toggle** in top-right
- **Responsive**: Works on desktop (primary) and tablet

---

### Page 1: Overview (Dashboard Home)

The main dashboard showing real-time system status at a glance.

```
┌──────────────────────────────────────────────────────┐
│  Overview                                             │
├──────────┬──────────┬──────────┬────────────────────┤
│ Total    │ Active   │ Healthy  │ Current Strategy    │
│ Requests │ Providers│ Providers│ ● Failover          │
│  12,847  │   3/5    │   4/5    │   [Change ▾]        │
├──────────┴──────────┴──────────┴────────────────────┤
│                                                      │
│  📊 Request Rate (last 1h)        Traffic by Provider│
│  ┌──────────────────────┐   ┌──────────────────────┐│
│  │ ▁▂▃▅▇█▇▅▃▂▁▂▃▅▇   │   │  ██████ Ali-1  45%   ││
│  │ Line Chart (req/min) │   │  ████   Zhipu  30%   ││
│  │                      │   │  ███    Ali-2  25%   ││
│  └──────────────────────┘   └──────────────────────┘│
│                                                      │
│  Recent Activity                                     │
│  ┌──────────────────────────────────────────────────┐│
│  │ 13:05:02  ali-1      POST /v1/chat/completions ✓││
│  │ 13:04:58  ali-1      POST /v1/chat/completions ✓││
│  │ 13:04:51  zhipu-1    POST /v1/chat/completions ✓││
│  │ 13:04:30  ali-1      429 → failover → zhipu-1  ⚠││
│  └──────────────────────────────────────────────────┘│
└──────────────────────────────────────────────────────┘
```

**Components:**
- **Stats cards**: Total requests, active/healthy provider counts, current strategy with **quick-switch dropdown**
  - Strategy card MUST include a `<Select>` dropdown allowing the user to change strategy directly from the Overview page without navigating to Routing
  - Changing strategy via dropdown dispatches `setStrategy()` and shows a toast notification
- **Request rate chart**: Line chart showing requests/minute over last 1h (Recharts)
- **Traffic distribution**: Horizontal bar chart per provider, driven by **live provider stats** from the store (not static mock data)
- **Activity feed**: Scrolling list of recent requests with status, auto-updates via WebSocket
  - MUST show a **live indicator** (pulsing icon + "Live" label) in the section header
  - Newly arrived log entries MUST have a **highlight animation** (e.g., background glow for 1.5s) to distinguish them from existing entries
  - Failover events should display a warning icon and "failover" label

---

### Page 2: Providers

Manage and monitor all configured upstream providers.

```
┌──────────────────────────────────────────────────────┐
│  Providers                           [+ Add Provider]│
├──────────────────────────────────────────────────────┤
│                                                      │
│  ┌─ ali-account-1 ──────────────────────────────┐   │
│  │ ● Healthy    Priority: 1    Requests: 5,432  │   │
│  │ Base URL: https://dashscope.aliyuncs.com/...  │   │
│  │ Errors: 12 (0.2%)   Avg Latency: 1.2s        │   │
│  │ Models: qwen-plus, qwen-turbo                 │   │
│  │                                                │   │
│  │ ▁▂▃▅▇█▅▃ (latency sparkline)                 │   │
│  │                                                │   │
│  │ [Disable] [Edit] [Test Connection] [Move ↑↓]  │   │
│  └────────────────────────────────────────────────┘   │
│                                                      │
│  ┌─ zhipu-account-1 ───────────────────────────┐    │
│  │ ● Healthy    Priority: 2    Requests: 3,215  │   │
│  │ Base URL: https://open.bigmodel.cn/api/...    │   │
│  │ Errors: 5 (0.15%)  Avg Latency: 0.8s         │   │
│  │ Models: glm-4, glm-4-flash                   │   │
│  │                                                │   │
│  │ ▁▂▃▅▇█▅▃ (latency sparkline)                 │   │
│  │                                                │   │
│  │ [Disable] [Edit] [Test Connection] [Move ↑↓]  │   │
│  └────────────────────────────────────────────────┘   │
│                                                      │
│  ┌─ ali-account-2 ──────────────────────────────┐   │
│  │ ⚠ Cooldown (quota exceeded, recovers 13:15)  │   │
│  │ ... (dimmed card)                              │   │
│  └────────────────────────────────────────────────┘   │
│                                                      │
└──────────────────────────────────────────────────────┘
```

**Features per provider card:**
- **Status badge**: Healthy (green), Cooldown (yellow), Unhealthy (red), Disabled (gray)
- **Key stats**: Request count, error rate, average latency
- **Latency sparkline**: Mini chart showing recent latency trend
- **Actions**:
  - **Disable/Enable**: Toggle provider on/off without removing config. MUST show toast notification on toggle.
  - **Edit**: Modal form with ALL editable fields: `base_url`, `api_key`, `priority`, `weight`, `models`. API key input uses `type="password"`. Save dispatches `updateProvider()` and shows success toast.
  - **Test Connection**: Simulate a test request with **loading spinner** on the button. On completion show success toast (with latency) or error toast. Disabled when provider is disabled.
  - **Reorder**: Drag-and-drop via `@dnd-kit` with grip handle. On reorder, update priority numbers and show toast.
- **Cooldown indicator**: Shows remaining cooldown time with **live countdown timer** (updating every second until recovery). Display in a warning-styled banner with `animate-pulse`.
- **Add Provider**: Modal form with fields: `name` (required, unique), `base_url`, `api_key`, `priority`, `weight`, `models`. On save, append to providers list and show success toast. The `[+ Add Provider]` button MUST open this modal.
- **API key masking**: Keys shown as `sk-****xxxx` by default, with an eye icon toggle to reveal/hide.

---

### Page 3: Routing

Central control for routing strategy and rules.

```
┌──────────────────────────────────────────────────────┐
│  Routing Strategy                                     │
├──────────────────────────────────────────────────────┤
│                                                      │
│  Select Strategy:                                    │
│  ┌────────────┐ ┌────────────┐ ┌────────────┐      │
│  │  Failover   │ │ Round Robin│ │  Weighted   │      │
│  │  ● Active   │ │            │ │  Random     │      │
│  │             │ │            │ │             │      │
│  │ Use providers│ │ Distribute │ │ Random pick │      │
│  │ in priority │ │ evenly     │ │ by weight   │      │
│  │ order, auto │ │ across all │ │ values      │      │
│  │ switch on   │ │ healthy    │ │             │      │
│  │ failure     │ │ providers  │ │             │      │
│  └────────────┘ └────────────┘ └────────────┘      │
│  ┌────────────┐ ┌────────────┐                      │
│  │  Quota-     │ │  Manual /  │                      │
│  │  Aware      │ │  Fixed     │                      │
│  │             │ │            │                      │
│  │ Track token │ │ Always use │                      │
│  │ usage and   │ │ a specific │                      │
│  │ switch at   │ │ provider   │                      │
│  │ threshold   │ │ (pin)      │                      │
│  └────────────┘ └────────────┘                      │
│                                                      │
├──────────────────────────────────────────────────────┤
│  Failover Settings                                   │
│  ┌──────────────────────────────────────────────┐   │
│  │ Max retries per request:  [3    ▾]            │   │
│  │ Cooldown after quota error: [60   ] seconds   │   │
│  │ Failure threshold:        [3    ] consecutive │   │
│  │ Recovery check interval:  [600  ] seconds     │   │
│  │                                                │   │
│  │              [Apply Changes]                   │   │
│  └──────────────────────────────────────────────┘   │
│                                                      │
├──────────────────────────────────────────────────────┤
│  Provider Priority / Weight                          │
│  ┌──────────────────────────────────────────────┐   │
│  │  Provider        Priority  Weight  Status     │   │
│  │  ali-account-1      1       50     ● Healthy  │   │
│  │  zhipu-account-1    2       30     ● Healthy  │   │
│  │  ali-account-2      3       20     ⚠ Cooldown │   │
│  │                                                │   │
│  │  (drag to reorder / click to edit values)     │   │
│  └──────────────────────────────────────────────┘   │
│                                                      │
├──────────────────────────────────────────────────────┤
│  Model Routing Rules (Optional)                      │
│  ┌──────────────────────────────────────────────┐   │
│  │  Rule: model = "qwen-*"  → ali-account-1     │   │
│  │  Rule: model = "glm-*"   → zhipu-account-1   │   │
│  │  Default: follow strategy                     │   │
│  │                                    [+ Add Rule]│   │
│  └──────────────────────────────────────────────┘   │
│                                                      │
└──────────────────────────────────────────────────────┘
```

**Routing Strategies:**

| Strategy | Description | UI Control |
|----------|-------------|------------|
| **Failover** | Try providers in priority order; auto-switch on failure | Priority list (drag-to-reorder) |
| **Round Robin** | Distribute evenly across healthy providers | Toggle on/off per provider |
| **Weighted Random** | Random selection weighted by `weight` value | Weight sliders per provider |
| **Quota-Aware** | Track token usage per provider, switch at threshold | Token budget input per provider |
| **Manual / Fixed** | Pin all traffic to one specific provider | Provider dropdown selector |

**Strategy Settings (dynamic per selected strategy):**

The settings section below the strategy selector MUST dynamically change based on the currently active strategy:

- **Failover**: Show `max_retries`, `cooldown_secs`, `failure_threshold`, `recovery_secs` inputs with an "Apply Changes" button.
- **Round Robin**: Show a list of providers with on/off toggles to include/exclude from the rotation pool.
- **Weighted Random**: Show a list of providers with **weight sliders** (range 0-100). Weights are normalized to percentages. Include a visual bar showing relative distribution.
- **Quota-Aware**: Show a list of providers with **token budget** number inputs (monthly/daily limit). Show current usage percentage bar. Include a **threshold** input (e.g., switch at 90% usage).
- **Manual / Fixed**: Show a single **provider dropdown selector** to pin all traffic. Display a warning if the selected provider is unhealthy/disabled.

All strategy settings changes MUST show a toast notification on apply.

**Model Routing Rules:**
- Optional rules that override the global strategy for specific models
- Pattern matching (glob: `qwen-*`, `glm-*`, exact: `gpt-4`)
- Useful when certain models are only available on certain providers
- Each rule has: pattern input + target provider `<Select>` dropdown + delete button
- "Default: follow global strategy" label shown below rules list
- Add Rule form: pattern `<Input>` + provider `<Select>` + "Add Rule" `<Button>`
- On add/delete MUST show toast notification

---

### Page 4: Logs

Real-time request log viewer with filtering.

```
┌──────────────────────────────────────────────────────┐
│  Logs                                                 │
├──────────────────────────────────────────────────────┤
│  Filter: [All ▾] [All Providers ▾] [Search...      ]│
│          Status    Provider          Keyword          │
├──────────────────────────────────────────────────────┤
│  Time       Provider    Method  Path              St │
│  ─────────────────────────────────────────────────── │
│  13:05:02   ali-1       POST    /v1/chat/comp..  200 │
│  13:04:58   ali-1       POST    /v1/chat/comp..  200 │
│  13:04:51   zhipu-1     POST    /v1/chat/comp..  200 │
│  13:04:30   ali-1       POST    /v1/chat/comp..  429 │
│             ↳ Failover → zhipu-1                 200 │
│  13:04:15   ali-1       POST    /v1/embeddings   200 │
│  13:03:58   ali-1       POST    /v1/chat/comp..  200 │
│  ...                                                 │
├──────────────────────────────────────────────────────┤
│  ── Log Detail (click row to expand) ──              │
│  Request ID: req_abc123                              │
│  Duration: 2.3s (TTFB: 0.8s)                        │
│  Model: qwen-plus                                    │
│  Streaming: Yes                                      │
│  Provider chain: ali-1 (429) → zhipu-1 (200)        │
│  Request size: 1.2 KB                                │
│  Response size: 3.4 KB                               │
└──────────────────────────────────────────────────────┘
```

**Features:**
- **Real-time streaming** via WebSocket (new logs appear at top)
- **Filters**: Status code (2xx/4xx/5xx/all-errors), provider dropdown, keyword search input
  - Filter bar uses a compact horizontal layout with icon prefix
  - Show `{filtered} / {total}` count label
- **Expandable rows**: Click to see full request detail panel below the row
  - Detail panel shows: Request ID, Duration (with TTFB), Model, Streaming (Yes/No), Request Size, Response Size, Failover Chain (if any)
  - Failover Chain rendered as a horizontal badge sequence with arrow icons between steps
- **Failover chain**: Rows with failover events have a subtle amber background tint
- **Export**: Download button MUST trigger a real file download
  - Export as JSON: `JSON.stringify(filteredLogs, null, 2)` as `.json` file
  - Use `Blob` + `URL.createObjectURL` + synthetic `<a>` click pattern
- **Auto-scroll toggle**: Pause/Resume button in the header
  - When `autoScroll=true`, the log container MUST auto-scroll to top when new logs arrive (use `useEffect` + `scrollTop = 0` on the container ref)
  - When `autoScroll=false`, the scroll position stays where the user left it
- **Buffer**: Keep last 1000 entries in memory (enforced in `store.addLog`), older entries available via API pagination

---

### Page 5: Config

View and edit the raw configuration with validation.

```
┌──────────────────────────────────────────────────────┐
│  Configuration                                        │
├──────────────────────────────────────────────────────┤
│                                                      │
│  ┌─ Editor ────────────────────────────────────┐    │
│  │ [server]                                     │    │
│  │ listen = "127.0.0.1:9090"                    │    │
│  │ request_timeout_secs = 300                   │    │
│  │                                               │    │
│  │ [routing]                                     │    │
│  │ strategy = "failover"                         │    │
│  │ ...                                           │    │
│  │                                               │    │
│  │ (TOML editor with syntax highlighting)       │    │
│  └───────────────────────────────────────────────┘    │
│                                                      │
│  ● Config valid                                      │
│  [Save & Reload]  [Reset to File]  [Download]        │
│                                                      │
├──────────────────────────────────────────────────────┤
│  Config File Path: /Users/xxx/.config/zz/config.toml │
│  Last modified: 2026-03-21 12:30:00                  │
│  Last reloaded: 2026-03-21 12:30:05                  │
└──────────────────────────────────────────────────────┘
```

**Features:**
- **TOML editor** with syntax highlighting
  - Use `CodeMirror` with TOML language support (package: `@codemirror/lang-json` or `codemirror-lang-toml`) OR a `<Textarea>` with `font-mono` styling as a minimum viable fallback
  - Editor MUST have: monospace font, line numbers (optional), adequate min-height (~500px), resizable
- **Real-time validation**: Shows errors inline as you type
  - Validation badge: "Valid" (green) or "Invalid" (red) next to the title
  - "Unsaved changes" warning badge when editor content differs from last-saved state
  - Basic TOML structure checks: must contain `[server]` and `[[providers]]` sections
- **Save & Reload**: Write to disk + hot-reload proxy config. MUST show success toast on save.
- **Reset**: Revert editor to current on-disk config. MUST show info toast on reset.
- **Download**: Export current editor content as `config.toml` file download.
- **API key masking in editor**: When displaying the TOML content, API key values (`api_key = "..."`) should be masked as `sk-****xxxx` by default. Provide a toggle button (eye icon) to reveal/hide all keys in the editor.
- **Metadata footer**: Show config file path and **both** "Last modified" and "Last reloaded" timestamps.

---

## Admin REST API (Backend)

The UI communicates with the backend via these endpoints:

### Providers

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/zz/api/providers` | List all providers with status and stats |
| `PUT` | `/zz/api/providers/{name}` | Update provider config (priority, weight, enabled, etc.) |
| `POST` | `/zz/api/providers` | Add new provider |
| `DELETE` | `/zz/api/providers/{name}` | Remove provider |
| `POST` | `/zz/api/providers/{name}/test` | Test provider connectivity |
| `POST` | `/zz/api/providers/{name}/enable` | Enable provider |
| `POST` | `/zz/api/providers/{name}/disable` | Disable provider |
| `POST` | `/zz/api/providers/{name}/reset` | Reset health/cooldown state |

### Routing

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/zz/api/routing` | Get current routing strategy and settings |
| `PUT` | `/zz/api/routing` | Update routing strategy and settings |
| `GET` | `/zz/api/routing/rules` | Get model routing rules |
| `PUT` | `/zz/api/routing/rules` | Update model routing rules |

### Stats & Logs

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/zz/api/stats` | Aggregated stats (request counts, error rates, latency) |
| `GET` | `/zz/api/stats/timeseries?period=1h` | Time-series data for charts |
| `GET` | `/zz/api/logs?limit=100&offset=0` | Paginated request logs |
| `WS` | `/zz/ws` | WebSocket for real-time stats + log streaming |

### Config

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/zz/api/config` | Get current config (TOML string) |
| `PUT` | `/zz/api/config` | Validate + save + hot-reload config |
| `POST` | `/zz/api/config/validate` | Validate config without saving |

### System

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/zz/api/health` | Proxy health check |
| `GET` | `/zz/api/version` | Version info |

---

## WebSocket Protocol

Endpoint: `ws://127.0.0.1:9090/zz/ws`

### Server → Client Messages

```jsonc
// Real-time request log entry
{
  "type": "log",
  "data": {
    "id": "req_abc123",
    "timestamp": "2026-03-21T13:05:02Z",
    "method": "POST",
    "path": "/v1/chat/completions",
    "provider": "ali-account-1",
    "status": 200,
    "duration_ms": 2300,
    "ttfb_ms": 800,
    "model": "qwen-plus",
    "streaming": true,
    "request_bytes": 1200,
    "response_bytes": 3400,
    "failover_chain": null  // or ["ali-1:429", "zhipu-1:200"]
  }
}

// Provider state change
{
  "type": "provider_state",
  "data": {
    "name": "ali-account-1",
    "status": "cooldown",  // healthy | cooldown | unhealthy | disabled
    "cooldown_until": "2026-03-21T13:15:00Z",
    "consecutive_failures": 3
  }
}

// Periodic stats snapshot (every 5s)
{
  "type": "stats",
  "data": {
    "total_requests": 12847,
    "requests_per_minute": 23.5,
    "active_providers": 3,
    "healthy_providers": 4,
    "total_providers": 5,
    "strategy": "failover",
    "per_provider": {
      "ali-account-1": { "requests": 5432, "errors": 12, "avg_latency_ms": 1200 },
      "zhipu-account-1": { "requests": 3215, "errors": 5, "avg_latency_ms": 800 }
    }
  }
}
```

### Client → Server Messages

```jsonc
// Subscribe to specific event types (default: all)
{
  "type": "subscribe",
  "events": ["log", "provider_state", "stats"]
}
```

---

## UI Component Tree

```
App
├── Layout
│   ├── TopNav (logo, page tabs, dark mode toggle)
│   └── PageContent
│       ├── OverviewPage
│       │   ├── StatsCards (total requests, active/healthy providers, strategy selector)
│       │   ├── RequestRateChart (Recharts line chart)
│       │   ├── TrafficDistribution (Recharts bar/pie chart)
│       │   └── ActivityFeed (real-time log list)
│       ├── ProvidersPage
│       │   ├── ProviderCard[] (status, stats, sparkline, actions)
│       │   ├── AddProviderModal (form)
│       │   └── EditProviderModal (form)
│       ├── RoutingPage
│       │   ├── StrategySelector (card grid)
│       │   ├── StrategySettings (dynamic form per strategy)
│       │   ├── ProviderPriorityTable (draggable table)
│       │   └── ModelRoutingRules (rule list + add form)
│       ├── LogsPage
│       │   ├── LogFilters (status, provider, search)
│       │   ├── LogTable (virtualized, expandable rows)
│       │   └── LogDetail (side panel or expanded row)
│       └── ConfigPage
│           ├── ConfigEditor (CodeMirror with TOML highlighting)
│           ├── ValidationStatus (inline errors)
│           └── ConfigActions (save, reset, download)
├── WebSocketProvider (context for real-time data)
└── ThemeProvider (dark/light mode)
```

---

## UI Directory Structure

```
ui/
├── index.html
├── package.json
├── vite.config.ts
├── tsconfig.json
├── tailwind.config.ts
├── postcss.config.js
├── src/
│   ├── main.tsx
│   ├── App.tsx
│   ├── index.css               # Tailwind imports
│   ├── api/
│   │   ├── client.ts           # Fetch wrapper for /zz/api/*
│   │   └── types.ts            # TypeScript types matching API responses
│   ├── hooks/
│   │   ├── useWebSocket.ts     # WebSocket connection + auto-reconnect
│   │   ├── useProviders.ts     # Provider data + mutations
│   │   ├── useRouting.ts       # Routing config
│   │   ├── useStats.ts         # Real-time stats
│   │   └── useLogs.ts          # Log data + filters
│   ├── stores/
│   │   └── store.ts            # Zustand store (real-time state from WS)
│   ├── components/
│   │   ├── layout/
│   │   │   ├── TopNav.tsx
│   │   │   └── Layout.tsx
│   │   ├── providers/
│   │   │   ├── ProviderCard.tsx
│   │   │   ├── ProviderForm.tsx
│   │   │   └── ProviderStatusBadge.tsx
│   │   ├── routing/
│   │   │   ├── StrategySelector.tsx
│   │   │   ├── StrategySettings.tsx
│   │   │   ├── PriorityTable.tsx
│   │   │   └── ModelRuleEditor.tsx
│   │   ├── logs/
│   │   │   ├── LogTable.tsx
│   │   │   ├── LogFilters.tsx
│   │   │   └── LogDetail.tsx
│   │   ├── charts/
│   │   │   ├── RequestRateChart.tsx
│   │   │   ├── TrafficDistribution.tsx
│   │   │   └── LatencySparkline.tsx
│   │   └── common/
│   │       ├── StatsCard.tsx
│   │       └── StatusIndicator.tsx
│   └── pages/
│       ├── Overview.tsx
│       ├── Providers.tsx
│       ├── Routing.tsx
│       ├── Logs.tsx
│       └── Config.tsx
└── components.json             # shadcn/ui config
```

---

## Visual Design Guidelines

### Theme
- **Dark mode default** (coding tool users prefer dark themes)
- Light mode available via toggle
- Color palette aligned with modern dev tool aesthetics (similar to Clash Verge / Yacd)

### Status Colors
| Status | Color | Usage |
|--------|-------|-------|
| Healthy | `emerald-500` | Provider online, no issues |
| Cooldown | `amber-500` | Quota hit, waiting to recover |
| Unhealthy | `red-500` | Multiple failures |
| Disabled | `zinc-400` | Manually disabled |

### Provider Traffic Flow Visualization
- On Overview page, optional **Sankey diagram** or **animated flow** showing:
  ```
  [Client] ──▶ [ZZ Proxy] ──▶ [Ali-1]    45% ███████
                           ──▶ [Zhipu-1]  30% █████
                           ──▶ [Ali-2]    25% ████
  ```
- Use animated dots/particles on the flow lines for active requests (like Clash connection animation)

### Responsive Behavior
- Primary target: **Desktop** (1280px+)
- Tablet (768px+): Stack cards vertically, collapse charts
- Mobile: Not a priority (local tool)

---

## Data Flow Summary

```
                    ┌─────────────┐
                    │  Zustand     │
     ┌─────────────│  Store       │──────────────┐
     │              └──────┬──────┘              │
     │                     │                      │
  WebSocket          REST API calls          React Components
  (real-time)        (mutations)             (read from store)
     │                     │                      │
     ▼                     ▼                      ▼
┌─────────┐        ┌─────────────┐        ┌─────────────┐
│ /zz/ws  │        │ /zz/api/*   │        │ UI Render   │
└─────────┘        └─────────────┘        └─────────────┘
```

1. **Initial load**: REST API fetches current state → populate store
2. **Real-time updates**: WebSocket pushes stats/logs/state changes → update store
3. **User actions**: UI dispatches REST API calls → backend processes → broadcasts update via WS → store updates → UI re-renders

---

## Build & Embedding

1. `cd ui && pnpm build` → produces `ui/dist/`
2. Rust binary uses `rust-embed` to embed `ui/dist/**` at compile time
3. Requests to `/zz/ui/*` serve embedded static files
4. Single binary distribution, no separate frontend deployment needed
5. Dev mode: `vite dev` with proxy to Rust backend for hot-reload during development
