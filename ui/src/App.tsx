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
import { useMockWebSocket } from "@/hooks/useMockWebSocket";

/**
 * Root application component with routing.
 */
function App() {
  const darkMode = useAppStore((s) => s.darkMode);

  useEffect(() => {
    if (darkMode) {
      document.documentElement.classList.add("dark");
    } else {
      document.documentElement.classList.remove("dark");
    }
  }, [darkMode]);

  // Start mock WebSocket simulation for live data
  useMockWebSocket();

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
