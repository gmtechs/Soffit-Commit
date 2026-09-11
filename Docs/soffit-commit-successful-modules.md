# Soffit Commit — Successful Modules

Purpose: for each module, describe what it *feels like* when it's genuinely working, plus concrete, checkable behaviors. Use this to validate a build, not just to plan one — every bullet under "Acceptance behavior" should be something you can actually try and see pass or fail.

---

## 1. Identity & pairing

**Feels like:** Installing the app on a second computer and being connected to your first one within two minutes, without touching a router, a firewall, or typing an IP address anywhere.

**Acceptance behavior**
- First launch generates a device identity automatically; nothing about it is user-visible or user-configurable unless they dig into Settings.
- "Add device" always produces both a QR code and a short text code, valid for a limited time, single-use.
- Pairing succeeds when both computers are on the same WiFi, and also when one is on a different network entirely (e.g. mobile hotspot vs. home broadband) — no setting has to change between the two cases.
- Once paired, both apps show each other as a peer immediately, even before any share exists.
- Restarting either app reconnects to already-paired peers automatically with zero user action.
- Un-pairing a device on one side revokes that device's access everywhere; the other side sees it move to "removed" rather than silently going offline.

## 2. Peer & permission management

**Feels like:** Confidently controlling exactly who can touch what, and always being able to see, at a glance, who currently has access to a given folder.

**Acceptance behavior**
- Every share has a per-peer permission of No Access / View / Edit, defaulting to No Access for newly paired devices.
- Changing a permission takes effect on an online peer within a couple of seconds, and is enforced — not just hidden in the UI. A peer downgraded to View cannot push an edit even by crafting a request directly.
- A peer with No Access cannot see the share exists at all (not even its name).
- Permission changes are visible in the Activity feed with who changed what, when.

## 3. File browser & generic file access

**Feels like:** A normal file manager that happens to also show you what's shared, with whom, and its sync state — nothing feels foreign compared to Finder/Explorer.

**Acceptance behavior**
- Any file type can be browsed, renamed, moved, deleted (within permission), and opened with the OS's default application.
- File list shows accurate size, modified date, and sync status without requiring a manual refresh.
- Opening an unsupported file type never errors out silently — it either previews, or opens externally, or explains clearly why not (e.g. permission).
- Folder structure of a share is preserved exactly as it exists on the owning device — no flattening, no renaming.

## 4. Sync engine (transfer, locks, conflicts)

**Feels like:** Trusting that when you open a file, you're looking at the current version, and when you save, the other side will get it — without ever wondering "did that actually go through?"

**Acceptance behavior**
- Opening a file for editing visibly locks it for other peers within a second or two; they see who holds the lock.
- Releasing the lock (closing the file, or saving and closing) frees it for others immediately.
- An idle lock auto-releases after the configured timeout, with the holder warned before it happens.
- Large files transfer incrementally and resume correctly if the connection drops mid-transfer — no restart from zero.
- Two edits that genuinely diverge (e.g. both sides edited while offline) are caught on reconnect and presented as a conflict, never silently overwritten.
- All of this keeps working with zero configuration when the two computers are behind different NATs — the user never sees a "connection failed, check your firewall" dead end for a case iroh's relay fallback should have handled.

## 5. Excel module

**Feels like:** Opening a real spreadsheet and it just working — formulas intact, formatting intact — plus the option to fill it in like a form when that's faster than clicking cells.

**Acceptance behavior**
- Any valid `.xlsx` file opens correctly regardless of its layout: multiple sheets, merged cells, non-standard starting position of the data table, existing formulas and conditional formatting.
- Grid mode: editing a cell, applying formatting, and adding a formula behaves like a normal spreadsheet, with working undo/redo.
- Form mode: the header row and data range are detected automatically; the user is never asked to manually describe their file's shape before Form mode becomes available.
- Switching Grid ↔ Form on the same open file never loses unsaved changes and never corrupts formatting elsewhere in the sheet.
- Saving from Form mode only changes the cells that were actually edited — a colleague opening the file in real Excel afterward sees no unexpected formatting or formula changes anywhere else.
- If a sheet has no detectable header/table (e.g. a free-form layout), Form mode is clearly disabled with an explanation, rather than generating a nonsense form.
- Two peers can't silently overwrite each other's spreadsheet edits — this is enforced by the same lock model as §4, visible inside the Excel screen itself (not just the file browser).

## 6. SQL module

**Feels like:** A lightweight, fast SQL console that works on whatever tabular file you point it at, without installing or configuring a database server.

**Acceptance behavior**
- Opening a `.sql` file shows syntax highlighting and runs against the intended target with one click.
- Querying a `.csv`, `.parquet`, or `.sqlite` file directly (no import step) returns correct results.
- Editing a result grid and applying changes always shows the exact SQL that will run before it runs — never a silent write.
- Query errors show the actual database error message, not a generic failure.
- Running the same query twice with no underlying data change returns identical results (no hidden caching bugs).

## 7. Authentication

**Feels like:** A normal app login — quick, local, and clearly separate from the "which computers are paired" concept.

**Acceptance behavior**
- First run prompts to create a local password; it is never stored or transmitted in plaintext anywhere, including logs.
- Wrong password is rejected with a clear message and no hint about whether the username/account exists.
- Optional idle-timeout re-lock works exactly as configured (e.g. locks after 15 minutes idle if that's the setting).
- Losing the local password without a recovery mechanism configured is explained plainly — the user is told up front what happens if they forget it (no silent data loss, no false promise of recovery that isn't implemented).

## 8. Activity feed & notifications

**Feels like:** Never having to wonder "did that sync happen" or "did they see my share request" — the app tells you.

**Acceptance behavior**
- Every share, permission change, lock, unlock, sync completion, and conflict appears in the Activity feed, newest first, with a human-readable description ("Alice locked budget.xlsx", not a raw event code).
- A toast/notification appears for events relevant to the current user without needing the Activity screen open.
- The unread/pending count shown in the sidebar (e.g. pending pairing requests) always matches what's actually pending — never stale.

## 9. Dashboard (home)

**Feels like:** Opening the app and immediately knowing the state of your whole setup — what's synced, what needs attention, who's online — without clicking into anything.

**Acceptance behavior**
- Every number shown (storage used, files synced, sync health, file-type breakdown) reflects real current state, not placeholder or stale cached values, and updates without a manual refresh when something changes.
- Clicking any stat card's "Details" link takes the user to the exact underlying data (e.g. the filtered file list) — never a dead link.
- The connected-peers panel accurately reflects who is online right now.

## 10. Settings & device management

**Feels like:** A single, calm place to see every paired device, every share, and every setting — nothing scattered across multiple screens.

**Acceptance behavior**
- Listing all paired devices, with the ability to rename or remove any of them, is available from one screen.
- Changing the local sync-cache location, lock timeout, or theme (light/dark) takes effect immediately without a restart.
- Removing a device from Settings has the exact same effect described in §1 (immediate access revocation), not a softer/different behavior reached from a different path.
