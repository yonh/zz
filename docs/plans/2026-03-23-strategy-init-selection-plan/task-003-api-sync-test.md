# Task 003: Strategy API Sync Test

**depends-on**: task-002-strategy-selector-impl

## Description

Create a test that verifies the Strategy selector updates correctly when API data loads, ensuring the displayed strategy reflects the server-provided value.

## Execution Context

**Task Number**: 003 of 004
**Phase**: Integration
**Prerequisites**: Task 002 must be complete with passing tests

## BDD Scenario

```gherkin
Scenario: API-provided strategy is displayed after initialization
  Given the application is loading data from the API
  When the initFromApi function completes successfully
  And the API returns systemStats with strategy "round-robin"
  Then the Strategy selector should update to display "Round Robin"
  And the selection should persist across re-renders
```

**Spec Source**: `../2026-03-23-strategy-init-selection-design/bdd-specs.md` (Scenario 2)

## Files to Modify/Create

- Modify: `ui/src/pages/__tests__/Overview.strategy.test.tsx`

## Steps

### Step 1: Verify Scenario
- Ensure `API-provided strategy is displayed after initialization` exists in the BDD specs

### Step 2: Implement Test (Red)
- Add test to existing test file: `ui/src/pages/__tests__/Overview.strategy.test.tsx`
- Test name: `should update strategy display after API load`
- Test should:
  - Mock the API to return a different strategy (e.g., "round-robin")
  - Trigger the initFromApi action
  - Wait for state update
  - Assert that the selector displays "Round Robin"
- **Verification**: Run test command and verify it FAILS

### Step 3: Verify Test Failure
- Run: `cd ui && npm test -- --run Overview.strategy.test.tsx`
- Confirm new test fails appropriately

## Verification Commands

```bash
# Run specific test
cd ui && npm test -- --run Overview.strategy.test.tsx

# Run all Overview tests
cd ui && npm test -- --run Overview
```

## Success Criteria

- Test added to existing test file
- Test fails (Red) indicating the sync behavior needs verification
- Test uses proper async handling for API mock
