import React, { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";import { HardDrive, RefreshCw, FileEdit, AlertTriangle, Users } from "lucide-react";
import { Tooltip, ResponsiveContainer, PieChart, Pie, Cell, Legend } from "recharts";
import { dashboardStats, type DashboardStats } from "../lib/tauri";
import { StatCard } from "../components/ui/StatCard";
import { SyncProgressPanel } from "../components/ui/StatusBar";
import { AnimatedRing, CountUp, StatusChip } from "../components/ui/premium";

function formatBytes(b: number) {
  if (b >= 1e9) return (b / 1e9).toFixed(1) + " GB";
  if (b >= 1e6) return (b / 1e6).toFixed(1) + " MB";
  if (b >= 1e3) return (b / 1e3).toFixed(1) + " KB";
  return b + " B";
}

export function DashboardPage() {
  const [stats, setStats] = useState<DashboardStats | null>(null);
  const navigate = useNavigate();

  useEffect(() => {
    dashboardStats().then(setStats).catch(() => {});
    const id = setInterval(() => dashboardStats().then(setStats).catch(() => {}), 10000);
    return () => clearInterval(id);
  }, []);

  const s = stats;
  const pieData = s
    ? [
        { name: "Excel", value: s.file_type_breakdown.excel },
        { name: "SQL", value: s.file_type_breakdown.sql },
        { name: "Other", value: s.file_type_breakdown.other },
      ]
    : [];
  const COLORS = ["#3B6BFF", "#06B6D4", "#8B5CF6"];
  const totalFiles = pieData.reduce((total, entry) => total + entry.value, 0);

  const cardGrid: React.CSSProperties = { display: "grid", gap: 16 };

  return (
    <div className="dashboard" style={{ display: "flex", flexDirection: "column", gap: 16 }}>
      <div className="dashboard-primary" style={cardGrid}>
        <section className="dashboard-health" aria-label="Sync health">
          <p className="dashboard-eyebrow">Workspace overview</p>
          <h2 style={{ fontSize: 18, marginTop: 6 }}>Sync health</h2>
          <div style={{ position: "relative", width: 180, margin: "24px auto 12px" }}>
            <AnimatedRing pct={s?.sync_health_percent ?? 0} size={180} stroke={12} track="var(--color-border)" />
            <div style={{ position: "absolute", bottom: 8, width: "100%", textAlign: "center", fontSize: 38, fontWeight: 700, fontVariantNumeric: "tabular-nums" }}>
              {s ? <CountUp value={s.sync_health_percent} format={n => `${Math.round(n)}%`} /> : "—"}
            </div>
          </div>
          <p style={{ textAlign: "center", color: "var(--color-text-secondary)", fontSize: 12 }}>Shares fully up to date</p>
          <div style={{ textAlign: "center", marginTop: 14 }}>
            <StatusChip tone={!s ? "gray" : s.sync_health_percent >= 100 ? "green" : "blue"}>
              {!s ? "Loading overview" : s.sync_health_percent >= 100 ? "Up to date" : "Sync in progress"}
            </StatusChip>
          </div>
        </section>
        <SyncProgressPanel />
      </div>

      <div className="dashboard-kpis" style={cardGrid}>
        <StatCard icon={<HardDrive size={16} />} title="Storage used" value={s ? formatBytes(s.storage_used_bytes) : "—"} caption="Across all shares" />
        <StatCard icon={<RefreshCw size={16} />} title="Files synced" value={s ? String(s.files_synced) : "—"} caption={`of ${totalFiles} indexed files`} />
        <StatCard icon={<FileEdit size={16} />} title="Files edited this month" value={s ? String(s.files_edited_this_month) : "—"} caption="File saves since the 1st" />
        <StatCard icon={<AlertTriangle size={16} />} title="Conflicts resolved" value={s ? String(s.conflicts_resolved) : "—"} caption="All time" />
      </div>

      <div className="dashboard-secondary" style={cardGrid}>

        {/* File types donut */}
        <div style={{ background: "var(--color-surface)", borderRadius: "var(--radius-card)", border: "1px solid var(--color-border)", padding: 20 }}>
          <p style={{ fontSize: 13, color: "var(--color-text-secondary)", fontWeight: 500, marginBottom: 8 }}>File types</p>
          {totalFiles > 0 ? <ResponsiveContainer width="100%" height={130}>
            <PieChart>
              <Pie data={pieData} innerRadius={35} outerRadius={55} dataKey="value" paddingAngle={3}>
                {pieData.map((_, i) => <Cell key={i} fill={COLORS[i]} />)}
              </Pie>
              <Legend iconType="circle" iconSize={8} wrapperStyle={{ fontSize: 12 }} />
              <Tooltip contentStyle={{ fontSize: 12 }} />
            </PieChart>
          </ResponsiveContainer> : <p style={{ padding: "40px 0", color: "var(--color-text-secondary)", fontSize: 13, textAlign: "center" }}>{s ? "No indexed files yet" : "Loading file types…"}</p>}
        </div>

        {/* Connected peers */}
        <div style={{ background: "var(--color-surface)", borderRadius: "var(--radius-card)", border: "1px solid var(--color-border)", padding: 20 }}>
          <p style={{ fontSize: 13, color: "var(--color-text-secondary)", fontWeight: 500, marginBottom: 12 }}>Connected peers</p>
          <div style={{ display: "flex", alignItems: "center", gap: 12, marginBottom: 12 }}>
            <div style={{ width: 40, height: 40, borderRadius: "50%", background: "rgba(59,130,246,0.1)", display: "flex", alignItems: "center", justifyContent: "center" }}>
              <Users size={20} color="var(--color-info)" />
            </div>
            <div>
              <p style={{ fontSize: 20, fontWeight: 700 }}>{s?.online_peers ?? 0}</p>
              <p style={{ fontSize: 12, color: "var(--color-text-secondary)" }}>of {s?.total_peers ?? 0} peers online</p>
            </div>
          </div>
          <button onClick={() => navigate("/peers")} style={{ fontSize: 12, color: "var(--color-primary)", background: "none", border: "none", cursor: "pointer" }}>
            View all peers →
          </button>
        </div>
      </div>

      {/* Row 4: recent files table */}
      <RecentFilesTable />
    </div>
  );
}

function RecentFilesTable() {
  const navigate = useNavigate();
  const [shares, setShares] = useState<any[]>([]);
  const [files, setFiles] = useState<any[]>([]);

  useEffect(() => {
    import("../lib/tauri").then(m => {
      m.listShares().then(async (ss) => {
        setShares(ss);
        // Collect files from first share
        if (ss.length > 0) {
          const fs = await m.listFiles(ss[0].id);
          setFiles(fs.slice(0, 8));
        }
      }).catch(() => {});
    });
  }, []);

  return (
    <div style={{ background: "var(--color-surface)", borderRadius: "var(--radius-card)", border: "1px solid var(--color-border)", padding: 20 }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 16 }}>
        <p style={{ fontWeight: 600, fontSize: 15 }}>Shared files</p>
        <button onClick={() => navigate("/files")} style={{ fontSize: 13, color: "var(--color-primary)", background: "none", border: "none", cursor: "pointer" }}>View all →</button>
      </div>
      <div style={{ border: "1px solid var(--color-border)", borderRadius: 10, overflow: "hidden" }}>
        <table style={{ width: "100%", borderCollapse: "collapse", fontSize: 13 }}>
          <thead>
            <tr style={{ borderBottom: "1px solid var(--color-border)", background: "var(--color-bg)" }}>
              <th style={{ padding: "10px 14px", textAlign: "left", fontWeight: 500, color: "var(--color-text-secondary)" }}>File</th>
              <th style={{ padding: "10px 14px", textAlign: "left", fontWeight: 500, color: "var(--color-text-secondary)" }}>Status</th>
              <th style={{ padding: "10px 14px", textAlign: "left", fontWeight: 500, color: "var(--color-text-secondary)" }}>Modified</th>
            </tr>
          </thead>
          <tbody>
            {files.length === 0 ? (
              <tr>
                <td colSpan={3} style={{ padding: "24px 14px", textAlign: "center", color: "var(--color-text-muted)" }}>
                  {shares.length === 0
                    ? <><span>No shared files yet. </span><button onClick={() => navigate("/files")} style={{ color: "var(--color-primary)", background: "none", border: "none", cursor: "pointer" }}>Add a share →</button></>
                    : "No files indexed. Open Files and click Refresh."}
                </td>
              </tr>
            ) : files.map((f: any) => (
              <tr key={f.id} style={{ borderBottom: "1px solid var(--color-border)" }}
                onMouseEnter={e => (e.currentTarget.style.background = "var(--color-bg)")}
                onMouseLeave={e => (e.currentTarget.style.background = "transparent")}>
                <td style={{ padding: "10px 14px" }}>{f.relative_path}</td>
                <td style={{ padding: "10px 14px" }}><StatusPillInline status={f.sync_status} /></td>
                <td style={{ padding: "10px 14px", color: "var(--color-text-secondary)" }}>
                  {f.modified_at ? new Date(f.modified_at).toLocaleDateString() : "—"}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

function StatusPillInline({ status }: { status: string }) {
  const colors: Record<string, string> = { synced: "var(--color-success)", syncing: "var(--color-info)", conflict: "var(--color-danger)", locked: "var(--color-warning)", pending: "var(--color-text-muted)" };
  const color = colors[status] ?? colors.synced;
  return (
    <span style={{ display: "inline-flex", alignItems: "center", gap: 5, fontSize: 12, fontWeight: 500, color }}>
      <span style={{ width: 6, height: 6, borderRadius: "50%", background: color, display: "inline-block" }} />
      {status.charAt(0).toUpperCase() + status.slice(1)}
    </span>
  );
}
