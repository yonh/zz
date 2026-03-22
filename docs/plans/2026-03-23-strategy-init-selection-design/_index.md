# Strategy Initialization Selection Design

## Summary

Optimize the Strategy selector on the Overview dashboard to properly display the selected routing strategy configuration on initial render.

## Problem Statement

The Strategy selector on the Overview page currently shows an unselected/placeholder state on initial render, even though a default strategy ("failover") is defined in the store. This creates a confusing user experience where users cannot immediately see the active routing strategy.

## Proposed Solution

Ensure the Strategy selector component properly displays the current strategy value from the store, handling both:
1. **Initial render** with default store values
2. **Post-API load** with server-provided strategy

## Technical Approach

1. Verify the Select component properly binds to `systemStats.strategy`
2. Ensure the SelectValue renders the correct label for the current value
3. Handle loading states gracefully without showing empty placeholders

## Files Affected

- `ui/src/pages/Overview.tsx` - Strategy selector component
- `ui/src/stores/store.ts` - Strategy state management

## See Also

- [BDD Specifications](./bdd-specs.md) - Detailed test scenarios
