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
- **Stats cards**: Total requests, active/healthy provider counts, current strategy (quick-switch dropdown)
- **Request rate chart**: Line chart showing requests/minute over last 1h (Recharts)
- **Traffic distribution**: Horizontal bar chart or pie chart per provider
- **Activity feed**: Scrolling list of recent requests with status, auto-updates via WebSocket

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
  - **Disable/Enable**: Toggle provider on/off without removing config
  - **Edit**: Inline edit or modal for base_url, api_key, priority, weight, models
  - **Test Connection**: Send a lightweight test request to verify connectivity
  - **Reorder**: Drag-and-drop or arrow buttons to change priority
- **Cooldown indicator**: Shows remaining cooldown time with countdown
- **Add Provider**: Modal form to add new provider (hot-reload, no restart)

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

**Model Routing Rules:**
- Optional rules that override the global strategy for specific models
- Pattern matching (glob: `qwen-*`, `glm-*`, exact: `gpt-4`)
- Useful when certain models are only available on certain providers

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
- **Filters**: Status code (2xx/4xx/5xx), provider, keyword search
- **Expandable rows**: Click to see full request detail
- **Failover chain**: Visual indicator when a request was retried across providers
- **Export**: Download logs as JSON/CSV
- **Auto-scroll toggle**: Pin to latest or free-scroll
- **Buffer**: Keep last 1000 entries in memory, older entries available via API pagination

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
- **TOML editor** with syntax highlighting (Monaco Editor or CodeMirror)
- **Real-time validation**: Shows errors inline as you type
- **Save & Reload**: Write to disk + hot-reload proxy config
- **Reset**: Revert editor to current on-disk config
- **API key masking**: Keys shown as `sk-****xxxx` by default, click to reveal

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
