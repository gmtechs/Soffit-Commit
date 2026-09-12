import React, { useEffect, useState } from "react";
import { Moon, Sun, Clock, Wifi, Copy, Brain, Download, Trash2, CheckCircle, AlertCircle, UserPlus, Pencil, Save, SlidersHorizontal, CircleHelp, Info } from "lucide-react";
import { getSetting, setSetting, listPeers, getNodeId, aiStatus, aiDownloadModels, aiRemoveModels, listUsers, createUser, updateUser, isTauri, type Peer, type AiStatus, type User as AppUser } from "../lib/tauri";
import { listen } from "@tauri-apps/api/event";
import { Button } from "../components/ui/Button";
import { useToast } from "../components/ui/Toast";
import { useAuthStore } from "../store/auth";
import { HelpPage } from "./Help";
import { AboutPage } from "./About";

export function SettingsPage() {
  const [theme, setTheme] = useState("light");
  const [lockTimeout, setLockTimeout] = useState("15");
  const [peers, setPeers] = useState<Peer[]>([]);
  const [nodeId, setNodeId] = useState<string | null>(null);
  const [section, setSection] = useState<"preferences" | "accounts" | "help" | "about">("preferences");
  const { toast } = useToast();

  useEffect(() => {
    getSetting("theme").then(v => { if (v) { setTheme(v); document.documentElement.setAttribute("data-theme", v); } });
    getSetting("lock_timeout_minutes").then(v => { if (v) setLockTimeout(v); });
    listPeers().then(setPeers).catch(() => {});
    getNodeId().then(setNodeId).catch(() => {});
  }, []);

  const save = async () => {
    try {
      await setSetting("theme", theme);
      await setSetting("lock_timeout_minutes", lockTimeout);
      document.documentElement.setAttribute("data-theme", theme);
      toast("success", "Settings saved");
    } catch (err: any) { toast("danger", String(err)); }
  };

  const copyNodeId = () => {
    if (nodeId) { navigator.clipboard.writeText(nodeId); toast("success", "Node ID copied"); }
  };

  const sectionStyle: React.CSSProperties = {
    background: "var(--color-surface)", borderRadius: "var(--radius-card)",
    boxShadow: "var(--shadow-card)", padding: 24, marginBottom: 20,
  };
  const inputStyle: React.CSSProperties = {
    padding: "8px 10px", borderRadius: 8, border: "1px solid var(--color-border)",
    fontSize: 13, background: "var(--color-bg)", outline: "none", color: "var(--color-ink)",
  };

  return (
    <div style={{ maxWidth: 1080 }}>
      <div style={{ marginBottom: 26 }}>
        <p style={{ fontSize: 11, color: "var(--color-primary)", fontWeight: 700, letterSpacing: ".08em", textTransform: "uppercase", marginBottom: 8 }}>Workspace controls</p>
        <h1 style={{ fontSize: 28, letterSpacing: "-.025em" }}>Settings</h1>
        <p style={{ fontSize: 13, color: "var(--color-text-secondary)", marginTop: 6 }}>Manage your workspace, local accounts, and product information.</p>
      </div>

      <div style={{ display: "grid", gridTemplateColumns: "190px minmax(0, 1fr)", gap: 28, alignItems: "start" }}>
        <nav aria-label="Settings sections" style={{ display: "flex", flexDirection: "column", gap: 3, position: "sticky", top: 12 }}>
          {[
            ["preferences", "Preferences", SlidersHorizontal], ["accounts", "Accounts", UserPlus], ["help", "Help & Docs", CircleHelp], ["about", "About", Info],
          ].map(([key, label, Icon]) => <button key={key as string} onClick={() => setSection(key as typeof section)} style={{ display: "flex", alignItems: "center", gap: 9, padding: "10px 11px", textAlign: "left", border: "1px solid", borderColor: section === key ? "var(--color-border)" : "transparent", background: section === key ? "var(--color-surface)" : "transparent", color: section === key ? "var(--color-ink)" : "var(--color-text-secondary)", fontWeight: section === key ? 700 : 500, borderRadius: 6, fontSize: 13 }}><Icon size={15} /> {label as string}</button>)}
        </nav>
        <div>

      {section === "preferences" && <>

      {/* Appearance */}
      <div style={sectionStyle}>
        <h2 style={{ fontSize: 14, fontWeight: 600, marginBottom: 16, display: "flex", alignItems: "center", gap: 8 }}>
          {theme === "dark" ? <Moon size={16} /> : <Sun size={16} />} Appearance
        </h2>
        <div style={{ display: "flex", gap: 10 }}>
          {["light", "dark"].map(t => (
            <button key={t} onClick={() => { setTheme(t); document.documentElement.setAttribute("data-theme", t); }}
              style={{ padding: "8px 20px", borderRadius: 8, border: `2px solid ${theme === t ? "var(--color-primary)" : "var(--color-border)"}`, background: theme === t ? "#FEF3E2" : "var(--color-bg)", color: theme === t ? "var(--color-primary)" : "var(--color-ink)", fontWeight: 500, fontSize: 13, cursor: "pointer" }}>
              {t.charAt(0).toUpperCase() + t.slice(1)}
            </button>
          ))}
        </div>
      </div>

      {/* Lock timeout */}
      <div style={sectionStyle}>
        <h2 style={{ fontSize: 14, fontWeight: 600, marginBottom: 8, display: "flex", alignItems: "center", gap: 8 }}>
          <Clock size={16} /> Lock timeout
        </h2>
        <p style={{ fontSize: 12, color: "var(--color-text-secondary)", marginBottom: 12 }}>
          Idle locks auto-release after this many minutes, preventing permanent lockouts from crashed clients.
        </p>
        <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
          <input type="number" min={1} max={120} value={lockTimeout} onChange={e => setLockTimeout(e.target.value)}
            style={{ ...inputStyle, width: 80 }} />
          <span style={{ fontSize: 13, color: "var(--color-text-secondary)" }}>minutes</span>
        </div>
      </div>

      {/* Network identity */}
      <div style={sectionStyle}>
        <h2 style={{ fontSize: 14, fontWeight: 600, marginBottom: 12, display: "flex", alignItems: "center", gap: 8 }}>
          <Wifi size={16} /> Network identity
        </h2>
        {nodeId ? (
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <code style={{ fontSize: 11, background: "var(--color-bg)", padding: "6px 10px", borderRadius: 6, flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", border: "1px solid var(--color-border)" }}>
              {nodeId}
            </code>
            <button onClick={copyNodeId} style={{ background: "none", border: "none", cursor: "pointer", color: "var(--color-text-muted)" }}>
              <Copy size={14} />
            </button>
          </div>
        ) : (
          <p style={{ fontSize: 13, color: "var(--color-text-muted)" }}>iroh node starting…</p>
        )}
        <p style={{ fontSize: 11, color: "var(--color-text-muted)", marginTop: 8 }}>
          This is your permanent device address — it never changes and is how peers find you.
        </p>
      </div>

      {/* Connected devices */}
      <div style={sectionStyle}>
        <h2 style={{ fontSize: 14, fontWeight: 600, marginBottom: 16 }}>Connected devices ({peers.length})</h2>
        {peers.length === 0 ? (
          <p style={{ fontSize: 13, color: "var(--color-text-muted)" }}>No paired devices yet. Go to Peers to add one.</p>
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
            {peers.map(p => (
              <div key={p.id} style={{ display: "flex", alignItems: "center", gap: 10, padding: "10px 12px", borderRadius: 8, background: "var(--color-bg)", border: "1px solid var(--color-border)" }}>
                <div style={{ width: 32, height: 32, borderRadius: "50%", background: p.is_online ? "#E8F5E9" : "var(--color-bg)", display: "flex", alignItems: "center", justifyContent: "center", fontWeight: 600, fontSize: 12, color: p.is_online ? "var(--color-success)" : "var(--color-text-muted)" }}>
                  {p.display_name.slice(0, 2).toUpperCase()}
                </div>
                <div style={{ flex: 1 }}>
                  <p style={{ fontSize: 13, fontWeight: 500 }}>{p.display_name}</p>
                  <p style={{ fontSize: 11, color: "var(--color-text-muted)" }}>{p.node_id.slice(0, 20)}…</p>
                </div>
                <span style={{ fontSize: 11, fontWeight: 500, color: p.is_online ? "var(--color-success)" : "var(--color-text-muted)" }}>
                  {p.is_online ? "● Online" : "○ Offline"}
                </span>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* AI Models */}
      <AiPanel />
      <Button variant="primary" onClick={save}>Save settings</Button>
      </>}
      {section === "accounts" && <AccountPanel />}
      {section === "help" && <HelpPage />}
      {section === "about" && <AboutPage />}
        </div>
      </div>
    </div>
  );
}

function AccountPanel() {
  const { user: activeUser, setUser } = useAuthStore();
  const { toast } = useToast();
  const [users, setUsers] = useState<AppUser[]>([]);
  const [editing, setEditing] = useState(false);
  const [adding, setAdding] = useState(false);
  const [username, setUsername] = useState(activeUser?.username ?? "");
  const [password, setPassword] = useState("");
  const [newUsername, setNewUsername] = useState("");
  const [newPassword, setNewPassword] = useState("");

  const load = () => listUsers().then(setUsers).catch(() => {});
  useEffect(() => { if (isTauri()) load(); }, []);
  useEffect(() => setUsername(activeUser?.username ?? ""), [activeUser?.username]);

  if (!activeUser) return null;
  const field: React.CSSProperties = { width: "100%", padding: "9px 10px", border: "1px solid var(--color-border)", background: "var(--color-bg)", color: "var(--color-ink)", borderRadius: 6, fontSize: 13 };
  const updateProfile = async () => {
    if (!isTauri()) { toast("danger", "Profile changes in browser mode are managed by this browser's local account."); return; }
    if (password && password.length < 8) { toast("danger", "New passwords must be at least 8 characters"); return; }
    try { const updated = await updateUser(activeUser.id, username, password || undefined); setUser(updated); setPassword(""); setEditing(false); load(); toast("success", "Profile updated"); }
    catch (error) { toast("danger", String(error)); }
  };
  const addAccount = async () => {
    if (newPassword.length < 8) { toast("danger", "Passwords must be at least 8 characters"); return; }
    try { await createUser(newUsername, newPassword); setNewUsername(""); setNewPassword(""); setAdding(false); load(); toast("success", "Local account added"); }
    catch (error) { toast("danger", String(error)); }
  };

  return <div style={{ background: "var(--color-surface)", border: "1px solid var(--color-border)", padding: 24, marginBottom: 20 }}>
    <div style={{ display: "flex", justifyContent: "space-between", gap: 16, alignItems: "flex-start", marginBottom: 18 }}>
      <div><h2 style={{ fontSize: 15 }}>Accounts</h2><p style={{ fontSize: 12, color: "var(--color-text-secondary)", marginTop: 4 }}>{isTauri() ? "Local accounts can sign in on this device." : "Browser mode keeps your account in this browser only."}</p></div>
      {isTauri() && <Button variant="secondary" size="sm" onClick={() => setAdding(value => !value)}><UserPlus size={14} /> Add user</Button>}
    </div>
    <div style={{ padding: 14, border: "1px solid var(--color-border)", background: "var(--color-bg)" }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}><div><p style={{ fontWeight: 700, fontSize: 13 }}>{activeUser.username}</p><p style={{ fontSize: 11, color: "var(--color-text-muted)", marginTop: 3 }}>Signed-in account</p></div><Button variant="ghost" size="sm" onClick={() => setEditing(value => !value)}><Pencil size={13} /> Edit profile</Button></div>
      {editing && <div style={{ display: "grid", gridTemplateColumns: "minmax(0, 1fr) minmax(0, 1fr) auto", gap: 8, marginTop: 14 }}><input aria-label="Profile username" value={username} onChange={e => setUsername(e.target.value)} style={field} /><input aria-label="New password" type="password" placeholder="New password (optional)" value={password} onChange={e => setPassword(e.target.value)} style={field} /><Button variant="primary" size="sm" onClick={updateProfile}><Save size={13} /> Save</Button></div>}
    </div>
    {adding && <div style={{ display: "grid", gridTemplateColumns: "minmax(0, 1fr) minmax(0, 1fr) auto", gap: 8, marginTop: 12 }}><input aria-label="New username" placeholder="New username" value={newUsername} onChange={e => setNewUsername(e.target.value)} style={field} /><input aria-label="New user password" type="password" placeholder="Password, 8+ characters" value={newPassword} onChange={e => setNewPassword(e.target.value)} style={field} /><Button variant="primary" size="sm" onClick={addAccount} disabled={!newUsername.trim() || !newPassword}><UserPlus size={13} /> Add</Button></div>}
    {isTauri() && users.filter(account => account.id !== activeUser.id).length > 0 && <p style={{ marginTop: 14, fontSize: 12, color: "var(--color-text-secondary)" }}>{users.filter(account => account.id !== activeUser.id).length} additional local account{users.length === 2 ? "" : "s"} available.</p>}
  </div>;
}

function AiPanel() {
  const [status, setStatus] = useState<AiStatus | null>(null);
  const [downloading, setDownloading] = useState(false);
  const [progress, setProgress] = useState<{ filename: string; pct: number } | null>(null);
  const { toast } = useToast();

  const reload = () => aiStatus().then(setStatus).catch(() => {});

  useEffect(() => {
    if (!isTauri()) return;
    reload();
    // Listen for download progress events
    let unlisten: (() => void) | undefined;
    listen<{ filename: string; bytes_done: number; bytes_total: number }>("ai-download-progress", (e) => {
      const { filename, bytes_done, bytes_total } = e.payload;
      const pct = bytes_total > 0 ? Math.round((bytes_done / bytes_total) * 100) : 0;
      setProgress({ filename: filename.replace(".gguf", ""), pct });
    }).then(u => { unlisten = u; });
    return () => { unlisten?.(); };
  }, []);

  const handleDownload = async () => {
    setDownloading(true);
    setProgress(null);
    try {
      await aiDownloadModels();
      toast("success", "Models downloaded successfully");
      reload();
    } catch (err: any) {
      toast("danger", `Download failed: ${err}`);
    } finally {
      setDownloading(false);
      setProgress(null);
    }
  };

  const handleRemove = async () => {
    try {
      await aiRemoveModels();
      toast("success", "Models removed");
      reload();
    } catch (err: any) {
      toast("danger", String(err));
    }
  };

  const sectionStyle: React.CSSProperties = {
    background: "var(--color-surface)", borderRadius: "var(--radius-card)",
    border: "1px solid var(--color-border)", padding: 24, marginBottom: 20,
  };

  const chatReady = status?.chat_model === "ready";
  const embedReady = status?.embed_model === "ready";
  const bothReady = chatReady && embedReady;
  const totalMb = ((status?.chat_size_bytes ?? 0) + (status?.embed_size_bytes ?? 0)) / (1024 * 1024);

  return (
    <div style={sectionStyle}>
      <h2 style={{ fontSize: 14, fontWeight: 600, marginBottom: 4, display: "flex", alignItems: "center", gap: 8 }}>
        <Brain size={16} /> Soffit AI
      </h2>
      <p style={{ fontSize: 12, color: "var(--color-text-secondary)", marginBottom: 16 }}>
        Offline AI features — conflict explanation, activity summaries, data insights.
        Models run entirely on your device. Document chat retrieves relevant excerpts before answering to keep CPU and memory use controlled.
      </p>

      {/* Model status rows */}
      {[
        { label: "Chat model (Qwen3-0.6B Q4_K_M)", status: status?.chat_model, ready: chatReady, size: status?.chat_size_bytes ?? 0 },
        { label: "Embedding model (Qwen3-Embedding Q8_0)", status: status?.embed_model, ready: embedReady, size: status?.embed_size_bytes ?? 0 },
      ].map(m => (
        <div key={m.label} style={{ display: "flex", alignItems: "center", gap: 10, padding: "8px 0", borderBottom: "1px solid var(--color-border)" }}>
          {m.ready
            ? <CheckCircle size={14} color="var(--color-success)" />
            : <AlertCircle size={14} color="var(--color-text-muted)" />}
          <span style={{ flex: 1, fontSize: 12 }}>{m.label}</span>
          <span style={{ fontSize: 11, color: "var(--color-text-muted)" }}>
            {m.ready
              ? `${(m.size / (1024 * 1024)).toFixed(0)} MB`
              : typeof m.status === "object" && "error" in m.status
                ? "Invalid file — download again"
                : "Not downloaded"}
          </span>
        </div>
      ))}

      {/* Download progress */}
      {progress && (
        <div style={{ marginTop: 12 }}>
          <p style={{ fontSize: 12, marginBottom: 4 }}>{progress.filename} — {progress.pct}%</p>
          <div style={{ height: 6, borderRadius: 3, background: "var(--color-border)", overflow: "hidden" }}>
            <div style={{ height: "100%", width: `${progress.pct}%`, background: "var(--color-primary)", transition: "width 0.3s" }} />
          </div>
        </div>
      )}

      <div style={{ marginTop: 16, display: "flex", gap: 10, alignItems: "center" }}>
        {!bothReady && (
          <Button variant="primary" onClick={handleDownload} disabled={downloading}>
            <Download size={13} /> {downloading ? "Downloading…" : "Download models (~1.1 GB)"}
          </Button>
        )}
        {bothReady && (
          <>
            <span style={{ fontSize: 12, color: "var(--color-success)", display: "flex", alignItems: "center", gap: 5 }}>
              <CheckCircle size={13} /> AI ready · {totalMb.toFixed(0)} MB on disk
            </span>
            <Button variant="secondary" onClick={handleRemove}><Trash2 size={13} /> Remove models</Button>
          </>
        )}
      </div>
    </div>
  );
}
