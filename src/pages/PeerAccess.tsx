import { useEffect, useRef, useState } from "react";
import { Folder, Search, X, ChevronLeft, ChevronRight } from "lucide-react";
import { Button } from "../components/ui/Button";
import type { Peer, Share } from "../lib/tauri";
import "./PeerAccess.css";

type Level = "none" | "view" | "edit";
interface Props {
  peer: Peer;
  shares: Share[];
  permissions: Record<string, Level>;
  ready: boolean;
  onClose: () => void;
  onChange: (peerId: string, shareId: string, name: string, level: Level) => Promise<void>;
}
const PAGE_SIZE = 20;

export function PeerAccess({ peer, shares, permissions, ready, onClose, onChange }: Props) {
  const dialog = useRef<HTMLDialogElement>(null);
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState("all");
  const [page, setPage] = useState(0);
  const [saving, setSaving] = useState<string | null>(null);
  const search = useRef<HTMLInputElement>(null);
  const opener = useRef<HTMLElement | null>(null);
  useEffect(() => {
    const element = dialog.current;
    opener.current = document.activeElement as HTMLElement | null;
    element?.showModal();
    search.current?.focus();
    return () => { element?.close(); opener.current?.focus?.(); };
  }, []);
  const levelOf = (id: string) => permissions[`${peer.id}:${id}`] ?? "none";
  const shared = shares.filter(s => levelOf(s.id) !== "none").length;
  const filtered = shares.filter(s =>
    `${s.display_name} ${s.path}`.toLowerCase().includes(query.trim().toLowerCase()) &&
    (filter === "all" || (filter === "shared" ? levelOf(s.id) !== "none" : levelOf(s.id) === "none")));
  const pages = Math.max(1, Math.ceil(filtered.length / PAGE_SIZE));
  const current = Math.min(page, pages - 1);
  const visible = filtered.slice(current * PAGE_SIZE, (current + 1) * PAGE_SIZE);

  return (
    <dialog ref={dialog} className="peer-access" aria-labelledby="peer-access-title" onCancel={onClose}>
      <header className="peer-access-header">
        <div className="peer-access-heading">
          <span className="peer-access-icon"><Folder size={22} /></span>
          <div style={{ minWidth: 0 }}>
            <p className="peer-access-eyebrow">FOLDER ACCESS</p>
            <h2 id="peer-access-title" title={peer.display_name}>{peer.display_name}</h2>
          </div>
        </div>
        <Button variant="ghost" aria-label="Close folder access" onClick={onClose}><X size={18} /></Button>
      </header>
      <p className="peer-access-description">Control which folders on this PC this device can access. Changes save immediately.</p>
      <div className="peer-access-toolbar">
        <label className="peer-access-search"><Search size={16} />
          <input ref={search} aria-label="Search folders" placeholder="Search folders or paths…" value={query} onChange={e => { setQuery(e.target.value); setPage(0); }} />
        </label>
        <select aria-label="Filter folder access" value={filter} onChange={e => { setFilter(e.target.value); setPage(0); }}>
          <option value="all">All folders</option><option value="shared">Shared</option><option value="none">No access</option>
        </select>
      </div>
      <div className="peer-access-columns"><span>Folder</span><span>Permission</span></div>
      <div className="peer-access-list" aria-busy={saving !== null}>
        {!ready ? <p className="peer-access-empty">Folder permissions are unavailable. Retrying…</p> : visible.length === 0 ? (
          <p className="peer-access-empty">{shares.length === 0 ? "No local folders yet. Create a share in Files to manage its access here." : "No folders match your search or filter."}</p>
        ) : visible.map(s => (
          <div className="peer-access-row" key={s.id}>
            <span className="peer-access-folder"><Folder size={17} /></span>
            <div className="peer-access-name"><strong title={s.display_name}>{s.display_name}</strong><span title={s.path}>{s.path}</span></div>
            <div className="peer-access-permission">
              <select aria-label={`Permission for ${s.display_name}`} value={levelOf(s.id)} disabled={saving !== null} data-level={levelOf(s.id)}
                onChange={async e => { setSaving(s.id); try { await onChange(peer.id, s.id, s.display_name, e.target.value as Level); } finally { setSaving(null); } }}>
                <option value="none">No access</option><option value="view">Can view</option><option value="edit">Can edit</option>
              </select>
              {saving === s.id && <span role="status">Saving…</span>}
            </div>
          </div>
        ))}
      </div>
      <footer className="peer-access-footer">
        <span>{ready ? `${shared} of ${shares.length} folders shared` : "Loading permissions…"}</span>
        <div className="peer-access-pagination">
          <Button variant="secondary" size="sm" aria-label="Previous folders" disabled={current === 0} onClick={() => setPage(current - 1)}><ChevronLeft size={16} /></Button>
          <span aria-live="polite">{current + 1} / {pages}</span>
          <Button variant="secondary" size="sm" aria-label="Next folders" disabled={current === pages - 1} onClick={() => setPage(current + 1)}><ChevronRight size={16} /></Button>
        </div>
      </footer>
    </dialog>
  );
}
