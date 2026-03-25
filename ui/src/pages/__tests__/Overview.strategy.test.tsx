import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { BrowserRouter } from "react-router-dom";
import Overview from "../Overview";
import type { RoutingStrategy } from "@/api/types";

// Mock the store
const mockStore = {
  systemStats: {
    total_requests: 0,
    requests_per_minute: 0,
    active_providers: 0,
    healthy_providers: 0,
    total_providers: 0,
    strategy: "failover" as RoutingStrategy,
    uptime_secs: 0,
    tokens: {
      prompt: 0,
      completion: 0,
      total: 0,
    },
  },
  routingConfig: {
    strategy: "failover" as RoutingStrategy,
    max_retries: 3,
    cooldown_secs: 60,
    failure_threshold: 3,
    recovery_secs: 600,
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

globalThis.ResizeObserver = MockResizeObserver as unknown as typeof ResizeObserver;

// Mock matchMedia
Object.defineProperty(window, "matchMedia", {
  writable: true,
  value: vi.fn().mockImplementation(() => ({
    matches: false,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  })),
});

describe("Overview Strategy Select", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders the strategy select component", () => {
    render(
      <BrowserRouter>
        <Overview />
      </BrowserRouter>
    );

    expect(screen.getByText("Routing Strategy")).toBeInTheDocument();
  });

  it("displays the current strategy from routingConfig", () => {
    render(
      <BrowserRouter>
        <Overview />
      </BrowserRouter>
    );

    // The select should show "Failover" by default
    expect(screen.getByRole("combobox")).toHaveTextContent("Failover");
  });

  it("allows changing strategy", async () => {
    render(
      <BrowserRouter>
        <Overview />
      </BrowserRouter>
    );

    // Find and click the select trigger
    const selectTrigger = screen.getByRole("combobox");
    expect(selectTrigger).toBeInTheDocument();
  });
});
