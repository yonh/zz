# Task 001: Strategy Selector Display Test

**depends-on**: (none)

## Description

Create a test that verifies the Strategy selector displays the current strategy value from the store on initial render, ensuring the selected option is visible without any placeholder or empty state.

## Execution Context

**Task Number**: 001 of 004
**Phase**: Foundation
**Prerequisites**: None

## BDD Scenario

```gherkin
Scenario: Default strategy is displayed when page loads with store defaults
  Given the application has just initialized
  And the API has not yet responded with stats
  And the store contains defaultSystemStats with strategy "failover"
  When the Overview page renders
  Then the Strategy selector should display "Failover" as the selected value
  And no placeholder or empty state should be shown
```

**Spec Source**: `../2026-03-23-strategy-init-selection-design/bdd-specs.md` (Scenario 1)

## Files to Modify/Create

- Create: `ui/src/pages/__tests__/Overview.strategy.test.tsx`

## Steps

### Step 1: Verify Scenario
- Ensure `Default strategy is displayed when page loads with store defaults` exists in the BDD specs

### Step 2: Implement Test (Red)
- Create test file: `ui/src/pages/__tests__/Overview.strategy.test.tsx`
- Test name: `should display current strategy on initial render`
- Test should:
  - Render the Overview component with default store state
  - Find the Strategy selector element
  - Assert that "Failover" (or the label matching strategy value) is displayed
  - Assert that no placeholder text like "Select..." is shown
- **Verification**: Run test command and verify it FAILS

### Step 3: Verify Test Failure
- Run: `cd ui && npm test -- --run Overview.strategy.test.tsx`
- Confirm test fails with appropriate assertion error

## Verification Commands

```bash
# Run specific test
cd ui && npm test -- --run Overview.strategy.test.tsx

# Run all Overview tests
cd ui && npm test -- --run Overview
```

## Success Criteria

- Test file created with proper structure
- Test fails (Red) indicating the feature is not yet implemented
- Test targets the correct behavior per BDD scenario
