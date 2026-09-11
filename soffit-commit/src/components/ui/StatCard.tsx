import React from "react";
import { TrendingUp, TrendingDown, ArrowRight } from "lucide-react";

interface StatCardProps {
  icon: React.ReactNode;
  title: string;
  value: string;
  delta?: number;
  detailsLink?: () => void;
  children?: React.ReactNode;
}

export function StatCard({ icon, title, value, delta, detailsLink, children }: StatCardProps) {
  return (
    <div style={{
      background: "var(--color-surface)",
      borderRadius: "var(--radius-card)",
      border: "1px solid var(--color-border)",
      padding: "12px 14px",
    }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 6 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 7 }}>
          <span style={{ color: "var(--color-text-muted)" }}>{icon}</span>
          <span style={{ fontSize: 12, color: "var(--color-text-secondary)", fontWeight: 500 }}>{title}</span>
        </div>
        {detailsLink && (
          <button onClick={detailsLink} style={{ fontSize: 11, color: "var(--color-primary)", display: "flex", alignItems: "center", gap: 3, background: "none", border: "none", cursor: "pointer" }}>
            Details <ArrowRight size={10} />
          </button>
        )}
      </div>
      <div style={{ fontSize: 18, fontWeight: 700, color: "var(--color-ink)", marginBottom: 4 }}>{value}</div>
      {delta !== undefined && (
        <span style={{ display: "flex", alignItems: "center", gap: 4, fontSize: 11, fontWeight: 500, color: delta >= 0 ? "var(--color-success)" : "var(--color-danger)" }}>
          {delta >= 0 ? <TrendingUp size={12} /> : <TrendingDown size={12} />}
          {Math.abs(delta)}% vs last week
        </span>
      )}
      {children}
    </div>
  );
}
