import React, { useEffect, useRef, useState } from "react";
import { useNavigate, useLocation } from "react-router-dom";
import { Search, Sun, Moon, X, User, Link2, Bell, LayoutGrid, Settings, Users } from "lucide-react";
import { useAuthStore } from "../../store/auth";
import { applyTheme, useTheme } from "../../lib/theme";
import { getNodeId, setSetting, search, listPeers, listConflicts, listActivity, type SearchResult } from "../../lib/tauri";

const ROUTE_LABELS: Record<string, string> = {
  "/":          "Home",
  "/files":     "Files",
  "/peers":     "Peers",
  "/excel":     "Excel",
  "/activity":  "Activity",
  "/settings":  "Settings",
};

const NAVIGATION_RESULTS = [
  { route: "/", label: "Home", sub: "Overview", icon: LayoutGrid },
  { route: "/files", label: "Files", sub: "Browse shared files", icon: Search },
  { route: "/peers", label: "Peers", sub: "Connected devices", icon: Users },
  { route: "/settings", label: "Settings", sub: "Preferences and accounts", icon: Settings },
];

export function Topbar() {
  const { user, setUser } = useAuthStore();
  const navigate = useNavigate();
  const location = useLocation();
  const name = user?.username ?? "User";
  const initial = name.slice(0, 1).toUpperCase();

  const [nodeId, setNodeId] = useState<string | null>(null);
  const theme = useTheme();
  const [query, setQuery] = useState("");
  const [searchResults, setSearchResults] = useState<SearchResult[]>([]);
  const [peerResults, setPeerResults] = useState<{ id: string; display_name: string }[]>([]);
  const [navigationResults, setNavigationResults] = useState<typeof NAVIGATION_RESULTS>([]);
  const [showSearch, setShowSearch] = useState(false);
  const [searchFocusIdx, setSearchFocusIdx] = useState(-1);
  const [dropdownOpen, setDropdownOpen] = useState(false);
  const [notifications, setNotifications] = useState<{ id: string; type: string; message: string; path?: string }[]>([]);
  const [bellOpen, setBellOpen] = useState(false);
  const bellRef = useRef<HTMLDivElement>(null);
  const avatarCloseTimer = useRef<number | null>(null);

  const searchRef = useRef<HTMLDivElement>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);

  // Poll for real notification events (conflicts, activity)
  useEffect(() => {
    const poll = async () => {
      try {
        const [conflicts, activity] = await Promise.all([
          listConflicts(),
          listActivity(5),
        ]);
        const notifs: { id: string; type: string; message: string; path?: string }[] = [];
        conflicts.forEach(c => notifs.push({
          id: c.id, type: "conflict",
          message: `Conflict: ${c.file_path.split("/").pop()}`,
          path: c.file_path,
        }));
        activity.filter(a => a.action === "sync_error").forEach(a => notifs.push({
          id: a.id, type: "error",
          message: `Sync error: ${a.target}`,
        }));
        setNotifications(notifs);
      } catch { /* ignore */ }
    };
    poll();
    const id = setInterval(poll, 15000);
    return () => clearInterval(id);
  }, []);
  const pageTitle = ROUTE_LABELS[location.pathname] ?? "Soffit Commit";
  const isHome = location.pathname === "/";

  useEffect(() => {
    getNodeId().then(setNodeId).catch(() => {});
  }, []);

  // Close dropdowns on outside click
  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (searchRef.current && !searchRef.current.contains(e.target as Node)) {
        setShowSearch(false); setQuery(""); setSearchResults([]); setPeerResults([]); setNavigationResults([]);
      }
      if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
        setDropdownOpen(false);
      }
      if (bellRef.current && !bellRef.current.contains(e.target as Node)) {
        setBellOpen(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, []);

  // Close the account menu with Escape regardless of where focus sits
  useEffect(() => {
    if (!dropdownOpen) return;
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") setDropdownOpen(false); };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [dropdownOpen]);

  const toggleTheme = async () => {
    const next = theme === "light" ? "dark" : "light";
    applyTheme(next);
    await setSetting("theme", next).catch(() => {});
  };

  const handleSearch = async (q: string) => {
    setQuery(q);
    const term = q.trim().toLowerCase();
    if (term.length < 2) { setSearchResults([]); setPeerResults([]); setNavigationResults([]); setShowSearch(false); return; }
    setNavigationResults(NAVIGATION_RESULTS.filter(item => `${item.label} ${item.sub}`.toLowerCase().includes(term)));
    setShowSearch(true);
    try {
      const [files, peers] = await Promise.all([
        search(q.trim(), 8),
        listPeers().then(ps => ps.filter(p => p.display_name.toLowerCase().includes(q.toLowerCase()))),
      ]);
      setSearchResults(files);
      setPeerResults(peers);
      setSearchFocusIdx(-1);
    } catch { setSearchResults([]); setPeerResults([]); setSearchFocusIdx(-1); }
  };

  const allResults = [
    ...searchResults.map(r => ({ type: "file" as const, label: r.file_path.split("/").pop() ?? r.file_path, sub: r.file_path })),
    ...peerResults.map(p => ({ type: "peer" as const, label: p.display_name, sub: "Peer" })),
    ...navigationResults.map(r => ({ type: "page" as const, label: r.label, sub: r.sub, route: r.route })),
  ];

  const handleSearchKey = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown") { e.preventDefault(); setSearchFocusIdx(i => Math.min(i + 1, allResults.length - 1)); }
    else if (e.key === "ArrowUp") { e.preventDefault(); setSearchFocusIdx(i => Math.max(i - 1, -1)); }
    else if (e.key === "Enter" && searchFocusIdx >= 0) {
      const r = allResults[searchFocusIdx];
      if (r.type === "file") navigate("/files");
      else if (r.type === "peer") navigate("/peers");
      else navigate(r.route);
      setShowSearch(false); setQuery("");
    }
    else if (e.key === "Escape") { setShowSearch(false); setQuery(""); }
  };

  const signOut = () => { setUser(null); navigate("/login"); };

  return (
    <header style={{
      height: 56, background: "var(--color-surface)", borderBottom: "1px solid var(--color-border)",
      display: "flex", alignItems: "center", paddingInline: 20, gap: 14,
      position: "sticky", top: 0, zIndex: 50,
    }}>
      {/* Page title */}
      <div style={{ flex: 1 }}>
        <p style={{ fontSize: 15, fontWeight: 700, lineHeight: 1.2, color: "var(--color-ink)" }}>{pageTitle}</p>
        {isHome && (
          <p style={{ fontSize: 11, color: "var(--color-text-secondary)", marginTop: 1 }}>
            Welcome back, {name}
          </p>
        )}
      </div>

      {/* Search */}
      <div ref={searchRef} style={{ position: "relative" }}>
        <div style={{ display: "flex", alignItems: "center", gap: 7, background: "var(--color-bg)", borderRadius: 6, padding: "7px 12px", width: 220, border: "1px solid var(--color-border)" }}>
          <Search size={14} color="var(--color-text-muted)" />
          <input
            value={query}
            onChange={e => handleSearch(e.target.value)}
            onKeyDown={handleSearchKey}
            onFocus={() => allResults.length > 0 && setShowSearch(true)}
            placeholder="Search files, peers…"
            aria-label="Search"
            style={{ border: "none", background: "none", outline: "none", fontSize: 12, color: "var(--color-ink)", width: "100%" }}
          />
          {query && (
            <button onClick={() => { setQuery(""); setSearchResults([]); setPeerResults([]); setNavigationResults([]); setShowSearch(false); }}
              style={{ background: "none", border: "none", cursor: "pointer", padding: 0, color: "var(--color-text-muted)" }}>
              <X size={12} />
            </button>
          )}
        </div>

        {showSearch && (
          <div style={{ position: "absolute", top: "calc(100% + 4px)", left: 0, right: 0, background: "var(--color-surface)", borderRadius: 6, border: "1px solid var(--color-border)", boxShadow: "0 4px 16px rgba(0,0,0,0.08)", zIndex: 200, overflow: "hidden", minWidth: 260 }}>
            {allResults.length === 0 ? (
              <p style={{ padding: "12px 14px", fontSize: 12, color: "var(--color-text-muted)" }}>No results for "{query}"</p>
            ) : (
              <>
                {searchResults.length > 0 && (
                  <>
                    <p style={{ padding: "8px 12px 4px", fontSize: 10, fontWeight: 600, color: "var(--color-text-muted)", textTransform: "uppercase", letterSpacing: "0.06em" }}>Files</p>
                    {searchResults.map((r, i) => (
                      <button key={i} onClick={() => { navigate("/files"); setShowSearch(false); setQuery(""); }}
                        style={{ display: "block", width: "100%", textAlign: "left", padding: "8px 12px", background: searchFocusIdx === i ? "var(--color-bg)" : "none", border: "none", cursor: "pointer", fontSize: 12 }}>
                        <p style={{ fontWeight: 500, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{r.file_path.split("/").pop()}</p>
                        <p style={{ fontSize: 10, color: "var(--color-text-muted)", marginTop: 1 }}>{r.file_path}</p>
                      </button>
                    ))}
                  </>
                )}
                {peerResults.length > 0 && (
                  <>
                    <p style={{ padding: "8px 12px 4px", fontSize: 10, fontWeight: 600, color: "var(--color-text-muted)", textTransform: "uppercase", letterSpacing: "0.06em", borderTop: searchResults.length > 0 ? "1px solid var(--color-border)" : "none" }}>Peers</p>
                    {peerResults.map((p, i) => (
                      <button key={p.id} onClick={() => { navigate("/peers"); setShowSearch(false); setQuery(""); }}
                        style={{ display: "block", width: "100%", textAlign: "left", padding: "8px 12px", background: searchFocusIdx === searchResults.length + i ? "var(--color-bg)" : "none", border: "none", cursor: "pointer", fontSize: 12, fontWeight: 500 }}>
                        {p.display_name}
                      </button>
                    ))}
                  </>
                )}
                {navigationResults.length > 0 && (
                  <>
                    <p style={{ padding: "8px 12px 4px", fontSize: 10, fontWeight: 600, color: "var(--color-text-muted)", textTransform: "uppercase", letterSpacing: "0.06em", borderTop: searchResults.length + peerResults.length > 0 ? "1px solid var(--color-border)" : "none" }}>Navigate</p>
                    {navigationResults.map((item) => {
                      const Icon = item.icon;
                      return <button key={item.route} onClick={() => { navigate(item.route); setShowSearch(false); setQuery(""); }}
                        style={{ display: "flex", alignItems: "center", gap: 8, width: "100%", textAlign: "left", padding: "8px 12px", background: "none", border: "none", cursor: "pointer", fontSize: 12, color: "var(--color-ink)" }}>
                        <Icon size={14} color="var(--color-text-muted)" /><span><strong style={{ fontWeight: 600 }}>{item.label}</strong><span style={{ color: "var(--color-text-muted)", marginLeft: 6 }}>{item.sub}</span></span>
                      </button>;
                    })}
                  </>
                )}
              </>
            )}
          </div>
        )}
      </div>

      {/* Theme toggle */}
      <button onClick={toggleTheme} aria-label="Toggle theme"
        style={{ width: 32, height: 32, borderRadius: 6, background: "var(--color-bg)", border: "1px solid var(--color-border)", display: "flex", alignItems: "center", justifyContent: "center", cursor: "pointer" }}>
        {theme === "light" ? <Moon size={15} color="var(--color-text-secondary)" /> : <Sun size={15} color="var(--color-text-secondary)" />}
      </button>

      {/* Notification bell — wired to real conflicts/errors */}
      <div ref={bellRef} style={{ position: "relative" }}>
        <button onClick={() => setBellOpen(o => !o)} aria-label="Notifications"
          style={{ width: 32, height: 32, borderRadius: 6, background: "var(--color-bg)", border: "1px solid var(--color-border)", display: "flex", alignItems: "center", justifyContent: "center", cursor: "pointer", position: "relative" }}>
          <Bell size={15} color="var(--color-text-secondary)" />
          {notifications.length > 0 && (
            <span style={{ position: "absolute", top: 6, right: 6, width: 7, height: 7, borderRadius: "50%", background: "var(--color-danger)", border: "2px solid var(--color-surface)" }} />
          )}
        </button>
        {bellOpen && (
          <div style={{ position: "absolute", top: "calc(100% + 6px)", right: 0, width: 280, background: "var(--color-surface)", border: "1px solid var(--color-border)", borderRadius: 6, boxShadow: "0 4px 16px rgba(0,0,0,0.08)", zIndex: 300, overflow: "hidden" }}>
            <p style={{ padding: "10px 14px 6px", fontSize: 11, fontWeight: 700, color: "var(--color-text-muted)", textTransform: "uppercase", letterSpacing: "0.06em" }}>Notifications</p>
            {notifications.length === 0 ? (
              <p style={{ padding: "8px 14px 12px", fontSize: 12, color: "var(--color-text-muted)" }}>No pending events</p>
            ) : notifications.map(n => (
              <button key={n.id} onClick={() => { if (n.type === "conflict") navigate("/files"); setBellOpen(false); }}
                style={{ display: "block", width: "100%", textAlign: "left", padding: "9px 14px", background: "none", border: "none", borderTop: "1px solid var(--color-border)", cursor: "pointer", fontSize: 12, color: n.type === "conflict" ? "var(--color-danger)" : "var(--color-ink)" }}>
                <span style={{ fontWeight: 600 }}>{n.type === "conflict" ? "⚠ " : "✕ "}</span>{n.message}
              </button>
            ))}
          </div>
        )}
      </div>

      {/* iroh status */}
      <button aria-label="Network" title={nodeId ? `Connected: ${nodeId}` : "Connecting…"}
        style={{ width: 32, height: 32, borderRadius: 6, background: nodeId ? "rgba(22,163,74,0.08)" : "var(--color-bg)", border: "1px solid var(--color-border)", display: "flex", alignItems: "center", justifyContent: "center", cursor: "pointer" }}>
        <Link2 size={15} color={nodeId ? "var(--color-success)" : "var(--color-text-muted)"} />
      </button>

      {/* Avatar + dropdown */}
      <div ref={dropdownRef} style={{ position: "relative" }}
        onMouseEnter={() => { if (avatarCloseTimer.current) { window.clearTimeout(avatarCloseTimer.current); avatarCloseTimer.current = null; } setDropdownOpen(true); }}
        onMouseLeave={() => { avatarCloseTimer.current = window.setTimeout(() => { avatarCloseTimer.current = null; setDropdownOpen(false); }, 150); }}>
        <button onClick={() => setDropdownOpen(o => !o)}
          aria-label={`Account menu for ${name}`} aria-haspopup="menu" aria-expanded={dropdownOpen} title={name}
          style={{ width: 30, height: 30, borderRadius: "50%", background: "var(--color-primary)", border: "none", display: "flex", alignItems: "center", justifyContent: "center", color: "white", fontWeight: 700, fontSize: 12, cursor: "pointer", padding: 0 }}>
          {initial}
        </button>

        {dropdownOpen && (
          <div style={{ position: "absolute", top: "calc(100% + 6px)", right: 0, width: 220, background: "var(--color-surface)", border: "1px solid var(--color-border)", borderRadius: 6, boxShadow: "0 4px 16px rgba(0,0,0,0.08)", zIndex: 300, overflow: "hidden" }}>
            {/* Profile info */}
            <div style={{ padding: "12px 14px", borderBottom: "1px solid var(--color-border)" }}>
              <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 4 }}>
                <div style={{ width: 28, height: 28, borderRadius: "50%", background: "var(--color-primary)", display: "flex", alignItems: "center", justifyContent: "center", color: "white", fontWeight: 700, fontSize: 11 }}>{initial}</div>
                <div>
                  <p style={{ fontSize: 13, fontWeight: 600 }}>{name}</p>
                  <p style={{ fontSize: 11, color: "var(--color-success)", display: "flex", alignItems: "center", gap: 4 }}>
                    <span style={{ width: 6, height: 6, borderRadius: "50%", background: "var(--color-success)", display: "inline-block" }} /> Online
                  </p>
                </div>
              </div>
              {nodeId && (
                <p style={{ fontSize: 10, color: "var(--color-text-muted)", fontFamily: "monospace", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }} title={nodeId}>
                  {nodeId.slice(0, 24)}…
                </p>
              )}
            </div>
            {/* Theme toggle row */}
            <button onClick={toggleTheme}
              style={{ display: "flex", alignItems: "center", justifyContent: "space-between", width: "100%", padding: "10px 14px", background: "none", border: "none", cursor: "pointer", fontSize: 13, color: "var(--color-ink)", borderBottom: "1px solid var(--color-border)" }}>
              <span>{theme === "light" ? "Dark mode" : "Light mode"}</span>
              {theme === "light" ? <Moon size={14} /> : <Sun size={14} />}
            </button>
            {/* Settings link */}
            <button onClick={() => { navigate("/settings"); setDropdownOpen(false); }}
              style={{ display: "flex", alignItems: "center", gap: 8, width: "100%", padding: "10px 14px", background: "none", border: "none", cursor: "pointer", fontSize: 13, color: "var(--color-ink)", borderBottom: "1px solid var(--color-border)" }}>
              <User size={13} /> Network identity
            </button>
            {/* Sign out */}
            <button onClick={signOut}
              style={{ display: "flex", alignItems: "center", gap: 8, width: "100%", padding: "10px 14px", background: "none", border: "none", cursor: "pointer", fontSize: 13, color: "var(--color-danger)", fontWeight: 500 }}>
              Sign out
            </button>
          </div>
        )}
      </div>
    </header>
  );
}
