# Soffit Commit — Technical Specification

## 1. What this is

Soffit Commit is a standalone, cross-platform application that lets a person connect an unlimited number of their own computers (or other people's computers) together over the network and access/edit files that live on any of them — with per-peer control over who can view vs. edit. It targets **Excel (.xlsx)** and **SQL** as first-class, deeply-supported file types, and handles every other file type generically (browse, transfer, open with the OS default app).

It is explicitly **not** a real-time collaborative editor. There is no operational-transform / CRDT text-merging engine. Simultaneity is handled with a simple checkout-lock model. This keeps the system small, auditable, and fast to build correctly.

There is no central server. Every installed copy of the app is a full peer. Two people (or one person's two machines) can use it fully with nothing else installed and nothing else running in the cloud.

## 2. Goals / non-goals

**Goals**
- Any number of devices can pair with each other and share folders/files.
- Owner of a share decides, per peer, one of: No Access / View Only / Edit.
- Works whether the two computers are on the same LAN or on completely different networks (different WiFi, different countries) — no port forwarding, no manual NAT config.
- Excel files can be edited two ways: a raw spreadsheet grid ("Excel mode") and an auto-generated data-entry form ("Form mode") — both editing the same underlying file, safely, regardless of the file's original shape/layout.
- SQL files (`.sql` scripts) and tabular data (`.csv`, `.sqlite`, `.xlsx`, `.parquet`) can be queried and edited with real SQL.
- One codebase runs as a native desktop app (Windows/macOS/Linux) via Tauri **and** is reachable from an ordinary web browser on the same machine (or remotely, if the user chooses to expose it).
- Installing and pairing two computers should take under two minutes with no manual network configuration.

**Non-goals (v1)**
- Real-time, Google-Docs-style simultaneous co-editing of the same file region.
- Mobile apps (may be revisited later; Tauri supports it but it's out of scope here).
- Multi-tenant cloud hosting / SaaS backend.

## 3. High-level architecture

Every installed instance is identical and contains:

```
┌─────────────────────────────────────────────┐
│                  App UI                       │
│   React + TypeScript SPA                      │
│   Rendered in: (a) native Tauri webview        │
│               (b) any browser via localhost    │
└───────────────────┬───────────────────────────┘
                    │  (Tauri IPC  /  HTTP+WS over localhost)
┌───────────────────▼───────────────────────────┐
│                 Rust core                      │
│  ┌───────────────┐ ┌────────────────────────┐ │
│  │ axum server    │ │ iroh node               │ │
│  │ (local + LAN   │ │ (identity, NAT          │ │
│  │  HTTP/WS API)  │ │  traversal, transport)  │ │
│  └───────────────┘ └────────────────────────┘ │
│  ┌───────────────┐ ┌────────────────────────┐ │
│  │ Excel engine   │ │ SQL engine              │ │
│  │ (umya-         │ │ (DuckDB embedded)       │ │
│  │  spreadsheet)  │ │                         │ │
│  └───────────────┘ └────────────────────────┘ │
│  ┌───────────────┐ ┌────────────────────────┐ │
│  │ SQLite         │ │ Local blob cache        │ │
│  │ (auth, peers,  │ │ (synced file contents)  │ │
│  │  shares, locks,│ │                         │ │
│  │  activity log) │ │                         │ │
│  └───────────────┘ └────────────────────────┘ │
└───────────────────┬───────────────────────────┘
                    │  iroh (QUIC): direct, or relay if NAT blocks it
                    ▼
              Peer's Rust core (identical stack)
```

## 4. Networking layer

**Library:** [`iroh`](https://iroh.computer) (Rust, MIT/Apache-2.0).

- Each installation generates a persistent Ed25519 keypair on first run. The public key is the device's permanent **NodeId** — this is the device's address; there are no IPs to manage.
- iroh always attempts a **direct QUIC connection** first (fast path, typical on the same LAN or when both sides are reachable). If a direct path isn't possible (NAT/firewall), it automatically falls back to a **relay server**, so cross-network / cross-country connections work without any manual configuration. This single library satisfies both the "same LAN" and "different network" requirements.
- All traffic is end-to-end encrypted and mutually authenticated by construction (QUIC + NodeId-as-TLS-identity).

**Companion protocols used on top of iroh:**
- `iroh-blobs` — content-addressed, resumable, integrity-verified transfer of file bytes. This is the transport for actual file contents; it is content-agnostic, so it works identically for a 2 KB `.sql` file and a 200 MB spreadsheet.
- `iroh-gossip` — lightweight pub/sub between paired peers for small, frequent messages: presence ("I'm online"), lock state ("Alice is editing budget.xlsx"), share/permission changes, activity notifications. Nothing here needs to be large or CRDT-merged.

### Pairing flow
1. User A opens **Add device**, the app shows a short pairing code (and QR code) derived from an iroh ticket (NodeId + connection hint).
2. User B enters/scans the code in their own app.
3. Both apps exchange a confirmation; the new peer relationship is stored (NodeId ↔ display name ↔ default permission = No Access).
4. From then on, the two apps auto-reconnect whenever both are online — no re-pairing needed.

## 5. Permission & sharing model

- A **Share** = a folder (or a single file) exposed to zero or more paired peers.
- Per share, per peer, one of three permission levels: **No Access**, **View**, **Edit**.
- Permission changes propagate immediately via `iroh-gossip` to online peers, and are applied on reconnect for offline ones.
- All access is enforced on the **owning** device — a peer with View-only literally cannot fetch write capability; it's not just hidden in the UI.

### Locking (replaces real-time collaboration)
- Opening a file in Edit mode requests a **lock** from the owning device.
- If granted, the lock is broadcast to all peers with access to that share: they see "Locked by {name}" and the file becomes read-only for them until released.
- Locks auto-expire after an idle timeout (configurable, default 15 minutes) to avoid permanent lockouts from a crashed client, with a grace warning shown to the lock holder first.
- If two locks are ever requested at the same instant (race), the owning device's Rust core is the single source of truth and resolves it deterministically (first request wins); the loser gets a "just been locked by X" message, no data loss.

## 6. Local data model (SQLite)

One local SQLite database per installation (not synced — it's device-local state), roughly:

- `users` — local account(s) for app login (see §9 Authentication).
- `peers` — NodeId, display name, public key, trust status, last-seen.
- `shares` — path, display name, owner flag.
- `share_permissions` — share_id, peer_id, level (none/view/edit).
- `locks` — file path, held-by peer_id, acquired_at, expires_at.
- `activity_log` — timestamp, actor, action, target, metadata (for the activity feed / audit trail).
- `file_index` — cached metadata (size, modified time, content hash, file kind) for fast browsing without re-scanning disk every time.

## 7. Excel module

**Parsing/writing engine:** [`umya-spreadsheet`](https://github.com/MathNya/umya-spreadsheet) (Rust, MIT). Chosen because it preserves real OOXML structure — formulas, styles, merged cells, multiple sheets, named ranges — and supports **in-place editing** (only the touched cells change on save), which matters because file shapes are never predefined.

**Two edit modes on the same open file:**

1. **Excel mode** — a full spreadsheet grid in the frontend (open-source grid component, e.g. Fortune-sheet or Univer's client engine) with formula bar, multi-sheet tabs, cell formatting, and undo/redo. This talks to the Rust core cell-by-cell / range-by-range; it never requires re-uploading the whole file.
2. **Form mode** — the Rust core auto-detects the header row and the contiguous data range of the active sheet (no user-supplied schema required) and the frontend renders one editable form per row/record, with Prev/Next navigation. Saving a record writes only that row's cells back through `umya-spreadsheet`.
3. A single toggle switches between the two modes on the same file, at any time, without closing it.

**Constraints:**
- Auto-detection must handle: multiple tables on one sheet, non-A1-starting tables, merged header cells, and sheets with no clear header (fallback: raw grid only, Form mode disabled with an explanatory message rather than guessing wrong).
- Every Form-mode save is validated against the detected column types (number/date/text) before writing, with inline error messages — never a silent bad write.

## 8. SQL module

**Engine:** DuckDB, embedded via `duckdb-rs` (no separate database server/process to install).

- `.sql` files open in a Monaco-based editor with SQL syntax highlighting; a **Run** button executes the query against DuckDB.
- DuckDB can directly query `.csv`, `.json`, `.parquet` files and attach `.sqlite`/`.db` files — so this module doubles as the generic "run SQL over whatever tabular file is here" tool, including read access to `.xlsx` data via DuckDB's spreadsheet extension.
- Query results render as a grid; simple single-table results can be edited inline, generating the corresponding `UPDATE`/`INSERT` statements shown to the user before they're applied (never silent writes to a database file).

## 9. Authentication

- Local-only: a `users` table in the bundled SQLite database. Password hashed with Argon2id, never stored in plaintext.
- This authenticates a person **to their own installed copy of the app** (e.g., a shared family computer) — it is separate from device pairing, which is peer-to-peer identity between installations, not user accounts across a network.
- Session is a local token kept for the OS session; app can be configured to require re-entry after N minutes idle.
- Optional (recommended, see §12): OS-level biometric unlock (Touch ID / Windows Hello) as a convenience layer on top of the password, via Tauri's OS integration.

## 10. Generic file handling

- Anything that isn't Excel or SQL is treated as an opaque blob: list, rename, move, delete, download, and **open with the OS default app** (via Tauri's shell plugin) — so every file type "works" even without a custom in-app viewer.
- Lightweight built-in previews (nice-to-have, not core): images, PDF (via pdf.js), plain text/code (via Monaco), Markdown render.

## 11. Desktop + Web delivery

- The Rust core embeds an `axum` HTTP/WebSocket server bound to localhost (and optionally LAN) at a fixed port.
- The **same** React/TypeScript frontend is served two ways:
  - Inside the native Tauri window (desktop app icon, tray icon, OS notifications).
  - At `http://localhost:<port>` for any browser on that machine — and, if the user explicitly opts in, forwarded through the paired connection so a browser on another device can reach it too.
- This gives "desktop and web" from one frontend codebase and one Rust binary, with no separate web deployment to maintain.

## 12. Recommended additional features

These extend the core without reintroducing real-time-collaboration complexity:

- **Version history** — every save creates a lightweight snapshot (content-addressed via `iroh-blobs`, so duplicate content costs nothing extra); users can view and restore prior versions of a file.
- **Activity feed** — human-readable log of who opened/edited/locked/shared what and when (backed by `activity_log`).
- **Search** — filename search everywhere; for Excel/SQL/text files, optional content indexing (e.g. `tantivy`) for full-text search across shared files.
- **Notifications** — in-app toast + OS notification when: a peer requests access, a lock is released on a file you're waiting for, a sync completes or fails.
- **Conflict banner** — even with locking, handle the edge case of two offline edits diverging: on reconnect, detect divergent hashes and offer "keep mine / keep theirs / save both" rather than silently overwriting.
- **Selective sync** — per share, choose "always keep a local copy" vs. "fetch on open" to control disk usage and bandwidth.
- **Local cache encryption at rest** — encrypt the blob cache with a key derived from the user's app password.
- **Multiple devices per identity** — let one person register more than one of their own machines under one profile, so permissions can be granted to "me, everywhere" rather than per-machine.
- **Tags/favorites** and a **command palette** (Cmd/Ctrl+K) for fast navigation once the number of shared files grows.
- **Read-only share link** — generate a temporary relay-forwarded link so someone without the app installed can view (not edit) one file, for occasional external sharing.
- **Excel form templates** — save a detected form layout as a reusable template for similar future files.

## 13. Non-functional requirements

- **Cross-platform:** Windows, macOS, Linux from one Tauri codebase.
- **Offline-first:** browsing, editing, and querying already-synced/local files must work with zero network; sync resumes automatically when connectivity returns.
- **Security:** all peer traffic encrypted (QUIC/TLS via iroh); local auth passwords hashed (Argon2id); no telemetry sent anywhere by default.
- **Performance:** opening a multi-MB Excel file for Form mode must not require loading the whole grid UI; only the detected table range is parsed into the form.
- **Installability:** single installer per platform, no separate services, databases, or daemons to configure by hand.

## 14. Technology stack summary

| Concern | Choice |
|---|---|
| App shell | Tauri 2 (Rust + WebView) |
| Frontend | React + TypeScript |
| Spreadsheet grid UI | Fortune-sheet or Univer client engine |
| Code/SQL editor UI | Monaco |
| Networking / NAT traversal | iroh (+ iroh-blobs, iroh-gossip) |
| Local HTTP/WS server | axum |
| Excel read/write | umya-spreadsheet |
| SQL engine | DuckDB (duckdb-rs) |
| Local database | SQLite (auth, peers, shares, locks, activity, file index) |
| Password hashing | Argon2id |
| Full-text search (optional) | tantivy |
