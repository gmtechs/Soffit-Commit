import React from "react";
import { BookOpen, Users, Folder, Table2, Database } from "lucide-react";

const topics = [
  { icon: <Users size={18} />, title: "Pairing devices", desc: "Install Soffit Commit on both computers. Click 'Add device' on one, enter the code on the other. Done in under 2 minutes — no IP addresses or port forwarding needed." },
  { icon: <Folder size={18} />, title: "Sharing folders", desc: "Go to Files, click 'Add share', and pick a folder. Set per-peer permissions (No Access / View / Edit) from the Peers screen." },
  { icon: <Table2 size={18} />, title: "Excel mode & Form mode", desc: "Open any .xlsx from Files. Toggle between Grid (full spreadsheet) and Form (one record at a time) using the segmented control at the top. Switching never loses unsaved changes." },
  { icon: <Database size={18} />, title: "SQL console", desc: "Open a .sql file or point at a .csv, .parquet, or .sqlite file. Run queries against the embedded DuckDB engine. Changes always show a diff before writing." },
  { icon: <BookOpen size={18} />, title: "Locks & conflicts", desc: "When you open a file to edit, other peers see it as Locked. If two devices edit offline and reconnect, you'll see a Conflict banner with Keep mine / Keep theirs / Keep both options." },
];

export function HelpPage() {
  return (
    <div style={{ maxWidth: 680 }}>
      <div style={{ marginBottom: 24 }}>
        <h1 style={{ fontSize: 20, fontWeight: 700 }}>Help & Docs</h1>
        <p style={{ fontSize: 13, color: "var(--color-text-secondary)" }}>Quick reference for Soffit Commit features</p>
      </div>

      <div style={{ display: "flex", flexDirection: "column", gap: 14 }}>
        {topics.map((t) => (
          <div key={t.title} style={{ background: "var(--color-surface)", borderRadius: "var(--radius-card)", border: "1px solid var(--color-border)", padding: 20, display: "flex", gap: 14 }}>
            <div style={{ width: 36, height: 36, borderRadius: "50%", background: "#FEF3E2", display: "flex", alignItems: "center", justifyContent: "center", color: "var(--color-primary)", flexShrink: 0 }}>
              {t.icon}
            </div>
            <div>
              <p style={{ fontWeight: 600, marginBottom: 4 }}>{t.title}</p>
              <p style={{ fontSize: 13, color: "var(--color-text-secondary)", lineHeight: 1.6 }}>{t.desc}</p>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
