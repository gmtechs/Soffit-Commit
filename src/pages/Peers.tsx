import React, { useEffect, useState } from "react";
import { UserPlus, Trash2, Edit2, Check, X, Copy, Monitor } from "lucide-react";
import {
  listPeers, removePeer, renamePeer, generatePairingCode,
  consumePairingCode, reconnectPeers,
  listShares, setPermission, getPeerPermissions,
  type Peer, type PairingCodeWithQr, type Share,
} from "../lib/tauri";
import { Button } from "../components/ui/Button";
import { Modal } from "../components/ui/Modal";
import { useToast } from "../components/ui/Toast";

const inputStyle: React.CSSProperties = {
  width: "100%", padding: "9px 12px", borderRadius: 8,
  border: "1px solid var(--color-border)", fontSize: 13,
  background: "var(--color-bg)", color: "var(--color-ink)", outline: "none",
};

export function PeersPage() {
  const [peers, setPeers] = useState<Peer[]>([]);
  const [tab, setTab] = useState<"show" | "enter">("show");
  const [pairOpen, setPairOpen] = useState(false);
  const [pairingCode, setPairingCode] = useState<PairingCodeWithQr | null>(null);
  const [enteredCode, setEnteredCode] = useState("");
  const [peerName, setPeerName] = useState("");
  const [entering, setEntering] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editName, setEditName] = useState("");
  const [ownedShares, setOwnedShares] = useState<Share[]>([]);
  const [permMap, setPermMap] = useState<Record<string, "none" | "view" | "edit">>({});
  const { toast } = useToast();

  const load = () => {
    listShares()
      .then(all => setOwnedShares(all.filter(s => s.is_owner)))
      .catch(() => {});
    listPeers().then(async ps => {
      setPeers(ps);
      if (ps.length === 0) return;
      const rows = await Promise.all(ps.map(p => getPeerPermissions(p.id).catch(() => [])));
      setPermMap(prev => {
        const next: Record<string, "none" | "view" | "edit"> = { ...prev };
        ps.forEach((p, i) => {
          for (const row of rows[i]) next[`${p.id}:${row.share_id}`] = row.level;
        });
        return next;
      });
    }).catch(() => {});
  };

  useEffect(() => {
    load();
    reconnectPeers().then(n => {
      if (n > 0) toast("success", `Reconnecting to ${n} known peer${n !== 1 ? "s" : ""}…`);
    }).catch(() => {});
    const poll = window.setInterval(load, 3000);
    return () => window.clearInterval(poll);
  }, []);

  const openPairModal = async () => {
    try {
      const code = await generatePairingCode();
      setPairingCode(code);
      setTab("show");
      setPairOpen(true);
    } catch (err: any) { toast("danger", String(err)); }
  };

  const copyCode = () => {
    if (!pairingCode) return;
    navigator.clipboard.writeText(pairingCode.code);
    toast("success", "Connection code copied — paste it on the other PC");
  };

  const copyFullCode = () => {
    if (!pairingCode) return;
    navigator.clipboard.writeText(pairingCode.code);
    toast("success", "Full connection code copied");
  };

  const handleEnterCode = async () => {
    if (!enteredCode.trim()) { toast("danger", "Enter a code"); return; }
    const name = peerName.trim() || "Remote PC";
    setEntering(true);
    try {
      const addr = await consumePairingCode(enteredCode.trim(), name);
      toast("success", addr
        ? `Connecting to ${name} — direct connection established`
        : `Code accepted. ${name} will appear once they're online.`
      );
      setPairOpen(false);
      setEnteredCode(""); setPeerName("");
      setTimeout(load, 2000);
    } catch (err: any) { toast("danger", String(err)); }
    finally { setEntering(false); }
  };

  const handleRemove = async (id: string, name: string) => {
    if (!confirm(`Remove "${name}"? This revokes all their access.`)) return;
    try { await removePeer(id); toast("success", "Peer removed"); load(); }
    catch (err: any) { toast("danger", String(err)); }
  };

  const handleRename = async (id: string) => {
    if (!editName.trim()) return;
    try { await renamePeer(id, editName.trim()); setEditingId(null); load(); }
    catch (err: any) { toast("danger", String(err)); }
  };

  const handlePermission = async (peerId: string, shareId: string, shareName: string, level: string) => {
    try {
      await setPermission(shareId, peerId, level);
      setPermMap(m => ({ ...m, [`${peerId}:${shareId}`]: level as "none" | "view" | "edit" }));
      toast("success", level === "none"
        ? `Access to "${shareName}" revoked`
        : `"${shareName}" shared — this device can ${level === "view" ? "view" : "edit"}`);
    } catch (err: any) { toast("danger", String(err)); }
  };

  const tabStyle = (active: boolean): React.CSSProperties => ({
    flex: 1, padding: "8px 0", border: "none", borderRadius: 8, fontSize: 13,
    fontWeight: 500, cursor: "pointer", transition: "all 0.15s",
    background: active ? "var(--color-primary)" : "transparent",
    color: active ? "white" : "var(--color-text-secondary)",
  });

  return (
    <div>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 20 }}>
        <div>
          <h1 style={{ fontSize: 20, fontWeight: 700 }}>Peers</h1>
          <p style={{ fontSize: 13, color: "var(--color-text-secondary)" }}>
            {peers.length === 0 ? "No devices paired yet" : `${peers.filter(p => p.is_online).length} of ${peers.length} online`}
          </p>
        </div>
        <Button variant="primary" onClick={openPairModal}>
          <UserPlus size={15} /> Add device
        </Button>
      </div>

      <div style={{ background: "var(--color-surface)", borderRadius: "var(--radius-card)", boxShadow: "var(--shadow-card)", overflow: "hidden" }}>
        {peers.length === 0 ? (
          <div style={{ padding: 56, textAlign: "center", color: "var(--color-text-muted)" }}>
            <Monitor size={40} style={{ margin: "0 auto 14px" }} />
            <p style={{ fontWeight: 600, marginBottom: 6, color: "var(--color-ink)" }}>No paired devices</p>
            <p style={{ fontSize: 13, marginBottom: 20 }}>
              Click "Add device", copy the code, and paste it on the other PC running Soffit Commit.
            </p>
            <Button variant="primary" onClick={openPairModal}><UserPlus size={14} /> Add first device</Button>
          </div>
        ) : (
          <table style={{ width: "100%", borderCollapse: "collapse", fontSize: 13 }}>
            <thead>
              <tr style={{ background: "var(--color-bg)", borderBottom: "1px solid var(--color-border)" }}>
                {["Device", "Status", "Shared folders", "Last seen", "Actions"].map((h, i) => (
                  <th key={i} style={{ padding: "10px 16px", textAlign: "left", fontWeight: 500, color: "var(--color-text-secondary)" }}>{h}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {peers.map(p => (
                <tr key={p.id} style={{ borderBottom: "1px solid var(--color-border)" }}
                  onMouseEnter={e => (e.currentTarget.style.background = "var(--color-bg)")}
                  onMouseLeave={e => (e.currentTarget.style.background = "transparent")}>
                  <td style={{ padding: "12px 16px" }}>
                    <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                      <div style={{ width: 34, height: 34, borderRadius: "50%", background: p.is_online ? "#E8F5E9" : "var(--color-bg)", display: "flex", alignItems: "center", justifyContent: "center", fontWeight: 700, fontSize: 12, color: p.is_online ? "var(--color-success)" : "var(--color-text-muted)" }}>
                        {p.display_name.slice(0, 2).toUpperCase()}
                      </div>
                      {editingId === p.id ? (
                        <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
                          <input value={editName} onChange={e => setEditName(e.target.value)} autoFocus
                            onKeyDown={e => { if (e.key === "Enter") handleRename(p.id); if (e.key === "Escape") setEditingId(null); }}
                            style={{ ...inputStyle, width: 160, padding: "4px 8px" }} />
                          <button onClick={() => handleRename(p.id)} style={{ background: "none", border: "none", cursor: "pointer", color: "var(--color-success)" }}><Check size={14} /></button>
                          <button onClick={() => setEditingId(null)} style={{ background: "none", border: "none", cursor: "pointer", color: "var(--color-text-muted)" }}><X size={14} /></button>
                        </div>
                      ) : (
                        <div>
                          <p style={{ fontWeight: 500 }}>{p.display_name}</p>
                          <p style={{ fontSize: 11, color: "var(--color-text-muted)", marginTop: 1 }}>
                            {p.node_id.slice(0, 20)}…
                          </p>
                        </div>
                      )}
                    </div>
                  </td>
                  <td style={{ padding: "12px 16px" }}>
                    <span style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 12, fontWeight: 500, color: p.is_online ? "var(--color-success)" : "var(--color-text-muted)" }}>
                      <span style={{ width: 7, height: 7, borderRadius: "50%", background: p.is_online ? "var(--color-success)" : "var(--color-text-muted)", display: "inline-block", opacity: p.is_online ? 1 : 0.5 }} />
                      {p.is_online ? "Online" : "Offline"}
                    </span>
                  </td>
                  <td style={{ padding: "12px 16px" }}>
                    {ownedShares.length === 0 ? (
                      <span style={{ fontSize: 12, color: "var(--color-text-muted)" }}>No shares yet</span>
                    ) : (
                      <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
                        {ownedShares.map(s => (
                          <div key={s.id} style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 10 }}>
                            <span title={s.path} style={{ fontSize: 12, color: "var(--color-text-secondary)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", maxWidth: 150 }}>
                              {s.display_name}
                            </span>
                            <select
                              value={permMap[`${p.id}:${s.id}`] ?? "none"}
                              onChange={e => handlePermission(p.id, s.id, s.display_name, e.target.value)}
                              style={{ padding: "3px 6px", borderRadius: 6, border: "1px solid var(--color-border)", fontSize: 12, background: "var(--color-bg)", color: "var(--color-ink)", cursor: "pointer" }}
                            >
                              <option value="none">No access</option>
                              <option value="view">View</option>
                              <option value="edit">Edit</option>
                            </select>
                          </div>
                        ))}
                      </div>
                    )}
                  </td>
                  <td style={{ padding: "12px 16px", color: "var(--color-text-secondary)" }}>
                    {p.last_seen ? new Date(p.last_seen).toLocaleString() : "Never"}
                  </td>
                  <td style={{ padding: "12px 16px" }}>
                    <div style={{ display: "flex", gap: 6 }}>
                      <Button size="sm" variant="ghost" onClick={() => { setEditingId(p.id); setEditName(p.display_name); }}><Edit2 size={13} /></Button>
                      <Button size="sm" variant="destructive" onClick={() => handleRemove(p.id, p.display_name)}><Trash2 size={13} /></Button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      {/* Pairing modal */}
      <Modal open={pairOpen} onClose={() => setPairOpen(false)} title="Add device" width={500}>
        {/* Tab toggle */}
        <div style={{ display: "flex", background: "var(--color-bg)", borderRadius: 10, padding: 4, marginBottom: 20, gap: 4 }}>
          <button style={tabStyle(tab === "show")} onClick={() => setTab("show")}>Share my code</button>
          <button style={tabStyle(tab === "enter")} onClick={() => setTab("enter")}>Enter their code</button>
        </div>

        {tab === "show" && pairingCode && (
          <div style={{ textAlign: "center" }}>
            <p style={{ fontSize: 13, color: "var(--color-text-secondary)", marginBottom: 16 }}>
              On the other PC, open Soffit Commit, click "Add device" → "Enter their code", and type this code. Valid for 15 minutes, single use.
            </p>

            {/* Big code display */}
            <div style={{ background: "var(--color-bg)", borderRadius: 12, padding: "18px 24px", marginBottom: 12, display: "inline-block", minWidth: 240 }}>
              <p style={{ fontSize: 36, fontWeight: 700, letterSpacing: "0.15em", color: "var(--color-ink)", fontFamily: "monospace" }}>
                {pairingCode.short_code}
              </p>
            </div>

            <div style={{ display: "flex", gap: 8, justifyContent: "center", marginBottom: 16 }}>
              <Button variant="primary" onClick={copyCode}>
                <Copy size={13} /> Copy short code
              </Button>
              <Button variant="secondary" onClick={copyFullCode}>
                <Copy size={13} /> Copy full connection code
              </Button>
            </div>

            <p style={{ fontSize: 12, color: "var(--color-text-muted)", marginBottom: 4 }}>
              The "full connection code" includes your network address so the other PC can connect directly without a relay.
            </p>
            <p style={{ fontSize: 11, color: "var(--color-text-muted)" }}>
              Expires: {new Date(pairingCode.expires_at).toLocaleTimeString()}
            </p>

            <div style={{ marginTop: 16 }}>
              <Button variant="secondary" onClick={openPairModal}>Generate new code</Button>
            </div>
          </div>
        )}

        {tab === "enter" && (
          <div style={{ display: "flex", flexDirection: "column", gap: 14 }}>
            <p style={{ fontSize: 13, color: "var(--color-text-secondary)" }}>
              Paste the code shown on the other PC. You can use the short code (XXXX-XXXX) or the full connection code.
            </p>
            <div>
              <label style={{ fontSize: 12, fontWeight: 500, display: "block", marginBottom: 6 }}>Connection code</label>
              <input
                style={inputStyle}
                value={enteredCode}
                onChange={e => setEnteredCode(e.target.value)}
                placeholder="XXXX-XXXX or full code"
                autoFocus
                onKeyDown={e => e.key === "Enter" && handleEnterCode()}
              />
            </div>
            <div>
              <label style={{ fontSize: 12, fontWeight: 500, display: "block", marginBottom: 6 }}>Device name <span style={{ color: "var(--color-text-muted)", fontWeight: 400 }}>(optional)</span></label>
              <input
                style={inputStyle}
                value={peerName}
                onChange={e => setPeerName(e.target.value)}
                placeholder="e.g. Work Laptop"
                onKeyDown={e => e.key === "Enter" && handleEnterCode()}
              />
            </div>
            <div style={{ display: "flex", gap: 10, justifyContent: "flex-end" }}>
              <Button variant="secondary" onClick={() => setPairOpen(false)}>Cancel</Button>
              <Button variant="primary" onClick={handleEnterCode} disabled={entering || !enteredCode.trim()}>
                {entering ? "Connecting…" : "Connect"}
              </Button>
            </div>
          </div>
        )}
      </Modal>
    </div>
  );
}
