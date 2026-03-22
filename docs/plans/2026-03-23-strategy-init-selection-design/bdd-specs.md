# BDD Specifications: Strategy Initialization Selection

## Feature Overview

Optimize the Strategy selector component on the Overview page to properly display the selected configuration on initial render, ensuring users see the current routing strategy immediately when the page loads.

## User Story

As a user viewing the Overview dashboard, I want to see the currently active routing strategy displayed in the Strategy selector when the page first loads, so that I can immediately understand the current system configuration.

## BDD Scenarios

### Scenario 1: Default Strategy Display on Initial Load

```gherkin
Scenario: Default strategy is displayed when page loads with store defaults
  Given the application has just initialized
  And the API has not yet responded with stats
  And the store contains defaultSystemStats with strategy "failover"
  When the Overview page renders
  Then the Strategy selector should display "Failover" as the selected value
  And no placeholder or empty state should be shown
```

### Scenario 2: API Strategy Display After Data Load

```gherkin
Scenario: API-provided strategy is displayed after initialization
  Given the application is loading data from the API
  When the initFromApi function completes successfully
  And the API returns systemStats with strategy "round-robin"
  Then the Strategy selector should update to display "Round Robin"
  And the selection should persist across re-renders
```

### Scenario 3: Strategy Sync Between Store and UI

```gherkin
Scenario: Strategy remains synchronized between systemStats and routingConfig
  Given the user is viewing the Overview page
  And the current strategy is "failover"
  When the user changes the strategy to "weighted-random"
  Then both systemStats.strategy and routingConfig.strategy should be "weighted-random"
  And the UI should reflect "Weighted Random" in the selector
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

## Technical Context

### Current Implementation

- **Store Location**: `ui/src/stores/store.ts`
- **Component**: `ui/src/pages/Overview.tsx` (Strategy card, lines 214-240)
- **Default Strategy**: `"failover"` (defined in `defaultSystemStats`)
- **Strategy Options**: `["failover", "round-robin", "weighted-random", "quota-aware", "manual"]`

### Issue Analysis

The current implementation has a potential timing issue where:
1. The Select component's `value` prop depends on `systemStats.strategy`
2. The default value `"failover"` should match one of the option values
3. The SelectValue component should display the selected option's label

### Root Cause Hypothesis

1. **Possible timing issue**: Component may render before store initialization completes
2. **Possible value mismatch**: The strategy value format may not match exactly
3. **Possible Select component issue**: The shadcn Select may require explicit placeholder handling

## Acceptance Criteria

- [ ] Strategy selector displays the current strategy on initial page load
- [ ] No placeholder or empty state is shown when a valid strategy exists
- [ ] Strategy updates correctly when API data loads
- [ ] Strategy changes persist and sync between store slices
