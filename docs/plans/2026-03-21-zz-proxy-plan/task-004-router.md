# Task 004: Router Module - Failover Strategy

## Goal
Implement routing strategies with failover as default, supporting provider selection based on health and priority.

## BDD Scenarios

```gherkin
Scenario: Failover selects highest priority healthy provider
  Given three providers: ali-account-1 (priority=1), zhipu-account-1 (priority=2), ali-account-2 (priority=3)
  And all providers are healthy
  When select_provider() is called with strategy=failover
  Then returns ali-account-1

Scenario: Failover skips cooldown providers
  Given ali-account-1 (priority=1, state=Cooldown)
  And zhipu-account-1 (priority=2, state=Healthy)
  When select_provider() is called
  Then returns zhipu-account-1 (skips ali-account-1)

Scenario: Failover returns None when all providers unavailable
  Given all providers are in Cooldown or Unhealthy state
  When select_provider() is called
  Then returns None

Scenario: Round-robin distributes evenly
  Given three healthy providers
  When select_provider() is called 3 times with strategy=round-robin
  Then each provider is selected once (no repeats)

Scenario: Weighted-random respects weights
  Given provider A (weight=5), provider B (weight=1)
  When select_provider() is called many times
  Then provider A is selected approximately 5x more often than B
```

## Files to Create/Edit

**Create**:
- `src/router.rs` - Complete implementation

## Implementation Steps

1. Define RoutingStrategy enum:
   ```rust
   enum RoutingStrategy {
       Failover,
       RoundRobin,
       WeightedRandom,
   }
   ```

2. Define Router struct:
   - Reference to ProviderManager (Arc)
   - Current strategy
   - Round-robin index (AtomicUsize for thread-safety)

3. Implement `select_provider(&self) -> Option<Provider>`:
   - **Failover**: Sort by priority, filter healthy/available, return first
   - **RoundRobin**: Cycle through healthy providers
   - **WeightedRandom**: Select based on weight distribution

4. Implement `try_next_provider(exclude: &str) -> Option<Provider>`:
   - Used for retry logic - select next provider excluding the failed one
   - Same logic as select_provider but filter out excluded name

## Verification

Run:
```bash
cargo test --lib router
```

Expected:
- Priority-based selection tests pass
- Cooldown skipping tests pass
- All-strategy tests pass

## Dependencies
- Task 003 (Provider state management with health tracking)
