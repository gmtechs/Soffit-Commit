import React, { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";import { HardDrive, RefreshCw, FileEdit, AlertTriangle, Users } from "lucide-react";
import { AreaChart, Area, XAxis, YAxis, Tooltip, ResponsiveContainer, PieChart, Pie, Cell, Legend } from "recharts";
import { dashboardStats, type DashboardStats } from "../lib/tauri";
import { StatCard } from "../components/ui/StatCard";

function formatBytes(b: number) {
  if (b >= 1e9) return (b / 1e9).toFixed(1) + " GB";
  if (b >= 1e6) return (b / 1e6).toFixed(1) + " MB";
  if (b >= 1e3) return (b / 1e3).toFixed(1) + " KB";
  return b + " B";
}

// Placeholder sync activity data
const syncData = [
  { month: "Jan", bytes: 120 }, { month: "Feb", bytes: 340 }, { month: "Mar", bytes: 200 },
  { month: "Apr", bytes: 680 }, { month: "May", bytes: 420 }, { month: "Jun", bytes: 860 },
];

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
        { name: "Excel", value: s.file_type_breakdown.excel || 1 },
        { name: "SQL", value: s.file_type_breakdown.sql || 1 },
        { name: "Other", value: s.file_type_breakdown.other || 1 },
      ]
    : [];
  const COLORS = ["#4F86C6", "var(--color-success)", "#8B8B8B"];

  const cardGrid: React.CSSProperties = { display: "grid", gap: 20 };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 20 }}>
      {/* Row 1: stat cards */}
      <div style={{ ...cardGrid, gridTemplateColumns: "1fr 1fr 2fr" }}>
        <StatCard icon={<HardDrive size={18} />} title="Storage used" value={s ? formatBytes(s.storage_used_bytes) : "—"} delta={s?.storage_delta_pct ?? 0} detailsLink={() => navigate("/files")} />
        <StatCard icon={<RefreshCw size={18} />} title="Files synced" value={s ? String(s.files_synced) : "—"} delta={s?.files_synced_delta_pct ?? 0} detailsLink={() => navigate("/files")} />
        <div style={{ background: "var(--color-surface)", borderRadius: "var(--radius-card)", border: "1px solid var(--color-border)", padding: 20 }}>
          <p style={{ fontSize: 13, color: "var(--color-text-secondary)", fontWeight: 500, marginBottom: 12 }}>Sync activity (last 6 months)</p>
          <ResponsiveContainer width="100%" height={100}>
            <AreaChart data={syncData}>
              <defs>
                <linearGradient id="grad" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="5%" stopColor="var(--color-info)" stopOpacity={0.3} />
                  <stop offset="95%" stopColor="var(--color-info)" stopOpacity={0} />
                </linearGradient>
              </defs>
              <XAxis dataKey="month" tick={{ fontSize: 10, fill: "var(--color-text-muted)" }} axisLine={false} tickLine={false} />
              <YAxis hide />
              <Tooltip contentStyle={{ fontSize: 12 }} />
              <Area type="monotone" dataKey="bytes" stroke="var(--color-info)" fill="url(#grad)" strokeWidth={2} />
            </AreaChart>
          </ResponsiveContainer>
        </div>
      </div>

      {/* Row 2: more stats */}
      <div style={{ ...cardGrid, gridTemplateColumns: "1fr 1fr" }}>
        <StatCard icon={<FileEdit size={18} />} title="Files edited this month" value={s ? String(s.files_edited_this_month) : "—"} delta={s?.files_edited_delta_pct ?? 0} />
        <StatCard icon={<AlertTriangle size={18} />} title="Conflicts resolved" value={s ? String(s.conflicts_resolved) : "—"} delta={s?.conflicts_delta_pct ?? 0} />
      </div>

      {/* Row 3: health, file types, peers */}
      <div style={{ ...cardGrid, gridTemplateColumns: "1fr 1fr 1fr" }}>
        {/* Sync health gauge */}
        <div style={{ background: "var(--color-surface)", borderRadius: "var(--radius-card)", border: "1px solid var(--color-border)", padding: 20, display: "flex", flexDirection: "column", alignItems: "center" }}>
          <p style={{ fontSize: 13, color: "var(--color-text-secondary)", fontWeight: 500, marginBottom: 12, alignSelf: "flex-start" }}>Sync health</p>
          <div style={{ position: "relative", width: 120, height: 60, marginBottom: 8 }}>
            <svg viewBox="0 0 120 60" width="120" height="60">
              <path d="M10,60 A50,50 0 0,1 110,60" fill="none" stroke="var(--color-border)" strokeWidth="10" strokeLinecap="round" />
              {s && <path d="M10,60 A50,50 0 0,1 110,60" fill="none" stroke="var(--color-success)" strokeWidth="10" strokeLinecap="round"
                strokeDasharray={`${(s.sync_health_percent / 100) * 157} 157`} />}
            </svg>
            <div style={{ position: "absolute", bottom: 0, width: "100%", textAlign: "center", fontSize: 20, fontWeight: 700, color: "var(--color-ink)" }}>
              {s ? Math.round(s.sync_health_percent) : 0}%
            </div>
          </div>
          <p style={{ fontSize: 12, color: "var(--color-text-secondary)" }}>Shares fully up to date</p>
        </div>

        {/* File types donut */}
        <div style={{ background: "var(--color-surface)", borderRadius: "var(--radius-card)", border: "1px solid var(--color-border)", padding: 20 }}>
          <p style={{ fontSize: 13, color: "var(--color-text-secondary)", fontWeight: 500, marginBottom: 8 }}>File types</p>
          <ResponsiveContainer width="100%" height={130}>
            <PieChart>
              <Pie data={pieData} innerRadius={35} outerRadius={55} dataKey="value" paddingAngle={3}>
                {pieData.map((_, i) => <Cell key={i} fill={COLORS[i]} />)}
              </Pie>
              <Legend iconType="circle" iconSize={8} wrapperStyle={{ fontSize: 12 }} />
              <Tooltip contentStyle={{ fontSize: 12 }} />
            </PieChart>
          </ResponsiveContainer>
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
