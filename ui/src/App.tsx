import { useEffect } from "react";
import { BrowserRouter, Routes, Route } from "react-router-dom";
import { Toaster } from "sonner";
import Layout from "@/components/layout/Layout";
import Overview from "@/pages/Overview";
import Providers from "@/pages/Providers";
import Routing from "@/pages/Routing";
import Logs from "@/pages/Logs";
import Config from "@/pages/Config";
import { useAppStore } from "@/stores/store";
import { useWebSocket } from "@/hooks/useWebSocket";

/**
 * Root application component with routing.
 */
function App() {
  const darkMode = useAppStore((s) => s.darkMode);
  const loading = useAppStore((s) => s.loading);
  const error = useAppStore((s) => s.error);
  const initFromApi = useAppStore((s) => s.initFromApi);

  useEffect(() => {
    if (darkMode) {
      document.documentElement.classList.add("dark");
    } else {
      document.documentElement.classList.remove("dark");
    }
  }, [darkMode]);

  // Initialize data from API on mount
  useEffect(() => {
    initFromApi();
  }, [initFromApi]);

  // Start WebSocket connection for live data
  useWebSocket();

  // Loading state while fetching data from backend
  if (loading) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-background text-foreground">
        <div className="flex flex-col items-center gap-4">
          <div className="h-8 w-8 animate-spin rounded-full border-4 border-muted border-t-primary" />
          <p className="text-sm text-muted-foreground">Connecting to backend...</p>
        </div>
      </div>
    );
  }

  // Error state when backend is unreachable
  if (error) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-background text-foreground">
        <div className="flex flex-col items-center gap-4 max-w-md text-center">
          <div className="rounded-full bg-destructive/10 p-3">
            <svg className="h-6 w-6 text-destructive" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M12 9v2m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
            </svg>
          </div>
          <h2 className="text-lg font-semibold">Unable to connect to backend</h2>
          <p className="text-sm text-muted-foreground">{error}</p>
          <button
            onClick={() => initFromApi()}
            className="rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 transition-colors"
          >
            Retry
          </button>
        </div>
      </div>
    );
  }

  return (
    <BrowserRouter>
      <Toaster
        position="bottom-right"
        theme={darkMode ? "dark" : "light"}
        richColors
        closeButton
      />
      <Routes>
        <Route element={<Layout />}>
          <Route path="/" element={<Overview />} />
          <Route path="/providers" element={<Providers />} />
          <Route path="/routing" element={<Routing />} />
          <Route path="/logs" element={<Logs />} />
          <Route path="/config" element={<Config />} />
        </Route>
      </Routes>
    </BrowserRouter>
  );
}

export default App
