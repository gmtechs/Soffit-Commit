import React, { useEffect, useState } from "react";
import { Outlet } from "react-router-dom";
import { Sidebar } from "./Sidebar";
import { Topbar } from "./Topbar";
import { ConflictBanner } from "../ui/ConflictBanner";
import { StatusBar } from "../ui/StatusBar";

export function AppLayout() {
  const [sidebarWidth, setSidebarWidth] = useState(240);

  useEffect(() => {
    const mq = window.matchMedia("(max-width: 1100px)");
    const handler = (e: MediaQueryListEvent | MediaQueryList) =>
      setSidebarWidth(e.matches ? 64 : 240);
    handler(mq);
    mq.addEventListener("change", handler as (e: MediaQueryListEvent) => void);
    return () => mq.removeEventListener("change", handler as (e: MediaQueryListEvent) => void);
  }, []);

  return (
    <div style={{ display: "flex", height: "100vh", overflow: "hidden" }}>
      <Sidebar />
      <div style={{
        marginLeft: sidebarWidth, flex: 1,
        display: "flex", flexDirection: "column", overflow: "hidden",
        transition: "margin-left 0.2s",
      }}>
        <Topbar />
        <main style={{ flex: 1, overflowY: "auto", padding: 20, background: "var(--color-bg)" }}>
          <ConflictBanner />
          <Outlet />
        </main>
        <StatusBar />
      </div>
    </div>
  );
}
