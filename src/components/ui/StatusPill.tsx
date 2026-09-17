import React from "react";
import type { SyncStatus } from "../../lib/tauri";

const config: Record<SyncStatus, { tint: string; dot: string; label: string }> = {
  synced:   { tint: "rgba(52, 211, 153, 0.12)", dot: "var(--color-success)", label: "Synced"   },
  syncing:  { tint: "rgba(59, 107, 255, 0.12)", dot: "var(--color-info)",    label: "Syncing"  },
  conflict: { tint: "rgba(255, 92, 92, 0.12)",  dot: "var(--color-danger)",  label: "Conflict" },
  locked:   { tint: "rgba(250, 204, 21, 0.12)", dot: "var(--color-warning)", label: "Locked"   },
  pending:  { tint: "var(--color-surface-raised)", dot: "var(--color-text-muted)", label: "Pending" },
};

export function StatusPill({ status }: { status: SyncStatus }) {
  const c = config[status] ?? config.synced;
  // Syncing gets the audit's pulse read; conflicts get a steady urgent glow.
  const pulse = status === "syncing" ? "soffit-pulse 1.4s ease-in-out infinite" : undefined;
  return (
    <span style={{
      display: "inline-flex", alignItems: "center", gap: 6,
      padding: "3px 10px", borderRadius: 999,
      background: c.tint, fontSize: 12, fontWeight: 500,
      color: "var(--color-ink)",
      boxShadow: status === "conflict" ? "0 0 8px rgba(255, 92, 92, 0.35)" : undefined,
    }}>
      <span style={{ width: 6, height: 6, borderRadius: "50%", background: c.dot, flexShrink: 0, animation: pulse }} />
      {c.label}
    </span>
  );
}
