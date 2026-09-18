import React from "react";

interface StatCardProps {
  icon: React.ReactNode;
  title: string;
  value: string;
  /** Honest scope/window context, e.g. "Across all shares" or "All time". */
  caption?: string;
  children?: React.ReactNode;
}

export function StatCard({ icon, title, value, caption, children }: StatCardProps) {
  return (
    <div style={{
      background: "var(--color-surface)",
      borderRadius: "var(--radius-card)",
      border: "1px solid var(--color-border)",
      padding: "12px 14px",
    }}>
      <div style={{ display: "flex", alignItems: "center", gap: 9, marginBottom: 10 }}>
        <span style={{ width: 30, height: 30, borderRadius: 9, background: "var(--accent-glow)", color: "var(--color-primary)", display: "flex", alignItems: "center", justifyContent: "center", flexShrink: 0 }}>{icon}</span>
        <span style={{ fontSize: 12, color: "var(--color-text-secondary)", fontWeight: 500 }}>{title}</span>
      </div>
      <div style={{ fontSize: 20, fontWeight: 700, fontVariantNumeric: "tabular-nums", color: "var(--color-ink)" }}>{value}</div>
      {caption && <div style={{ fontSize: 11, color: "var(--color-text-muted)", marginTop: 4 }}>{caption}</div>}
      {children}
    </div>
  );
}
