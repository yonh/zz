# Task 002: Strategy Selector Display Implementation

**depends-on**: task-001-strategy-selector-test

## Description

Fix the Strategy selector component to properly display the selected strategy value on initial render. Ensure the Select component binds correctly to the store value and displays the corresponding label.

## Execution Context

**Task Number**: 002 of 004
**Phase**: Core Features
**Prerequisites**: Task 001 test must be failing (Red state)

## BDD Scenarios

### Scenario 1: Default Strategy Display

```gherkin
Scenario: Default strategy is displayed when page loads with store defaults
  Given the application has just initialized
  And the API has not yet responded with stats
  And the store contains defaultSystemStats with strategy "failover"
  When the Overview page renders
  Then the Strategy selector should display "Failover" as the selected value
  And no placeholder or empty state should be shown
```

### Scenario 4: Loading State Handling

```gherkin
Scenario: Strategy selector shows loading state during API fetch
  Given the application is in loading state
  And the API has not yet responded
  When the Overview page renders
  Then the Strategy selector should still show the default strategy
  And the selector should not be disabled or show an error
```

**Spec Source**: `../2026-03-23-strategy-init-selection-design/bdd-specs.md` (Scenarios 1 & 4)

## Files to Modify/Create

- Modify: `ui/src/pages/Overview.tsx:220-235` (Strategy selector card)

## Steps

### Step 1: Analyze Current Implementation
- Review the Select component usage in Overview.tsx lines 220-235
- Verify the value binding: `value={systemStats.strategy}`
- Check if SelectValue needs a placeholder prop

### Step 2: Implement Fix (Green)
- Modify the Strategy selector to ensure:
  - The Select component receives the strategy value from store
  - The SelectValue component renders the correct label for the selected value
  - No empty placeholder is shown when a valid strategy exists
- Possible approaches:
  - Add explicit placeholder only for invalid/undefined strategy values
  - Ensure the strategy value matches exactly with SelectItem values
  - Add aria-label or data-testid for better test targeting

### Step 3: Verify Implementation
- Run: `cd ui && npm test -- --run Overview.strategy.test.tsx`
- Confirm test now PASSES (Green)

## Verification Commands

```bash
# Run specific test
cd ui && npm test -- --run Overview.strategy.test.tsx

# Run all Overview tests
cd ui && npm test -- --run Overview

# Manual verification
cd ui && npm run dev
# Navigate to Overview page and verify Strategy selector shows selected value
```

## Success Criteria

- Test from Task 001 now passes
- Strategy selector displays current strategy on initial render
- No placeholder shown when valid strategy exists
- No regressions in other tests
