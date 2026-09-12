import React from "react";
import { ExternalLink, ShieldCheck, MonitorSmartphone, Sparkles } from "lucide-react";

const capabilities = [
  { icon: MonitorSmartphone, title: "Local-first workspace", text: "Your files, device connections, and offline AI models stay under your control." },
  { icon: ShieldCheck, title: "Built for private teams", text: "Use peer-to-peer sharing and local accounts without handing working files to a hosted service." },
  { icon: Sparkles, title: "Practical AI", text: "Optional on-device models help explain conflicts and answer questions about your documents." },
];

export function AboutPage() {
  return (
    <div style={{ maxWidth: 900 }}>
      <section style={{ padding: "18px 0 28px", borderBottom: "1px solid var(--color-border)" }}>
        <p style={{ color: "var(--color-primary)", fontWeight: 700, fontSize: 12, letterSpacing: ".08em", textTransform: "uppercase", marginBottom: 10 }}>Soffit Commit</p>
        <h1 style={{ fontSize: 30, letterSpacing: "-.03em", lineHeight: 1.1, maxWidth: 620 }}>A calmer place to manage shared work.</h1>
        <p style={{ marginTop: 14, maxWidth: 650, color: "var(--color-text-secondary)", lineHeight: 1.65 }}>Soffit Commit brings files, spreadsheet work, device pairing, activity, and offline assistance into one local-first workspace.</p>
      </section>

      <section style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(210px, 1fr))", gap: 1, marginTop: 24, border: "1px solid var(--color-border)", background: "var(--color-border)" }}>
        {capabilities.map(({ icon: Icon, title, text }) => <article key={title} style={{ background: "var(--color-surface)", padding: 22 }}>
          <Icon size={19} color="var(--color-primary)" />
          <h2 style={{ fontSize: 15, marginTop: 18, marginBottom: 7 }}>{title}</h2>
          <p style={{ fontSize: 13, color: "var(--color-text-secondary)", lineHeight: 1.55 }}>{text}</p>
        </article>)}
      </section>

      <section style={{ marginTop: 26, padding: 22, background: "var(--color-surface)", border: "1px solid var(--color-border)" }}>
        <p style={{ fontSize: 12, color: "var(--color-text-muted)", textTransform: "uppercase", letterSpacing: ".07em", fontWeight: 700 }}>Created by</p>
        <h2 style={{ marginTop: 8, fontSize: 20 }}>Laocta Techlabs</h2>
        <p style={{ marginTop: 8, fontSize: 13, color: "var(--color-text-secondary)" }}>Technology made for dependable, private work.</p>
        <a href="https://laocta.co.ke" target="_blank" rel="noreferrer" style={{ display: "inline-flex", alignItems: "center", gap: 7, marginTop: 16, color: "var(--color-primary)", fontSize: 13, fontWeight: 700 }}>
          laocta.co.ke <ExternalLink size={14} />
        </a>
      </section>
    </div>
  );
}
