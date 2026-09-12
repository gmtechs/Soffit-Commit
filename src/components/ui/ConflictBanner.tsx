import React, { useEffect, useState } from "react";
import { AlertTriangle, Check, X, Copy, Sparkles } from "lucide-react";
import { listConflicts, resolveConflict, aiExplainConflict, type Conflict } from "../../lib/tauri";
import { useToast } from "./Toast";
import { Button } from "./Button";

export function ConflictBanner() {
  const [conflicts, setConflicts] = useState<Conflict[]>([]);
  const [explanations, setExplanations] = useState<Record<string, string>>({});
  const [explaining, setExplaining] = useState<Record<string, boolean>>({});
  const { toast } = useToast();

  const load = () => listConflicts().then(setConflicts).catch(() => {});
  useEffect(() => { load(); const id = setInterval(load, 10000); return () => clearInterval(id); }, []);

  if (conflicts.length === 0) return null;

  const resolve = async (id: string, res: "keep_mine" | "keep_theirs" | "keep_both") => {
    try {
      await resolveConflict(id, res);
      toast("success", `Conflict resolved: ${res.replace(/_/g, " ")}`);
      load();
    } catch (err: any) { toast("danger", String(err)); }
  };

  const explain = async (c: Conflict) => {
    setExplaining(p => ({ ...p, [c.id]: true }));
    try {
      const result = await aiExplainConflict(
        c.file_path,
        `local hash: ${c.local_hash}, detected: ${c.detected_at}`,
        `remote peer: ${c.remote_peer_id}, hash: ${c.remote_hash}`,
      );
      setExplanations(p => ({ ...p, [c.id]: result.text }));
    } catch {
      setExplanations(p => ({ ...p, [c.id]: "AI unavailable — models not downloaded yet." }));
    } finally {
      setExplaining(p => ({ ...p, [c.id]: false }));
    }
  };

  return (
    <div style={{ marginBottom: 16 }}>
      {conflicts.map(c => (
        <div key={c.id} style={{
          background: "#FEF2F2", border: "1px solid var(--color-danger)", borderRadius: 10,
          padding: "12px 16px", marginBottom: 8,
        }}>
          <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
            <AlertTriangle size={16} color="var(--color-danger)" style={{ flexShrink: 0 }} />
            <div style={{ flex: 1 }}>
              <p style={{ fontSize: 13, fontWeight: 600, color: "var(--color-danger)" }}>Conflict detected</p>
              <p style={{ fontSize: 12, color: "var(--color-text-secondary)" }}>
                {c.file_path.split("/").pop()} — edited on two devices while offline
              </p>
            </div>
            <div style={{ display: "flex", gap: 6 }}>
              <Button size="sm" variant="secondary" onClick={() => explain(c)} disabled={explaining[c.id]}>
                <Sparkles size={12} /> {explaining[c.id] ? "…" : "Explain"}
              </Button>
              <Button size="sm" variant="secondary" onClick={() => resolve(c.id, "keep_mine")}>
                <Check size={12} /> Keep mine
              </Button>
              <Button size="sm" variant="secondary" onClick={() => resolve(c.id, "keep_theirs")}>
                <X size={12} /> Keep theirs
              </Button>
              <Button size="sm" variant="secondary" onClick={() => resolve(c.id, "keep_both")}>
                <Copy size={12} /> Keep both
              </Button>
            </div>
          </div>
          {explanations[c.id] && (
            <div style={{ marginTop: 10, padding: "8px 12px", background: "rgba(220,38,38,0.05)", borderRadius: 6, fontSize: 12, lineHeight: 1.6 }}>
              <span style={{ fontWeight: 600, fontSize: 11, color: "var(--color-danger)", display: "block", marginBottom: 3 }}>Soffit AI</span>
              {explanations[c.id]}
            </div>
          )}
        </div>
      ))}
    </div>
  );
}
