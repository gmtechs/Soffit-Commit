import React, { useEffect, useState } from "react";
import { dashboardStats, listActivity, getNodeId, type DashboardStats, type ActivityEntry } from "../../lib/tauri";

function SyncDot({ pct }: { pct: number }) {
  const color = pct === 100 ? "var(--color-success)" : pct === 0 ? "var(--color-danger)" : "var(--color-info)";
  const label = pct === 100 ? "Synced" : pct === 0 ? "Conflict" : "Syncing…";
  return (
    <span style={{ display: "flex", alignItems: "center", gap: 5 }}>
      <span style={{
        width: 7, height: 7, borderRadius: "50%", background: color, display: "inline-block",
        animation: pct > 0 && pct < 100 ? "pulse 1.4s ease-in-out infinite" : "none",
      }} />
      <span style={{ fontSize: 11, color: "var(--color-text-secondary)" }}>{label}</span>
    </span>
  );
}

function timeAgo(iso: string): string {
  const diff = Math.floor((Date.now() - new Date(iso).getTime()) / 1000);
  if (diff < 60)  return `${diff}s ago`;
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  return `${Math.floor(diff / 3600)}h ago`;
}

export function StatusBar() {
  const [stats, setStats] = useState<DashboardStats | null>(null);
  const [lastActivity, setLastActivity] = useState<ActivityEntry | null>(null);
  const [nodeId, setNodeId] = useState<string | null>(null);

  useEffect(() => {
    getNodeId().then(setNodeId).catch(() => {});
    const poll = async () => {
      try {
        const [s, acts] = await Promise.all([dashboardStats(), listActivity(1)]);
        setStats(s);
        if (acts.length > 0) setLastActivity(acts[0]);
      } catch { /* ignore */ }
    };
    poll();
    const id = setInterval(poll, 10000);
    return () => clearInterval(id);
  }, []);

  return (
    <>
      <style>{`@keyframes pulse { 0%,100%{opacity:1} 50%{opacity:0.4} }`}</style>
      <footer style={{
        height: 28, background: "var(--color-surface)", borderTop: "1px solid var(--color-border)",
        display: "flex", alignItems: "center", paddingInline: 20, gap: 20,
        fontSize: 11, color: "var(--color-text-secondary)", flexShrink: 0,
      }}>
        {/* Sync state */}
        <SyncDot pct={stats?.sync_health_percent ?? 100} />

        {/* Peer count */}
        <span>{stats?.online_peers ?? 0} of {stats?.total_peers ?? 0} peers online</span>

        {/* Last activity */}
        {lastActivity && (
          <span>Last activity {timeAgo(lastActivity.timestamp)}</span>
        )}

        {/* Spacer */}
        <span style={{ flex: 1 }} />

        {/* Node ID */}
        {nodeId && (
          <span style={{ color: "var(--color-text-muted)", fontFamily: "monospace", fontSize: 10 }} title={nodeId}>
            {nodeId.slice(0, 20)}…
          </span>
        )}
      </footer>
    </>
  );
}
