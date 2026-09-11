import React, { useEffect, useState } from "react";
import { Activity, Sparkles } from "lucide-react";
import { listActivity, aiActivitySummary, type ActivityEntry } from "../lib/tauri";
import { listen } from "@tauri-apps/api/event";
import { Button } from "../components/ui/Button";
import { useToast } from "../components/ui/Toast";

const actionLabel: Record<string, string> = {
  login: "signed in",
  account_created: "created account",
  peer_added: "paired with",
  peer_removed: "removed peer",
  permission_changed: "changed permissions on",
  share_created: "created share",
  share_deleted: "deleted share",
  file_locked: "locked",
  file_unlocked: "unlocked",
  file_saved: "saved",
  conflict_resolved: "resolved conflict on",
};

export function ActivityPage() {
  const [entries, setEntries] = useState<ActivityEntry[]>([]);
  const [summary, setSummary] = useState<string | null>(null);
  const [summarising, setSummarising] = useState(false);
  const { toast } = useToast();

  useEffect(() => {
    listActivity(100).then(setEntries).catch(() => {});
    const id = setInterval(() => listActivity(100).then(setEntries).catch(() => {}), 8000);
    // Stream AI tokens into summary
    let unlisten: (() => void) | undefined;
    listen<string>("ai-token", e => {
      if (summarising) setSummary(prev => (prev ?? "") + e.payload);
    }).then(u => { unlisten = u; });
    return () => { clearInterval(id); unlisten?.(); };
  }, []);

  const handleSummarise = async () => {
    setSummarising(true);
    setSummary("");
    try {
      const result = await aiActivitySummary("this week");
      setSummary(result.text);
    } catch (err: any) {
      toast("danger", `AI unavailable: ${err}`);
      setSummary(null);
    } finally {
      setSummarising(false);
    }
  };

  return (
    <div>
      <div style={{ marginBottom: 20, display: "flex", alignItems: "flex-end", justifyContent: "space-between" }}>
        <div>
          <h1 style={{ fontSize: 20, fontWeight: 700 }}>Activity</h1>
          <p style={{ fontSize: 13, color: "var(--color-text-secondary)" }}>Everything that happened across your shares</p>
        </div>
        <Button variant="secondary" size="sm" onClick={handleSummarise} disabled={summarising}>
          <Sparkles size={13} /> {summarising ? "Thinking…" : "AI summary"}
        </Button>
      </div>

      {summary && (
        <div style={{ background: "var(--color-surface)", border: "1px solid var(--color-border)", borderRadius: "var(--radius-card)", padding: 16, marginBottom: 16, fontSize: 13, lineHeight: 1.7 }}>
          <p style={{ fontWeight: 600, fontSize: 12, color: "var(--color-text-muted)", marginBottom: 6, textTransform: "uppercase", letterSpacing: "0.06em" }}>Soffit AI — Activity summary</p>
          <p>{summary}</p>
        </div>
      )}

      <div style={{ background: "var(--color-surface)", borderRadius: "var(--radius-card)", border: "1px solid var(--color-border)", overflow: "hidden" }}>
        {entries.length === 0 ? (
          <div style={{ padding: 48, textAlign: "center", color: "var(--color-text-muted)" }}>
            <Activity size={36} style={{ margin: "0 auto 12px" }} />
            <p>No activity yet</p>
          </div>
        ) : (
          <div style={{ display: "flex", flexDirection: "column" }}>
            {entries.map((e, i) => (
              <div key={e.id} style={{ display: "flex", alignItems: "flex-start", gap: 12, padding: "14px 20px",
                borderBottom: i < entries.length - 1 ? "1px solid var(--color-border)" : "none" }}>
                <div style={{ width: 32, height: 32, borderRadius: "50%", background: "var(--color-bg)", display: "flex", alignItems: "center", justifyContent: "center", flexShrink: 0, fontWeight: 600, fontSize: 12, color: "var(--color-primary)" }}>
                  {e.actor.slice(0, 2).toUpperCase()}
                </div>
                <div style={{ flex: 1 }}>
                  <p style={{ fontSize: 13, lineHeight: 1.5 }}>
                    <strong>{e.actor}</strong>{" "}
                    <span style={{ color: "var(--color-text-secondary)" }}>{actionLabel[e.action] ?? e.action}</span>{" "}
                    <strong>{e.target}</strong>
                    {e.metadata && <span style={{ color: "var(--color-text-muted)", fontSize: 12 }}> — {e.metadata}</span>}
                  </p>
                  <p style={{ fontSize: 11, color: "var(--color-text-muted)", marginTop: 2 }}>
                    {new Date(e.timestamp).toLocaleString()}
                  </p>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
