import React, { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import {
  Folder, FileSpreadsheet, Database, File,
  RefreshCw, Plus, Trash2, ExternalLink, HardDrive, FolderOpen,
  Star, Clock, Eye, X as XIcon,
  MessageCircle,
} from "lucide-react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { openPath } from "@tauri-apps/plugin-opener";
import {
  listShares, listFiles, createShare, deleteShare,
  refreshShare, setSelectiveSync,
  getFileHistory, getFavourites, addFavourite, removeFavourite,
  type Share, type FileIndex, type FileHistoryEntry, type FileFavourite,
} from "../lib/tauri";
import { StatusPill } from "../components/ui/StatusPill";
import { Button } from "../components/ui/Button";
import { Modal } from "../components/ui/Modal";
import { useToast } from "../components/ui/Toast";
import { FileViewer } from "../components/ui/FileViewer";
import { DocumentChat } from "../components/ui/DocumentChat";
import { useFileRouter } from "../store/fileRouter";

function FileKindIcon({ kind }: { kind: string }) {
  switch (kind) {
    case "excel":   return <FileSpreadsheet size={16} color="var(--color-success)" />;
    case "sql":     return <Database size={16} color="var(--color-info)" />;
    case "csv":     return <Database size={16} color="var(--color-info)" />;
    case "parquet": return <Database size={16} color="var(--color-info)" />;
    case "sqlite":  return <Database size={16} color="var(--color-info)" />;
    default:        return <File size={16} color="var(--color-text-muted)" />;
  }
}

function formatBytes(b: number) {
  if (b >= 1e9) return (b / 1e9).toFixed(1) + " GB";
  if (b >= 1e6) return (b / 1e6).toFixed(1) + " MB";
  if (b >= 1e3) return (b / 1e3).toFixed(1) + " KB";
  return b + " B";
}

const inputStyle: React.CSSProperties = {
  width: "100%", padding: "9px 12px", borderRadius: 8,
  border: "1px solid var(--color-border)", fontSize: 13,
  background: "var(--color-bg)", color: "var(--color-ink)", outline: "none",
};

export function FilesPage() {
  const [shares, setShares] = useState<Share[]>([]);
  const [selected, setSelected] = useState<Share | null>(null);
  const [files, setFiles] = useState<FileIndex[]>([]);
  const [addOpen, setAddOpen] = useState(false);
  const [newPath, setNewPath] = useState("");
  const [newName, setNewName] = useState("");
  const [previewFile, setPreviewFile] = useState<{ path: string; kind: string; name: string } | null>(null);
  const [showDocumentChat, setShowDocumentChat] = useState(false);
  const [history, setHistory] = useState<FileHistoryEntry[]>([]);
  const [favourites, setFavourites] = useState<FileFavourite[]>([]);
  const [sideTab, setSideTab] = useState<"shares" | "recent" | "favourites">("shares");
  const { toast } = useToast();
  const navigate = useNavigate();
  const { setPendingFile } = useFileRouter();

  const loadShares = () => listShares().then(setShares).catch(() => {});
  const loadHistory = () => getFileHistory(15).then(setHistory).catch(() => {});
  const loadFavourites = () => getFavourites().then(setFavourites).catch(() => {});

  useEffect(() => { loadShares(); loadHistory(); loadFavourites(); }, []);

  useEffect(() => {
    if (selected) listFiles(selected.id).then(setFiles).catch(() => {});
    else setFiles([]);
  }, [selected]);

  const handleAdd = async () => {
    if (!newPath.trim()) { toast("danger", "Please select a folder"); return; }
    const name = newName.trim() || newPath.split("/").filter(Boolean).pop() || "Share";
    try {
      const share = await createShare(newPath.trim(), name);
      toast("success", "Share created");
      setAddOpen(false); setNewPath(""); setNewName("");
      await loadShares();
      setSelected(share);
    } catch (err: any) { toast("danger", String(err)); }
  };

  const pickFolder = async () => {
    try {
      const selected = await openDialog({ directory: true, multiple: false, title: "Choose a folder to share" });
      if (typeof selected === "string") {
        setNewPath(selected);
        if (!newName) setNewName(selected.split("/").filter(Boolean).pop() || "");
      }
    } catch { /* user cancelled */ }
  };

  const handleDelete = async (id: string) => {
    if (!confirm("Remove this share? The files on disk are not deleted.")) return;
    try {
      await deleteShare(id);
      toast("success", "Share removed");
      if (selected?.id === id) setSelected(null);
      loadShares();
    } catch (err: any) { toast("danger", String(err)); }
  };

  const handleRefresh = async () => {
    if (!selected) return;
    try {
      await refreshShare(selected.id);
      const updated = await listFiles(selected.id);
      setFiles(updated);
      toast("success", `${updated.length} files indexed`);
    } catch (err: any) { toast("danger", String(err)); }
  };

  const handleSelectiveSync = async (share: Share) => {
    try {
      await setSelectiveSync(share.id, !share.selective_sync);
      toast("success", !share.selective_sync ? "Selective sync on — files fetched on open" : "Selective sync off — always keep local copy");
      loadShares();
    } catch (err: any) { toast("danger", String(err)); }
  };

  return (
    <div style={{ display: "flex", gap: 16, height: "100%", overflow: "hidden" }}>

      {/* ── Left panel: tabs for Shares / Recent / Favourites ── */}
      <div style={{ width: 240, minWidth: 240, background: "var(--color-surface)", borderRadius: "var(--radius-card)", border: "1px solid var(--color-border)", padding: 14, display: "flex", flexDirection: "column", overflow: "hidden" }}>
        {/* Tab bar */}
        <div style={{ display: "flex", gap: 2, marginBottom: 12, background: "var(--color-bg)", borderRadius: 8, padding: 3 }}>
          {([["shares", <Folder size={12} />], ["recent", <Clock size={12} />], ["favourites", <Star size={12} />]] as [string, React.ReactNode][]).map(([tab, icon]) => (
            <button key={tab} onClick={() => setSideTab(tab as any)}
              style={{ flex: 1, padding: "5px 0", borderRadius: 6, border: "none", fontSize: 11, fontWeight: 500, cursor: "pointer",
                background: sideTab === tab ? "var(--color-primary)" : "transparent",
                color: sideTab === tab ? "white" : "var(--color-text-secondary)",
                display: "flex", alignItems: "center", justifyContent: "center", gap: 3 }}>
              {icon} {tab.charAt(0).toUpperCase() + tab.slice(1)}
            </button>
          ))}
        </div>

        {/* Shares tab */}
        {sideTab === "shares" && (
          <>
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 8 }}>
              <span style={{ fontWeight: 600, fontSize: 13 }}>Shares</span>
              <Button size="sm" variant="primary" onClick={() => setAddOpen(true)}><Plus size={12} /></Button>
            </div>
            <div style={{ flex: 1, overflowY: "auto", display: "flex", flexDirection: "column", gap: 3 }}>
              {shares.length === 0 && <p style={{ fontSize: 12, color: "var(--color-text-muted)", textAlign: "center", marginTop: 20 }}>No shares yet</p>}
              {shares.map(s => (
                <div key={s.id} onClick={() => { setSelected(s); setPreviewFile(null); setShowDocumentChat(false); }}
                  style={{ display: "flex", alignItems: "center", gap: 7, padding: "8px 9px", borderRadius: 7, cursor: "pointer",
                    background: selected?.id === s.id ? "var(--color-bg)" : "transparent",
                    border: `1px solid ${selected?.id === s.id ? "var(--color-border)" : "transparent"}` }}>
                  <Folder size={14} color={s.selective_sync ? "var(--color-warning)" : "var(--color-primary)"} />
                  <span style={{ flex: 1, fontSize: 12, fontWeight: 500, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{s.display_name}</span>
                  <button onClick={ev => { ev.stopPropagation(); handleSelectiveSync(s); }} title="Toggle selective sync"
                    style={{ background: "none", border: "none", cursor: "pointer", color: s.selective_sync ? "var(--color-warning)" : "var(--color-text-muted)", padding: 1 }}>
                    <HardDrive size={11} />
                  </button>
                  <button onClick={ev => { ev.stopPropagation(); handleDelete(s.id); }}
                    style={{ background: "none", border: "none", cursor: "pointer", color: "var(--color-text-muted)", padding: 1 }}>
                    <Trash2 size={11} />
                  </button>
                </div>
              ))}
            </div>
          </>
        )}

        {/* Recent tab */}
        {sideTab === "recent" && (
          <div style={{ flex: 1, overflowY: "auto", display: "flex", flexDirection: "column", gap: 3 }}>
            <p style={{ fontWeight: 600, fontSize: 13, marginBottom: 6 }}>Recently opened</p>
            {history.length === 0 && <p style={{ fontSize: 12, color: "var(--color-text-muted)" }}>No history yet</p>}
            {history.map(h => {
              const name = h.file_path.split("/").pop() ?? h.file_path;
              return (
                <div key={h.id} onClick={() => setPreviewFile({ path: h.file_path, kind: h.file_kind, name })}
                  style={{ padding: "8px 9px", borderRadius: 7, cursor: "pointer", fontSize: 12 }}
                  onMouseEnter={e => (e.currentTarget.style.background = "var(--color-bg)")}
                  onMouseLeave={e => (e.currentTarget.style.background = "transparent")}>
                  <p style={{ fontWeight: 500, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{name}</p>
                  <p style={{ fontSize: 10, color: "var(--color-text-muted)", marginTop: 2 }}>{new Date(h.opened_at).toLocaleString()}</p>
                </div>
              );
            })}
          </div>
        )}

        {/* Favourites tab */}
        {sideTab === "favourites" && (
          <div style={{ flex: 1, overflowY: "auto", display: "flex", flexDirection: "column", gap: 3 }}>
            <p style={{ fontWeight: 600, fontSize: 13, marginBottom: 6 }}>Favourites</p>
            {favourites.length === 0 && <p style={{ fontSize: 12, color: "var(--color-text-muted)" }}>Star files to add them here</p>}
            {favourites.map(fav => (
              <div key={fav.id} style={{ display: "flex", alignItems: "center", gap: 6, padding: "8px 9px", borderRadius: 7, cursor: "pointer", fontSize: 12 }}
                onMouseEnter={e => (e.currentTarget.style.background = "var(--color-bg)")}
                onMouseLeave={e => (e.currentTarget.style.background = "transparent")}
                onClick={() => setPreviewFile({ path: fav.file_path, kind: fav.file_kind, name: fav.display_name })}>
                <Star size={12} color="var(--color-warning)" fill="var(--color-warning)" />
                <span style={{ flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", fontWeight: 500 }}>{fav.display_name}</span>
                <button onClick={ev => { ev.stopPropagation(); removeFavourite(fav.file_path).then(loadFavourites); }}
                  style={{ background: "none", border: "none", cursor: "pointer", color: "var(--color-text-muted)", padding: 1 }}>
                  <XIcon size={11} />
                </button>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* ── File list ── */}
      {!previewFile && (
        <div style={{ flex: 1, background: "var(--color-surface)", borderRadius: "var(--radius-card)", border: "1px solid var(--color-border)", padding: 20, overflowY: "auto", display: "flex", flexDirection: "column" }}>
        {!selected ? (
          <div style={{ flex: 1, display: "flex", flexDirection: "column", alignItems: "center", justifyContent: "center", color: "var(--color-text-muted)", gap: 8 }}>
            <Folder size={44} />
            <p style={{ fontWeight: 500 }}>Select a share to browse files</p>
            <Button variant="primary" onClick={() => setAddOpen(true)} style={{ marginTop: 8 }}><Plus size={14} /> Add share</Button>
          </div>
        ) : (
          <>
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 16 }}>
              <div>
                <h2 style={{ fontWeight: 600, fontSize: 15 }}>{selected.display_name}</h2>
                <p style={{ fontSize: 11, color: "var(--color-text-muted)", marginTop: 2 }}>{selected.path}</p>
              </div>
              <Button size="sm" variant="secondary" onClick={handleRefresh}><RefreshCw size={13} /> Refresh</Button>
            </div>
            {files.length === 0 ? (
              <div style={{ flex: 1, display: "flex", flexDirection: "column", alignItems: "center", justifyContent: "center", color: "var(--color-text-muted)", gap: 6 }}>
                <File size={36} /><p style={{ fontWeight: 500 }}>No files indexed</p>
                <p style={{ fontSize: 12 }}>Click Refresh to scan the folder</p>
              </div>
            ) : (
              <table style={{ width: "100%", borderCollapse: "collapse", fontSize: 13 }}>
                <thead>
                  <tr style={{ borderBottom: "1px solid var(--color-border)", background: "var(--color-bg)" }}>
                    {["File", "Size", "Status", "Modified", ""].map((h, i) => (
                      <th key={i} style={{ padding: "8px 10px", textAlign: "left", color: "var(--color-text-secondary)", fontWeight: 500 }}>{h}</th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {files.map(f => {
                    const fname = f.relative_path.split("/").pop() ?? f.relative_path;
                    return (
                      <tr key={f.id} style={{ borderBottom: "1px solid var(--color-border)" }}
                        onMouseEnter={e => (e.currentTarget.style.background = "var(--color-bg)")}
                        onMouseLeave={e => (e.currentTarget.style.background = "transparent")}>
                        <td style={{ padding: "10px 10px" }}>
                          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                            <FileKindIcon kind={f.file_kind} />
                            <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", maxWidth: 200 }}>{f.relative_path}</span>
                          </div>
                        </td>
                        <td style={{ padding: "10px 10px", color: "var(--color-text-secondary)", whiteSpace: "nowrap" }}>{formatBytes(f.size_bytes)}</td>
                        <td style={{ padding: "10px 10px" }}><StatusPill status={f.sync_status} /></td>
                        <td style={{ padding: "10px 10px", color: "var(--color-text-secondary)", whiteSpace: "nowrap" }}>
                          {f.modified_at ? new Date(f.modified_at).toLocaleDateString() : "—"}
                        </td>
                        <td style={{ padding: "10px 10px" }}>
                          <div style={{ display: "flex", gap: 4 }}>
                            {/* Preview — works for any file type */}
                            <button onClick={() => setPreviewFile({ path: `${selected.path}/${f.relative_path}`, kind: f.file_kind, name: fname })}
                              title="Preview" style={{ background: "none", border: "none", cursor: "pointer", color: "var(--color-text-muted)" }}>
                              <Eye size={13} />
                            </button>
                            {/* Favourite */}
                            <button onClick={() => addFavourite(`${selected.path}/${f.relative_path}`, fname, f.file_kind).then(loadFavourites)}
                              title="Add to favourites" style={{ background: "none", border: "none", cursor: "pointer", color: "var(--color-warning)" }}>
                              <Star size={13} />
                            </button>
                            {f.file_kind === "excel" && (
                              <button onClick={() => { setPendingFile({ path: `${selected.path}/${f.relative_path}`, kind: "excel" }); navigate("/excel"); }} title="Open in Excel mode"
                                style={{ background: "none", border: "none", cursor: "pointer", color: "var(--color-success)" }}>
                                <ExternalLink size={13} />
                              </button>
                            )}
                            {["sql","csv","parquet","sqlite"].includes(f.file_kind) && (
                              <button onClick={() => { setPendingFile({ path: `${selected.path}/${f.relative_path}`, kind: f.file_kind as any }); navigate("/sql"); }} title="Open in SQL console"
                                style={{ background: "none", border: "none", cursor: "pointer", color: "var(--color-info)" }}>
                                <ExternalLink size={13} />
                              </button>
                            )}
                          </div>
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            )}
          </>
        )}
        </div>
      )}

      {/* ── File preview panel ── */}
      {previewFile && (
        <div style={{ flex: 1, background: "var(--color-surface)", borderRadius: "var(--radius-card)", border: "1px solid var(--color-border)", display: "flex", flexDirection: "column", overflow: "hidden" }}>
          <div style={{ display: "flex", alignItems: "center", gap: 10, padding: "12px 16px", borderBottom: "1px solid var(--color-border)" }}>
            <FileKindIcon kind={previewFile.kind} />
            <span style={{ fontWeight: 600, fontSize: 13, flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{previewFile.name}</span>
            <button onClick={() => openPath(previewFile.path).catch(() => {})} title="Open with system app"
              style={{ background: "none", border: "none", cursor: "pointer", color: "var(--color-text-muted)" }}>
              <ExternalLink size={14} />
            </button>
            <button onClick={() => addFavourite(previewFile.path, previewFile.name, previewFile.kind).then(loadFavourites)}
              title="Add to favourites" style={{ background: "none", border: "none", cursor: "pointer", color: "var(--color-warning)" }}>
              <Star size={14} />
            </button>
            {["excel", "sql", "csv", "text", "generic"].includes(previewFile.kind) && <button onClick={() => setShowDocumentChat(value => !value)} title="Ask this document"
              style={{ background: "none", border: "none", cursor: "pointer", color: showDocumentChat ? "var(--color-primary)" : "var(--color-text-muted)" }}>
              <MessageCircle size={14} />
            </button>}
            <button onClick={() => setPreviewFile(null)} title="Close preview"
              style={{ background: "none", border: "none", cursor: "pointer", color: "var(--color-text-muted)" }}>
              <XIcon size={14} />
            </button>
          </div>
          <div style={{ flex: 1, overflow: "auto", display: "flex" }}>
            <FileViewer filePath={previewFile.path} fileKind={previewFile.kind} fileName={previewFile.name} />
            {showDocumentChat && <DocumentChat filePath={previewFile.path} fileKind={previewFile.kind} fileName={previewFile.name} />}
          </div>
        </div>
      )}

      <Modal open={addOpen} onClose={() => setAddOpen(false)} title="Add share">
        <div style={{ display: "flex", flexDirection: "column", gap: 14 }}>
          {/* Folder picker */}
          <div>
            <label style={{ fontSize: 12, fontWeight: 500, display: "block", marginBottom: 6 }}>Folder</label>
            <div style={{ display: "flex", gap: 8 }}>
              <div style={{ flex: 1, padding: "9px 12px", borderRadius: 8, border: "1px solid var(--color-border)", fontSize: 13, background: "var(--color-bg)", color: newPath ? "var(--color-ink)" : "var(--color-text-muted)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                {newPath || "No folder selected"}
              </div>
              <Button variant="secondary" onClick={pickFolder}>
                <FolderOpen size={14} /> Browse…
              </Button>
            </div>
          </div>
          {/* Display name */}
          <div>
            <label style={{ fontSize: 12, fontWeight: 500, display: "block", marginBottom: 6 }}>Display name <span style={{ color: "var(--color-text-muted)", fontWeight: 400 }}>(optional)</span></label>
            <input
              style={inputStyle} value={newName}
              onChange={e => setNewName(e.target.value)}
              placeholder={newPath ? newPath.split("/").filter(Boolean).pop() : "e.g. My Documents"}
              onKeyDown={e => e.key === "Enter" && handleAdd()}
            />
          </div>
          <div style={{ display: "flex", gap: 10, justifyContent: "flex-end" }}>
            <Button variant="secondary" onClick={() => { setAddOpen(false); setNewPath(""); setNewName(""); }}>Cancel</Button>
            <Button variant="primary" onClick={handleAdd} disabled={!newPath}>Add share</Button>
          </div>
        </div>
      </Modal>
    </div>
  );
}
