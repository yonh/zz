# Task 003: Provider State Management - Health Tracking

## Goal
Implement Provider state management with health tracking, cooldown, and failure counting.

## BDD Scenarios

```gherkin
Scenario: Provider health state transitions
  Given a healthy provider
  When mark_failure() is called
  Then failure_count increments by 1
  And health remains healthy until threshold reached

Scenario: Cooldown on quota error
  Given a healthy provider
  When mark_quota_exhausted() is called
  Then state changes to Cooldown
  And cooldown_until is set to current time + cooldown_secs

Scenario: Auto-recovery after cooldown period
  Given a provider in Cooldown state
  When cooldown period expires
  And is_available() is called
  Then returns true (provider is available again)

Scenario: Failure threshold triggers unhealthy state
  Given a provider with failure_threshold = 3
  When mark_failure() is called 3 times
  Then state changes to Unhealthy
  And recovery_at is set for recovery_secs later

Scenario: Shared state across threads
  Given a ProviderManager with multiple providers in DashMap
  When one thread marks a provider as failed
  Then another thread sees the updated failure count immediately
```

## Files to Create/Edit

**Create**:
- `src/provider.rs` - Complete implementation

## Implementation Steps

1. Define Provider state enum:
   ```rust
   enum ProviderState {
       Healthy,
       Cooldown { until: DateTime<Utc> },
       Unhealthy { recovery_at: DateTime<Utc> },
   }
   ```

2. Define Provider struct with:
   - Config (immutable)
   - Atomic/RefCell for state, failure_count, last_error
   - Methods: `mark_quota_exhausted()`, `mark_failure()`, `is_available()`, `reset()`

3. Define ProviderManager (wrapped in Arc<DashMap<String, Provider>>):
   - `new(config: &Config)` - initialize from config
   - `get_available()` - return healthy/not-cooldown providers
   - `get_by_name(name: &str)` - lookup by name
   - `mark_quota_exhausted(name: &str)` - mark specific provider
   - `mark_failure(name: &str)` - increment failure count

4. Implement health check logic:
   - Cooldown duration from config (default 60s)
   - Recovery duration from config (default 600s)
   - Auto-check state on each access (cleanup expired cooldown/unhealthy)

## Verification

Run:
```bash
cargo test --lib provider
```

Expected:
- All health state transition tests pass
- Thread-safety tests pass (concurrent access)

## Dependencies
- Task 002 (Config module for loading provider configuration)
