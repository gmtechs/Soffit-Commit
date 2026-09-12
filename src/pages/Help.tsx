import React from "react";
import { BookOpen, Users, Folder, Table2, Database, ArrowRight, MessageCircleQuestion } from "lucide-react";

const topics = [
  { icon: <Users size={18} />, title: "Pairing devices", desc: "Install Soffit Commit on both computers. Click 'Add device' on one, enter the code on the other. Done in under 2 minutes — no IP addresses or port forwarding needed." },
  { icon: <Folder size={18} />, title: "Sharing folders", desc: "Go to Files, click 'Add share', and pick a folder. Set per-peer permissions (No Access / View / Edit) from the Peers screen." },
  { icon: <Table2 size={18} />, title: "Excel mode & Form mode", desc: "Open any .xlsx from Files. Toggle between Grid (full spreadsheet) and Form (one record at a time) using the segmented control at the top. Switching never loses unsaved changes." },
  { icon: <Database size={18} />, title: "SQL console", desc: "Open a .sql file or point at a .csv, .parquet, or .sqlite file. Run queries against the embedded DuckDB engine. Changes always show a diff before writing." },
  { icon: <BookOpen size={18} />, title: "Locks & conflicts", desc: "When you open a file to edit, other peers see it as Locked. If two devices edit offline and reconnect, you'll see a Conflict banner with Keep mine / Keep theirs / Keep both options." },
];

export function HelpPage() {
  return (
    <div style={{ maxWidth: 980 }}>
      <div style={{ display: "grid", gridTemplateColumns: "minmax(0, 1fr) 250px", gap: 28, paddingBottom: 26, borderBottom: "1px solid var(--color-border)", marginBottom: 24 }}>
        <div>
          <p style={{ color: "var(--color-primary)", fontWeight: 700, fontSize: 11, letterSpacing: ".08em", textTransform: "uppercase", marginBottom: 10 }}>Support centre</p>
          <h1 style={{ fontSize: 28, letterSpacing: "-.025em" }}>Get moving with confidence.</h1>
          <p style={{ marginTop: 10, fontSize: 14, color: "var(--color-text-secondary)", maxWidth: 580, lineHeight: 1.6 }}>Short, practical guides for pairing devices, sharing working folders, and editing data safely.</p>
        </div>
        <div style={{ alignSelf: "end", padding: 16, background: "var(--color-surface)", border: "1px solid var(--color-border)" }}>
          <MessageCircleQuestion size={18} color="var(--color-primary)" />
          <p style={{ fontWeight: 700, marginTop: 10, fontSize: 13 }}>Start with your first device</p>
          <p style={{ fontSize: 12, lineHeight: 1.5, color: "var(--color-text-secondary)", marginTop: 4 }}>Pair a second computer before creating shared folders.</p>
        </div>
      </div>

      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(280px, 1fr))", gap: 14 }}>
        {topics.map((t, index) => <article key={t.title} style={{ position: "relative", minHeight: 190, background: "var(--color-surface)", border: "1px solid var(--color-border)", padding: 20, display: "flex", flexDirection: "column" }}>
          <span style={{ fontSize: 11, fontWeight: 700, color: "var(--color-text-muted)" }}>0{index + 1}</span>
          <div style={{ color: "var(--color-primary)", marginTop: 14 }}>{t.icon}</div>
          <h2 style={{ fontSize: 15, marginTop: 12 }}>{t.title}</h2>
          <p style={{ fontSize: 13, color: "var(--color-text-secondary)", lineHeight: 1.55, marginTop: 7 }}>{t.desc}</p>
          <ArrowRight size={15} color="var(--color-text-muted)" style={{ marginTop: "auto", paddingTop: 12 }} />
        </article>)}
      </div>
    </div>
  );
}
