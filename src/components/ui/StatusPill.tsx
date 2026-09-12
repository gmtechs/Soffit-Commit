import React from "react";
import type { SyncStatus } from "../../lib/tauri";

const config: Record<SyncStatus, { bg: string; dot: string; label: string }> = {
  synced:   { bg: "#F0FDF4", dot: "var(--color-success)", label: "Synced"   },
  syncing:  { bg: "#EFF6FF", dot: "var(--color-info)",    label: "Syncing"  },
  conflict: { bg: "#FEF2F2", dot: "var(--color-danger)",  label: "Conflict" },
  locked:   { bg: "#FFFBEB", dot: "var(--color-warning)", label: "Locked"   },
  pending:  { bg: "#F9FAFB", dot: "#9CA3AF",              label: "Pending"  },
};

export function StatusPill({ status }: { status: SyncStatus }) {
  const c = config[status] ?? config.synced;
  return (
    <span style={{
      display: "inline-flex", alignItems: "center", gap: 6,
      padding: "3px 10px", borderRadius: 999,
      background: c.bg, fontSize: 12, fontWeight: 500,
      color: "var(--color-ink)",
    }}>
      <span style={{ width: 6, height: 6, borderRadius: "50%", background: c.dot, flexShrink: 0 }} />
      {c.label}
    </span>
  );
}
