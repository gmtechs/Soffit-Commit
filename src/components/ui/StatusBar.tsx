import React, { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { dashboardStats, listActivity, listShares, listFiles, getNodeId, type DashboardStats, type ActivityEntry, type FileIndex, type Share } from "../../lib/tauri";

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

interface SyncCounts {
  total: number;
  pending: number;
  syncing: number;
  synced: number;
  error: number;
}

// Polls all shares' file indexes to build a snapshot of sync progress.
// Reacts live to `app://sync-event` so counts update in real time.
function useSyncCounts(): SyncCounts {
  const [counts, setCounts] = useState<SyncCounts>({ total: 0, pending: 0, syncing: 0, synced: 0, error: 0 });

  const refresh = async () => {
    try {
      const shares: Share[] = await listShares();
      let total = 0, pending = 0, syncing = 0, synced = 0, error = 0;
      for (const sh of shares) {
        const files: FileIndex[] = await listFiles(sh.id).catch(() => []);
        total += files.length;
        for (const f of files) {
          switch (f.sync_status) {
            case "pending":               pending++; break;
            case "syncing":               syncing++; break;
            case "synced":                synced++; break;
            case "conflict":              error++; break;
            default:                      synced++;
          }
        }
      }
      setCounts({ total, pending, syncing, synced, error });
    } catch {
      // Silently ignore — no shares or Tauri unavailable
    }
  };

  useEffect(() => {
    refresh();
    const id = setInterval(refresh, 10000);
    let unlisten = () => {};
    listen("app://sync-event", () => { refresh(); }).then(fn => { unlisten = fn; }).catch(() => {});
    return () => { clearInterval(id); unlisten(); };
  }, []);

  return counts;
}

export function StatusBar() {
  const [stats, setStats] = useState<DashboardStats | null>(null);
  const [lastActivity, setLastActivity] = useState<ActivityEntry | null>(null);
  const [nodeId, setNodeId] = useState<string | null>(null);
  const counts = useSyncCounts();

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

        {/* Sync progress counts */}
        {(counts.pending > 0 || counts.syncing > 0 || counts.error > 0) && (
          <span style={{ display: "flex", alignItems: "center", gap: 6 }}>
            {counts.syncing > 0 && <span style={{ width: 6, height: 6, borderRadius: "50%", background: "var(--color-info)", animation: "pulse 1.4s ease-in-out infinite" }} />}
            <span style={{ color: counts.error > 0 ? "var(--color-danger)" : "var(--color-info)" }}>
              {counts.pending > 0 && `${counts.pending} pending`}
              {counts.pending > 0 && counts.syncing > 0 && " · "}
              {counts.syncing > 0 && `${counts.syncing} syncing`}
                            {(counts.pending > 0 || counts.syncing > 0) && counts.error > 0 && " · "}
              {counts.error > 0 && `${counts.error} error`}
            </span>
          </span>
        )}

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

// ── Detailed sync progress panel ──────────────────────────────────────────────
// Shows a table of files that are pending / syncing / in conflict, with live
// updates via `app://sync-event` and 10-second polling of `listFiles`.
interface SyncProgressEntry {
  shareId: string;
  shareName: string;
  relativePath: string;
  status: string;
  sizeBytes: number;
}

export function SyncProgressPanel() {
  const [entries, setEntries] = useState<SyncProgressEntry[]>([]);
  const [showAll, setShowAll] = useState(false);

  const loadProgress = async () => {
    try {
      const shares: Share[] = await listShares();
      const all: SyncProgressEntry[] = [];
      for (const sh of shares) {
        const files: FileIndex[] = await listFiles(sh.id).catch(() => []);
        for (const f of files) {
          if (f.sync_status !== "synced" && f.sync_status !== "locked") {
            all.push({
              shareId: sh.id,
              shareName: sh.display_name,
              relativePath: f.relative_path,
              status: f.sync_status,
              sizeBytes: f.size_bytes,
            });
          }
        }
      }
      setEntries(all);
    } catch { /* ignore */ }
  };

  useEffect(() => {
    loadProgress();
    const id = setInterval(loadProgress, 10000);
    let unlisten = () => {};
    listen("app://sync-event", () => { loadProgress(); })
      .then(fn => { unlisten = fn; })
      .catch(() => {});
    return () => { clearInterval(id); unlisten(); };
  }, []);

  const activeEntries = entries.filter(e => e.status !== "synced");
  const visible = showAll ? activeEntries : activeEntries.slice(0, 6);

  const statusColor: Record<string, string> = {
    pending:    "var(--color-warning)",
    syncing:    "var(--color-info)",
    conflict:   "var(--color-danger)",
    sync_error: "var(--color-danger)",
  };

  const formatBytes = (b: number) => {
    if (b >= 1e6) return (b / 1e6).toFixed(1) + " MB";
    if (b >= 1e3) return (b / 1e3).toFixed(1) + " KB";
    return b + " B";
  };

  return (
    <div style={{
      background: "var(--color-surface)", borderRadius: "var(--radius-card)",
      border: "1px solid var(--color-border)", padding: 20,
    }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 12 }}>
        <p style={{ fontWeight: 600, fontSize: 15 }}>Sync progress</p>
        {activeEntries.length > 6 && (
          <button onClick={() => setShowAll(v => !v)}
            style={{
              fontSize: 11, color: "var(--color-primary)",
              background: "none", border: "none", cursor: "pointer", padding: 0,
            }}>
            {showAll ? "Show less" : `Show all (${activeEntries.length})`}
          </button>
        )}
      </div>

      {activeEntries.length === 0 ? (
        <div style={{ padding: "16px 0", textAlign: "center", color: "var(--color-text-muted)", fontSize: 12 }}>
          All files synced ✓
        </div>
      ) : (
        <div style={{ border: "1px solid var(--color-border)", borderRadius: 8, overflow: "hidden" }}>
          <table style={{ width: "100%", borderCollapse: "collapse", fontSize: 12 }}>
            <thead>
              <tr style={{ borderBottom: "1px solid var(--color-border)", background: "var(--color-bg)" }}>
                <th style={{ padding: "8px 10px", textAlign: "left", fontWeight: 500, color: "var(--color-text-secondary)" }}>File</th>
                <th style={{ padding: "8px 10px", textAlign: "left", fontWeight: 500, color: "var(--color-text-secondary)" }}>Share</th>
                <th style={{ padding: "8px 10px", textAlign: "left", fontWeight: 500, color: "var(--color-text-secondary)" }}>Size</th>
                <th style={{ padding: "8px 10px", textAlign: "left", fontWeight: 500, color: "var(--color-text-secondary)" }}>Status</th>
              </tr>
            </thead>
            <tbody>
              {visible.map((e, i) => (
                <tr key={`${e.shareId}:${e.relativePath}:${i}`} style={{ borderBottom: "1px solid var(--color-border)" }}>
                  <td style={{ padding: "8px 10px", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                    {e.relativePath.split("/").pop() ?? e.relativePath}
                  </td>
                  <td style={{ padding: "8px 10px", color: "var(--color-text-secondary)" }}>{e.shareName}</td>
                  <td style={{ padding: "8px 10px", color: "var(--color-text-secondary)" }}>{formatBytes(e.sizeBytes)}</td>
                  <td style={{ padding: "8px 10px" }}>
                    <span style={{
                      display: "flex", alignItems: "center", gap: 5,
                      color: statusColor[e.status] ?? "var(--color-text-muted)",
                    }}>
                      <span style={{
                        width: 5, height: 5, borderRadius: "50%",
                        background: statusColor[e.status] ?? "var(--color-text-muted)",
                        display: "inline-block",
                      }} />
                      {e.status.charAt(0).toUpperCase() + e.status.slice(1)}
                    </span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
