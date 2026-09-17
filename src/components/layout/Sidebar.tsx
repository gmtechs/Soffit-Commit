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
    background: isActive ? "var(--accent-glow)" : "transparent",
    color: isActive ? "var(--color-ink)" : "var(--color-text-secondary)",
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
      {/* Brand — the gradient logo mark is the app's signature moment. */}
      <div style={{ padding: "16px 4px 12px", display: "flex", alignItems: "center", gap: collapsed ? 0 : 9, justifyContent: collapsed ? "center" : "flex-start", cursor: "pointer" }}
        onClick={() => setCollapsed(c => !c)} title={collapsed ? "Expand" : "Collapse"}>
        <AppLogo size={28} />
        {!collapsed && <span style={{ fontSize: 15, fontWeight: 700, color: "var(--color-ink)", letterSpacing: "-0.01em" }}>Soffit Commit</span>}
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

      {/* Bottom glanceable widget: device pairing is the key action here. */}
      {!collapsed && (
        <div style={{ marginTop: "auto", marginBottom: 16, background: "var(--color-bg)", borderRadius: "var(--radius-control)", padding: 12, border: "1px solid var(--color-border)" }}>
          <div style={{ display: "flex", alignItems: "center", gap: 7, marginBottom: 8 }}>
            <Users size={13} color="var(--color-primary)" />
            <span style={{ fontWeight: 600, fontSize: 12 }}>Devices</span>
          </div>
          <div style={{ height: 4, borderRadius: 2, background: "var(--color-surface-raised)", overflow: "hidden", marginBottom: 10 }}>
            <div style={{ width: "100%", height: "100%", background: "var(--accent-gradient)" }} />
          </div>
          <p style={{ fontSize: 11, color: "var(--color-text-secondary)", marginBottom: 8 }}>Pair a device to start syncing</p>
          <NavLink to="/peers" style={{ textDecoration: "none" }}>
            <button style={{ width: "100%", padding: "6px 0", borderRadius: "var(--radius-control)", background: "var(--accent-gradient)", color: "white", fontWeight: 500, fontSize: 11, border: "none", cursor: "pointer" }}>
              Manage devices
            </button>
          </NavLink>
        </div>
      )}
    </aside>
  );
}
