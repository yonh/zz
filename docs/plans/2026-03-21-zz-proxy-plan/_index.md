# ZZ LLM API Reverse Proxy - Implementation Plan

## Goal
Build a lightweight, high-performance reverse proxy in Rust that sits between coding tools (Claude Code, Cursor) and multiple upstream LLM API providers with automatic failover when quota limits are hit.

## Architecture Overview
- **Body-transparent proxy**: Stream request/response without parsing (including SSE)
- **Header-aware**: Rewrite Authorization and Host per provider
- **URL-rewriting**: Map local path to upstream base URL + path
- **Failover-driven**: Detect quota exhaustion and auto-switch providers
- **Zero-downtime**: Seamless switching with retry on failover-eligible errors

## Core Constraints
1. **V1 Non-goals**: No body parsing, no auth on proxy, no TLS termination, no caching
2. **Performance**: < 1ms proxy overhead per request (excluding network)
3. **Body-transparency**: Never inspect/modify request/response content except error detection on non-2xx responses
4. **Streaming-first**: SSE must work with zero buffering

## Execution Plan

```yaml
tasks:
  - id: "001"
    subject: "Setup project dependencies and structure"
    slug: "setup-project"
    type: "setup"
    depends-on: []

  - id: "002"
    subject: "Config module - TOML parsing and validation"
    slug: "config-module"
    type: "impl"
    depends-on: ["001"]

  - id: "003"
    subject: "Provider state management - health tracking"
    slug: "provider-state"
    type: "impl"
    depends-on: ["002"]

  - id: "004"
    subject: "Router module - failover strategy"
    slug: "router-failover"
    type: "impl"
    depends-on: ["003"]

  - id: "005"
    subject: "Rewriter module - URL and header rewriting"
    slug: "rewriter-module"
    type: "impl"
    depends-on: ["004"]

  - id: "006"
    subject: "Error module - quota detection and error types"
    slug: "error-module"
    type: "impl"
    depends-on: ["002"]

  - id: "007"
    subject: "Stream module - SSE support"
    slug: "stream-module"
    type: "impl"
    depends-on: ["006"]

  - id: "008"
    subject: "Proxy module - request/response forwarding"
    slug: "proxy-core"
    type: "impl"
    depends-on: ["005", "007"]

  - id: "009"
    subject: "Admin endpoints - health/stats/reload"
    slug: "admin-endpoints"
    type: "impl"
    depends-on: ["008"]

  - id: "010"
    subject: "Logging module - structured logging"
    slug: "logging-module"
    type: "impl"
    depends-on: ["001"]

  - id: "011"
    subject: "Main entry point - server startup"
    slug: "main-entry"
    type: "impl"
    depends-on: ["008", "009", "010"]

  - id: "012"
    subject: "Integration test - manual verification"
    slug: "integration-test"
    type: "test"
    depends-on: ["011"]
```

## Task File References

- [Task 001: Setup project dependencies and structure](./task-001-setup.md)
- [Task 002: Config module](./task-002-config.md)
- [Task 003: Provider state management](./task-003-provider.md)
- [Task 004: Router module](./task-004-router.md)
- [Task 005: Rewriter module](./task-005-rewriter.md)
- [Task 006: Error module](./task-006-error.md)
- [Task 007: Stream module](./task-007-stream.md)
- [Task 008: Proxy module](./task-008-proxy.md)
- [Task 009: Admin endpoints](./task-009-admin.md)
- [Task 010: Logging module](./task-010-logging.md)
- [Task 011: Main entry point](./task-011-main.md)
- [Task 012: Integration test](./task-012-test.md)

## BDD Coverage

All scenarios from SPEC.md are covered:
- ✅ Config parsing (TOML validation, defaults, multi-provider)
- ✅ Provider health tracking (cooldown, failure counting)
- ✅ Failover routing (priority-based selection)
- ✅ URL/header rewriting (base_url + path, Authorization, Host)
- ✅ Quota detection (429, 403 with quota keywords)
- ✅ SSE streaming (chunked transfer, zero buffering)
- ✅ Admin endpoints (/zz/health, /zz/stats, /zz/reload)
- ✅ Transparent proxying (body pass-through)

## Dependency Chain

```
001 (setup)
 ├─ 002 (config) ──┐
 │   ├─ 003 (provider) ── 004 (router) ── 005 (rewriter) ──┐
 │   └───────────────────────────────────────────────────────┘
 └─ 010 (logging)                                               │
                                                                 │
002 (config) ── 006 (error) ── 007 (stream) ────────────────────┤
                                                                 │
                                                                 └─ 008 (proxy) ── 009 (admin) ──┐
                                                                                                  │
                                                                                                  └─ 011 (main) ── 012 (test)
```
