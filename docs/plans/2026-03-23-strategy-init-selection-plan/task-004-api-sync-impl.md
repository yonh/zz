# Task 004: Strategy API Sync Implementation

**depends-on**: task-003-api-sync-test

## Description

Ensure the Strategy selector properly updates when API data loads and maintains synchronization between systemStats.strategy and routingConfig.strategy store slices.

## Execution Context

**Task Number**: 004 of 004
**Phase**: Integration
**Prerequisites**: Task 003 test must be failing (Red state)

## BDD Scenario

```gherkin
Scenario: Strategy remains synchronized between systemStats and routingConfig
  Given the user is viewing the Overview page
  And the current strategy is "failover"
  When the user changes the strategy to "weighted-random"
  Then both systemStats.strategy and routingConfig.strategy should be "weighted-random"
  And the UI should reflect "Weighted Random" in the selector
```

**Spec Source**: `../2026-03-23-strategy-init-selection-design/bdd-specs.md` (Scenarios 2 & 3)

## Files to Modify/Create

- Modify: `ui/src/stores/store.ts:98-102` (setStrategy action)
- Modify: `ui/src/pages/__tests__/Overview.strategy.test.tsx` (add sync test)

## Steps

### Step 1: Analyze Current Store Implementation
- Review the setStrategy action in store.ts lines 98-102
- Verify it updates both systemStats.strategy and routingConfig.strategy

### Step 2: Implement/Verify Sync Logic (Green)
- Ensure the initFromApi function properly syncs strategy from API to both store slices
- Verify setStrategy updates both slices atomically
- If needed, add additional sync logic or fix timing issues

### Step 3: Add Sync Test
- Add test name: `should keep systemStats and routingConfig in sync`
- Test should:
  - Change strategy via setStrategy action
  - Verify both store slices have the same strategy value
  - Verify UI reflects the change

### Step 4: Verify Implementation
- Run: `cd ui && npm test -- --run Overview.strategy.test.tsx`
- Confirm ALL tests now PASS (Green)

## Verification Commands

```bash
# Run specific test
cd ui && npm test -- --run Overview.strategy.test.tsx

# Run all tests
cd ui && npm test -- --run

# Manual verification
cd ui && npm run dev
# Test: Change strategy, refresh page, verify strategy persists
```

## Success Criteria

- All tests pass
- Strategy selector updates correctly after API load
- systemStats.strategy and routingConfig.strategy stay synchronized
- No regressions in existing functionality
