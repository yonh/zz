# ZZ UI - Implementation Status & Fix Guide

This document tracks the current UI prototype implementation status against SPEC.md and SPEC-UI.md.

**Last audited**: 2026-03-21

---

## Implementation Status Matrix

### Legend

- ✅ Fully implemented and matches spec
- ⚠️ Partially implemented (minor gap)
- ❌ Not implemented or missing

---

## Page 1: Overview

| Feature | Status | Notes |
|---------|--------|-------|
| Stats cards (4 cards) | ✅ | Total Requests, Active/Healthy Providers, Strategy |
| Strategy quick-switch dropdown | ✅ | `<Select>` with 5 strategies + toast on change |
| Request Rate chart (line, 1h) | ✅ | Uses `generateRequestRateData()` |
| Traffic Distribution chart (live) | ✅ | Driven by live provider stats from store |
| Activity Feed (real-time) | ✅ | WebSocket mock pushes new logs |
| Live indicator (pulse + "Live") | ✅ | Radio icon + label |
| New-log highlight animation | ✅ | 1.5s bg-primary glow on new entries |
| Failover event indicator | ✅ | Warning icon + "failover" label |

---

## Page 2: Providers

| Feature | Status | Notes |
|---------|--------|-------|
| Status badges (4 states) | ✅ | Healthy/Cooldown/Unhealthy/Disabled |
| Key stats (requests, errors, rate, latency) | ✅ | 4-column grid |
| Latency sparkline | ✅ | Recharts LineChart |
| Drag-and-drop reorder (@dnd-kit) | ✅ | GripVertical handle |
| Disable/Enable toggle + toast | ✅ | |
| Edit modal (all fields incl. priority) | ✅ | base_url, api_key, priority, weight, models |
| Test Connection (spinner + toast) | ✅ | Random success/failure simulation |
| API key masking (eye toggle) | ✅ | `sk-****xxxx` format |
| Cooldown indicator (live countdown) | ✅ | `CooldownCountdown` component updating every 1s |
| Add Provider modal | ✅ | Full form with validation (name unique, base_url required) |

---

## Page 3: Routing

| Feature | Status | Notes |
|---------|--------|-------|
| Strategy selector (5 cards) | ✅ | Active badge on selected |
| Strategy change toast | ✅ | |
| Dynamic strategy settings | ✅ | `StrategySettings` switch renders per-strategy UI |
| — Failover settings | ✅ | max_retries, cooldown_secs, failure_threshold, recovery_secs + Apply |
| — Round Robin settings | ✅ | Provider on/off toggles with rotation count |
| — Weighted Random settings | ✅ | Weight sliders (range 0-100) + percentage + Apply |
| — Quota-Aware settings | ✅ | Token budget inputs + threshold input + Apply |
| — Manual / Fixed settings | ✅ | Provider dropdown + Pin button + unhealthy warning |
| Provider Priority/Weight table (DnD) | ✅ | Sortable rows |
| Model Routing Rules (CRUD) | ✅ | Pattern + target + delete + add form |
| Add Rule form + toast | ✅ | |

---

## Page 4: Logs

| Feature | Status | Notes |
|---------|--------|-------|
| Real-time log streaming | ✅ | Via mock WebSocket |
| Filters (status/provider/keyword) | ✅ | With `{filtered} / {total}` count label |
| Expandable rows with detail | ✅ | Full detail panel |
| Failover chain visualization | ✅ | Badge sequence with arrows |
| Failover row tint | ✅ | Amber background |
| Export button (JSON download) | ✅ | `handleExport()` with Blob + toast |
| Auto-scroll toggle (wired to DOM) | ✅ | `scrollContainerRef` + `useEffect` scrolls to top |
| 1000-entry buffer | ✅ | Enforced in `store.addLog` |

---

## Page 5: Config

| Feature | Status | Notes |
|---------|--------|-------|
| TOML editor (textarea) | ✅ | Monospace, resizable, min-h 500px |
| Syntax highlighting | ⚠️ | Plain `<Textarea>` fallback (CodeMirror optional future enhancement) |
| Real-time validation | ✅ | Basic heuristic (checks `[server]` + `[[providers]]`) |
| Validation badges | ✅ | Valid/Invalid/Unsaved |
| Save & Reload + toast | ✅ | |
| Reset + toast | ✅ | |
| Download | ✅ | Real `.toml` file download |
| API key masking in editor | ✅ | Eye toggle button, `maskApiKeys()` regex replace |
| Metadata: Last modified + Last reloaded | ✅ | Both timestamps shown, updated on edit/save |

---

## Store & Types

| Item | Status | Notes |
|------|--------|-------|
| `Provider.headers` field | ✅ | Optional `headers?: Record<string, string>` |
| `Provider.token_budget` field | ✅ | Optional `token_budget?: number` |
| `RoutingConfig.pinned_provider` | ✅ | Optional `pinned_provider?: string` |
| `addProvider` store action | ✅ | Appends provider + updates systemStats counts |
| `removeProvider` store action | ✅ | Removes by name + updates systemStats counts |
| `setPinnedProvider` store action | ✅ | Sets `routingConfig.pinned_provider` |
| `updateProviderWeight` store action | ✅ | Updates individual provider weight |
| `SystemStats.per_provider` | N/A | Data available via `Provider.stats`; not needed separately |

---

## Structural Items (acceptable for prototype, address before production)

| Item | Status | Notes |
|------|--------|-------|
| Component file splitting | ⚠️ | 5 page-level files with inline components. Refactor when complexity grows. |
| `api/client.ts` REST wrapper | ❌ | Create when backend REST API is ready |
| Individual hooks (`useProviders.ts` etc.) | ❌ | Extract from store when refactoring |
| `components.json` (shadcn config) | ❌ | Add if using `npx shadcn-ui add` CLI |
| Real WebSocket client | ❌ | Replace `useMockWebSocket` when backend `/zz/ws` is ready |
| CodeMirror syntax highlighting | ⚠️ | Optional enhancement for Config editor |

---

## Summary

All **P0**, **P1**, **P2**, and **P3** functional gaps identified in the initial audit have been resolved. The only remaining items are:

1. **Structural refactoring** (component splitting, dedicated hooks) — deferred to production phase
2. **Backend integration** (REST client, real WebSocket) — blocked on backend implementation
3. **CodeMirror** syntax highlighting — optional enhancement, current `<Textarea>` fallback is spec-compliant

**TypeScript compilation**: ✅ Zero errors (`npx tsc --noEmit` passes clean)
