import React, { useEffect } from "react";
import { BrowserRouter, Routes, Route, Navigate } from "react-router-dom";
import { AppLayout } from "./components/layout/AppLayout";
import { LoginPage } from "./pages/Login";
import { DashboardPage } from "./pages/Dashboard";
import { FilesPage } from "./pages/Files";
import { PeersPage } from "./pages/Peers";
import { ExcelPage } from "./pages/Excel";
import { ActivityPage } from "./pages/Activity";
import { SettingsPage } from "./pages/Settings";
import { useAuthStore } from "./store/auth";
import { ToastProvider } from "./components/ui/Toast";
import { isTauri } from "./lib/tauri";

function RequireAuth({ children }: { children: React.ReactNode }) {
  const { user } = useAuthStore();
  return user ? <>{children}</> : <Navigate to="/login" replace />;
}

export default function App() {
  const { user } = useAuthStore();

  useEffect(() => {
    if (!isTauri()) return;
    const blockContextMenu = (event: MouseEvent) => event.preventDefault();
    window.addEventListener("contextmenu", blockContextMenu);
    return () => window.removeEventListener("contextmenu", blockContextMenu);
  }, []);

  return (
    <ToastProvider>
      <BrowserRouter>
        <Routes>
          <Route path="/login" element={user ? <Navigate to="/" replace /> : <LoginPage />} />
          <Route element={<RequireAuth><AppLayout /></RequireAuth>}>
            <Route path="/" element={<DashboardPage />} />
            <Route path="/files" element={<FilesPage />} />
            <Route path="/peers" element={<PeersPage />} />
            <Route path="/excel" element={<ExcelPage />} />
            <Route path="/activity" element={<ActivityPage />} />
            <Route path="/settings" element={<SettingsPage />} />
          </Route>
          <Route path="*" element={<Navigate to="/" replace />} />
        </Routes>
      </BrowserRouter>
    </ToastProvider>
  );
}
