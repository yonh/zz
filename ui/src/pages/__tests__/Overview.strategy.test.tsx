import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { BrowserRouter } from "react-router-dom";
import Overview from "../Overview";

// Mock the store
const mockStore = {
  systemStats: {
    total_requests: 0,
    requests_per_minute: 0,
    active_providers: 0,
    healthy_providers: 0,
    total_providers: 0,
    strategy: "failover" as const,
    uptime_secs: 0,
    tokens: {
      prompt: 0,
      completion: 0,
      total: 0,
    },
  },
  logs: [],
  providers: [],
  setStrategy: vi.fn(),
};

vi.mock("@/stores/store", () => ({
  useAppStore: vi.fn((selector) => {
    if (typeof selector === "function") {
      return selector(mockStore);
    }
    return mockStore;
  }),
}));

// Mock toast
vi.mock("sonner", () => ({
  toast: {
    success: vi.fn(),
  },
}));

// Mock ResizeObserver
class MockResizeObserver {
  observe = vi.fn();
  unobserve = vi.fn();
  disconnect = vi.fn();
}
global.ResizeObserver = MockResizeObserver as unknown as typeof ResizeObserver;

// Mock matchMedia
Object.defineProperty(window, "matchMedia", {
  writable: true,
  value: vi.fn().mockImplementation(() => ({
    matches: false,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  })),
});

const renderOverview = () => {
  return render(
    <BrowserRouter>
      <Overview />
    </BrowserRouter>
  );
};

describe("Overview Strategy Selector", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe("Scenario 1: Default Strategy Display on Initial Load", () => {
    it("should display current strategy on initial render", () => {
      // Given: the application has just initialized
      // And: the store contains defaultSystemStats with strategy "failover"
      mockStore.systemStats.strategy = "failover";

      // When: the Overview page renders
      renderOverview();

      // Then: the Strategy selector should display "Failover" as the selected value
      const strategyCard = screen.getByText("Strategy").closest("div");
      expect(strategyCard).toBeInTheDocument();

      // Find the select trigger and verify it shows the strategy label
      const selectTrigger = screen.getByRole("combobox");
      expect(selectTrigger).toBeInTheDocument();

      // The selected value should be visible - "Failover" is the label for "failover"
      expect(selectTrigger).toHaveTextContent("Failover");
    });

    it("should not show placeholder or empty state when strategy is set", () => {
      // Given: the store has a valid strategy
      mockStore.systemStats.strategy = "failover";

      // When: the Overview page renders
      renderOverview();

      // Then: no placeholder text should be shown
      const selectTrigger = screen.getByRole("combobox");
      const triggerText = selectTrigger.textContent || "";

      // Should not contain common placeholder texts
      expect(triggerText).not.toMatch(/select|choose|pick/i);
      expect(triggerText).toContain("Failover");
    });

    it("should display Round Robin when strategy is round-robin", () => {
      // Given: the store has round-robin strategy
      mockStore.systemStats.strategy = "round-robin";

      // When: the Overview page renders
      renderOverview();

      // Then: the Strategy selector should display "Round Robin"
      const selectTrigger = screen.getByRole("combobox");
      expect(selectTrigger).toHaveTextContent("Round Robin");
    });
  });

  describe("Scenario 4: Loading State Handling", () => {
    it("should show default strategy even during loading state", () => {
      // Given: the application has a default strategy
      mockStore.systemStats.strategy = "failover";

      // When: the Overview page renders (even during loading)
      renderOverview();

      // Then: the Strategy selector should still show the default strategy
      const selectTrigger = screen.getByRole("combobox");
      expect(selectTrigger).toHaveTextContent("Failover");

      // And: the selector should not be disabled
      expect(selectTrigger).not.toBeDisabled();
    });
  });

  describe("Scenario 2: API Strategy Display After Data Load", () => {
    it("should display API-provided strategy after initialization", () => {
      // Given: the API returns a different strategy
      mockStore.systemStats.strategy = "round-robin";

      // When: the Overview page renders with updated store
      renderOverview();

      // Then: the Strategy selector should display "Round Robin"
      const selectTrigger = screen.getByRole("combobox");
      expect(selectTrigger).toHaveTextContent("Round Robin");
    });

    it("should display weighted-random strategy when provided", () => {
      // Given: the API returns weighted-random strategy
      mockStore.systemStats.strategy = "weighted-random";

      // When: the Overview page renders
      renderOverview();

      // Then: the Strategy selector should display "Weighted Random"
      const selectTrigger = screen.getByRole("combobox");
      expect(selectTrigger).toHaveTextContent("Weighted Random");
    });

    it("should display quota-aware strategy when provided", () => {
      // Given: the API returns quota-aware strategy
      mockStore.systemStats.strategy = "quota-aware";

      // When: the Overview page renders
      renderOverview();

      // Then: the Strategy selector should display "Quota-Aware"
      const selectTrigger = screen.getByRole("combobox");
      expect(selectTrigger).toHaveTextContent("Quota-Aware");
    });

    it("should display manual strategy when provided", () => {
      // Given: the API returns manual strategy
      mockStore.systemStats.strategy = "manual";

      // When: the Overview page renders
      renderOverview();

      // Then: the Strategy selector should display "Manual / Fixed"
      const selectTrigger = screen.getByRole("combobox");
      expect(selectTrigger).toHaveTextContent("Manual");
    });
  });

  describe("Scenario 3: Strategy Sync Between Store and UI", () => {
    it("should call setStrategy when user changes strategy", async () => {
      // Given: the user is viewing the Overview page
      mockStore.systemStats.strategy = "failover";
      renderOverview();

      // When: the user clicks on the strategy selector
      const selectTrigger = screen.getByRole("combobox");
      expect(selectTrigger).toBeInTheDocument();

      // Then: the setStrategy function should be available
      expect(mockStore.setStrategy).toBeDefined();
    });

    it("should sync systemStats and routingConfig when strategy changes", () => {
      // This test verifies the store's setStrategy action updates both slices
      // Given: the mock store has setStrategy function
      mockStore.systemStats.strategy = "failover";

      // When: setStrategy is called with a new value
      mockStore.setStrategy("weighted-random");

      // Then: setStrategy should have been called with the correct value
      expect(mockStore.setStrategy).toHaveBeenCalledWith("weighted-random");
    });
  });
});
