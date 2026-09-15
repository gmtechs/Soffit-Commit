import React, { useEffect, useState } from "react";
import { NavLink, useNavigate } from "react-router-dom";
import {
  LayoutGrid, Folder, Users, Table2, Activity,
  Settings,
} from "lucide-react";
import { AppLogo } from "../ui/AppLogo";
import { useAuthStore } from "../../store/auth";

const navItems = [
  { to: "/",         icon: <LayoutGrid size={18} />, label: "Home"     },
  { to: "/files",    icon: <Folder size={18} />,     label: "Files"    },
  { to: "/peers",    icon: <Users size={18} />,      label: "Peers"    },
  { to: "/excel",    icon: <Table2 size={18} />,     label: "Excel"    },
  { to: "/activity", icon: <Activity size={18} />,   label: "Activity" },
];

const secondaryItems = [
  { to: "/settings", icon: <Settings size={18} />,  label: "Settings"    },
];

export function Sidebar() {
  const navigate = useNavigate();
  const [collapsed, setCollapsed] = useState(false);

  useEffect(() => {
    const mq = window.matchMedia("(max-width: 1100px)");
    const handler = (e: MediaQueryListEvent | MediaQueryList) => setCollapsed(e.matches);
    handler(mq);
    mq.addEventListener("change", handler as (e: MediaQueryListEvent) => void);
    return () => mq.removeEventListener("change", handler as (e: MediaQueryListEvent) => void);
  }, []);

  const W = collapsed ? 64 : 240;

  // Active state: 2px left-border accent + faint tint, ink text (no orange fill)
  const navStyle = (isActive: boolean): React.CSSProperties => ({
    display: "flex",
    alignItems: "center",
    gap: collapsed ? 0 : 10,
    justifyContent: collapsed ? "center" : "flex-start",
    padding: collapsed ? "9px 0" : "9px 12px",
    borderRadius: 6,
    textDecoration: "none",
    fontSize: 13,
    fontWeight: isActive ? 600 : 500,
    background: isActive ? "rgba(226,113,0,0.08)" : "transparent",
    color: "var(--color-ink)",
    borderLeft: isActive ? "2px solid var(--color-primary)" : "2px solid transparent",
    transition: "all 0.12s",
    whiteSpace: "nowrap" as const,
  });

  const label = (text: string) => collapsed ? null : <span>{text}</span>;
  const sectionLabel = (text: string) => collapsed ? null : (
    <p style={{ fontSize: 10, fontWeight: 600, color: "var(--color-text-muted)", letterSpacing: "0.07em", textTransform: "uppercase", padding: "10px 12px 4px" }}>
      {text}
    </p>
  );

  return (
    <aside style={{
      width: W, minWidth: W, height: "100vh",
      background: "var(--color-surface)",
      borderRight: "1px solid var(--color-border)",
      display: "flex", flexDirection: "column",
      padding: `0 ${collapsed ? 8 : 10}px`,
      position: "fixed", top: 0, left: 0, zIndex: 100,
      transition: "width 0.2s, min-width 0.2s",
      overflow: "hidden",
    }}>
      {/* Brand */}
      <div style={{ padding: "16px 4px 12px", display: "flex", alignItems: "center", gap: collapsed ? 0 : 8, justifyContent: collapsed ? "center" : "flex-start", cursor: "pointer" }}
        onClick={() => setCollapsed(c => !c)} title={collapsed ? "Expand" : "Collapse"}>
        <div style={{ width: 28, height: 28, minWidth: 28, borderRadius: 6, background: "var(--color-ink)", display: "flex", alignItems: "center", justifyContent: "center", color: "white" }}>
          <AppLogo size={20} />
        </div>
        {!collapsed && <span style={{ fontSize: 15, fontWeight: 700, color: "var(--color-ink)" }}>Soffit Commit</span>}
      </div>

      {/* Main nav */}
      <div style={{ marginBottom: 4 }}>
        {sectionLabel("Main")}
        <nav style={{ display: "flex", flexDirection: "column", gap: 1 }}>
          {navItems.map(({ to, icon, label: lbl }) => (
            <NavLink key={to} to={to} end={to === "/"} style={({ isActive }) => navStyle(isActive)} title={collapsed ? lbl : undefined}>
              {icon}{label(lbl)}
            </NavLink>
          ))}
        </nav>
      </div>

      {/* Secondary nav */}
      <div style={{ marginTop: 8 }}>
        {sectionLabel("General")}
        <nav style={{ display: "flex", flexDirection: "column", gap: 1 }}>
          {secondaryItems.map(({ to, icon, label: lbl }) => (
            <NavLink key={to} to={to} style={({ isActive }) => navStyle(isActive)} title={collapsed ? lbl : undefined}>
              {icon}{label(lbl)}
            </NavLink>
          ))}
        </nav>
      </div>

      {/* Bottom storage CTA */}
      {!collapsed && (
        <div style={{ marginTop: "auto", marginBottom: 16, background: "var(--color-bg)", borderRadius: 6, padding: 12, border: "1px solid var(--color-border)" }}>
          <div style={{ display: "flex", alignItems: "center", gap: 7, marginBottom: 6 }}>
            <AppLogo size={13} />
            <span style={{ fontWeight: 600, fontSize: 12 }}>Storage</span>
          </div>
          <p style={{ fontSize: 11, color: "var(--color-text-secondary)", marginBottom: 8 }}>Manage connected devices</p>
          <NavLink to="/peers" style={{ textDecoration: "none" }}>
            <button style={{ width: "100%", padding: "6px 0", borderRadius: 6, background: "var(--color-ink)", color: "white", fontWeight: 500, fontSize: 11, border: "none", cursor: "pointer" }}>
              Manage devices
            </button>
          </NavLink>
        </div>
      )}
    </aside>
  );
}
