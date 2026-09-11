# Soffit Commit — UI Specification

This spec replicates the structure and visual language of the supplied reference dashboard, restyled with the supplied brand tokens, and adapted to Soffit Commit's actual features (file access, peers, Excel, SQL) instead of a financial dashboard's content.

## 1. Design tokens

**Typography**
- Font family: `Inter`, fallback `system-ui, sans-serif`.
- Weights used: 400 (body), 500 (labels, nav items), 600 (card values / emphasis), 700 (page headings only).

**Color palette**

| Token | Hex | Usage |
|---|---|---|
| `--color-primary` | `#E27100` (Golden Flame) | Primary accent: active nav pill, primary buttons, active states, key data highlights |
| `--color-ink` | `#2D2C2A` (Obsidian) | Primary text, dark buttons (e.g. sidebar CTA), sidebar active-item text |
| `--color-bg` | `#F4F3F0` | App/page background |
| `--color-surface` | `#FFFFFF` | Cards, sidebar, topbar |
| `--color-border` | `#E9E7E2` | Card borders, dividers, table row separators |
| `--color-text-secondary` | `#6E6D68` | Subtitles, helper text, table secondary column text |
| `--color-text-muted` | `#A6A49D` | Placeholder text, disabled state |
| `--color-success` | `#16A34A` | Positive delta, "Synced" status |
| `--color-danger` | `#DC2626` | Negative delta, "Conflict" status, destructive actions |
| `--color-info` | `#3B82F6` | Secondary chart series, informational badges |
| `--color-warning` | `#F59E0B` | "Locked"/pending status |

**Radius & elevation**
- Card radius: `16px`. Small controls (buttons, inputs, pills): `10px`. Fully round: avatars, status dots, the central donut-chart label.
- Cards use a single soft shadow: `0 1px 2px rgba(0,0,0,0.04), 0 8px 24px rgba(0,0,0,0.04)`. No borders **and** shadows both — border for flat cards inside dense areas (e.g. table), shadow for top-level cards on the dashboard.

**Spacing**
- Base unit 4px. Page gutter 24px. Card internal padding 20–24px. Gap between dashboard cards 20px.

**Iconography**
- Outline-style icon set throughout (Tabler Icons or Phosphor, outline variant), 20px in nav/table rows, 18px inline in cards. No filled icons, no emoji.

## 2. Global layout

Three fixed regions, matching the reference image's skeleton:

```
┌───────────┬──────────────────────────────────────────┐
│           │  Topbar (greeting, search, actions,       │
│  Sidebar  │  profile)                                 │
│  260px    ├──────────────────────────────────────────┤
│  white    │                                            │
│           │   Main content (scrollable), --color-bg   │
│           │   background, grid of cards                │
└───────────┴──────────────────────────────────────────┘
```

- Sidebar: fixed width 260px, white surface, full height, no shadow (separated from content by a 1px `--color-border` line).
- Topbar: 72px tall, white surface, sits above content, bottom border `--color-border`.
- Content area: background `--color-bg`, 24px padding, cards arranged in a responsive grid (see §4).

## 3. Sidebar

Top to bottom, exactly mirroring the reference:

1. **Brand row** — logo mark + "Soffit Commit" wordmark, 24px from top, Inter 600, 18px.
2. **Section label** — "Main menu", 12px, `--color-text-muted`, uppercase, letter-spacing 0.04em.
3. **Nav items** (icon + label, 44px row height, 12px radius):
   - Home (dashboard) — icon `layout-grid`
   - Files — icon `folder`
   - Peers — icon `users` (badge showing count of pending pairing requests, red circle, white number — same treatment as the reference's message badge)
   - Excel — icon `table`
   - SQL — icon `terminal-2` / `database`
   - Activity — icon `activity`
   - The currently active item gets the **filled orange pill** treatment exactly like the reference: `--color-primary` background, white text/icon, full-width rounded rect (radius 10px).
   - Inactive items: `--color-ink` text at 80% opacity, transparent background, hover = `--color-bg` background.
4. **Section label** — "General".
5. **Secondary nav items** (same row style, no badges): Help & Docs, Settings, Sign out (sign out row uses `--color-danger` text, matching the reference's red "Log Out").
6. **Bottom CTA card** — pinned above the sidebar's bottom edge, replaces the reference's "Upgrade to Pro" card:
   - Icon/badge (small circular icon, orange), heading "Storage", one line of helper text ("2.4 GB used across 3 devices" — dynamic), and a dark (`--color-ink` background, white text) full-width button labeled **Manage devices**.

## 4. Topbar

Left to right:

- **Greeting block**: "Welcome back, {first name}" (20px, 600) + subtitle line "{N} files synced today" or similar live status (14px, `--color-text-secondary}`) — same two-line pattern as the reference's "Welcome Back, Bennett / New opportunities are ready in your pipeline."
- **Search bar** (center-right, pill shape, `--color-bg` fill, 40px tall, search icon left-aligned, placeholder "Search files, peers…").
- **Icon buttons**: notification bell (with unread-dot), pairing/share icon.
- **Profile chip**: circular avatar, name + status line ("Online" in `--color-success` dot + text), small chevron to open the account menu — matching the reference's avatar+name+email+chevron block exactly in structure (swap email line for online/offline status).

## 5. Dashboard (home) screen

Same card grid rhythm as the reference, content re-mapped to this product:

### Row 1 — four stat cards (2 narrow + 1 wide, matching the reference's Current Balance / Total Savings / Total Revenue arrangement)
- **Storage used** — icon `database`, big value ("18.4 GB"), delta vs last week, "Details →" link.
- **Files synced** — icon `refresh`, big value, delta vs last week, "Details →" link.
- **Sync activity** (wide card, spans the remaining width like "Total Revenue"): a smoothed area/flow chart of bytes transferred per day over the last 6 months, with a callout bubble showing the peak day's percentage-of-capacity, exactly mirroring the reference's revenue flow chart treatment (two-tone orange→blue gradient fill, dashed vertical marker on the peak month, month labels along the x-axis).

### Row 2 — two more stat cards (mirrors Total Income / Total Expenses)
- **Files edited this month** — positive delta in green.
- **Conflicts resolved** — negative-framed delta in red if conflicts rose.

### Row 3 — three cards (mirrors Retention Rate / Leads Status / Schedule)
- **Sync health** — radial gauge (same half-donut-of-dashes style as the reference's "Retention Rate"), showing % of shares fully up to date, needle pointer, 0–100 scale labeled underneath.
- **File types** — donut chart (same style as "Leads Status"), segments = Excel / SQL / Other, center label = total file count, legend row below with colored dots (orange/blue/green matching the reference).
- **Connected peers** — reuses the reference's "Schedule" card shape: a horizontal mini date/peer-status strip at top, and below it a "next scheduled sync" or "currently active peer" entry with avatar stack and a small "View all" chevron link, exactly like the reference's "Mesh Weekly Meeting" row.

### Row 4 — data table ("Shared files"), replacing "Lead Conversion Tracker"
Same table chrome as the reference: title left, search input + Filter button right, checkbox column, then columns:

| ☐ | File | Shared with | Size | Status | Modified | Synced | Actions |
|---|---|---|---|---|---|---|---|
| ☐ | icon + filename | avatar(s) of peers with access | e.g. 4.2 MB | pill: Synced (green) / Locked (amber, shows who) / Conflict (red) | date | date | ⋯ menu |

Row height, avatar treatment, sortable column chevrons, and the "⋯" row-actions menu all match the reference table exactly.

## 6. Files screen

- Breadcrumb path bar at top (mirrors a folder path), toggle between **grid** and **list** view (icons top-right, same icon-button style as topbar).
- Each file/folder card or row: type icon (folder / xlsx / sql / generic-file), name, size, status pill (Synced/Locked/Conflict — same pill component as the dashboard table), and a lock icon with the holder's avatar when applicable.
- Right-click / "⋯" menu: Open, Open in Excel mode / SQL mode (context-sensitive), Share, Version history, Rename, Delete.

## 7. Excel mode screen

- Top toolbar: file name + save state, a segmented control **Grid | Form** (this is the mode toggle from the technical spec), sheet tabs along the bottom like real Excel, formula bar directly under the toolbar when in Grid mode.
- **Grid mode**: full spreadsheet component, cell selection, formula bar bound to selected cell, standard formatting toolbar (bold/italic/border/number-format/merge).
- **Form mode**: centered card containing one record's fields (label = detected column header, input = type-appropriate control), Prev/Next controls, a record counter ("Record 12 of 340"), and a **Jump to row** field. Validation errors show inline under the offending field in `--color-danger`, matching the app's global form-error styling.
- A slim right-hand panel (collapsible) shows "Who else has this open" (avatars) and the lock status.

## 8. SQL screen

- Left: file/table browser (attached databases and files).
- Center: Monaco SQL editor with a **Run** button (primary orange) and a secondary **Format** button.
- Bottom (resizable split): results grid, with an **Apply changes** button appearing only when the result set is edited, opening a confirmation diff (matching the technical spec's "show generated SQL before writing").

## 9. Peers / pairing screen

- **Add device** button (top-right, primary orange) opens a modal with a large QR code + short alphanumeric code, and a matching "Enter a code" input for pairing from the other side.
- Peer list as rows: avatar/initial, device name, online/offline dot, and a permission dropdown per shared item (No Access / View / Edit) — dropdown uses the same pill/segmented styling as status badges elsewhere.

## 10. Shared components

- **Button** — primary (orange fill, white text), secondary (white fill, `--color-border` outline, ink text), dark (ink fill, white text — used for the sidebar CTA), destructive (red text, transparent, used for Sign out / Delete).
- **StatCard** — icon chip (soft-orange circular background, orange icon) + title + "⋯" menu, big value line, small delta pill (green/red arrow + %), optional "Details →" link.
- **StatusPill** — rounded-full, colored dot + label: Synced (green), Locked (amber), Conflict (red), No access (gray).
- **DataTable** — checkbox column, sortable headers (chevron up/down icon), zebra-free white rows separated by `--color-border` hairlines, hover state = `--color-bg` row tint.
- **Modal** — centered, white, 16px radius, same shadow as cards, dimmed backdrop.
- **Toast/notification** — bottom-right, white card, colored left accent bar matching the message type (success/warning/danger).

## 11. States

- **Empty states**: centered icon + one-line message + primary action button (e.g., empty Files screen → "No shares yet" + "Add device" button).
- **Loading**: skeleton blocks matching each component's exact footprint (never a generic spinner over the whole page).
- **Offline peer**: peer avatar shown at 40% opacity with a small gray dot instead of green.
- **Conflict**: red banner at the top of the affected file's view with "Keep mine / Keep theirs / Keep both" actions, per the technical spec's conflict handling.

## 12. Responsive behavior

- Below 1100px: sidebar collapses to icon-only (64px wide, labels in tooltip on hover).
- Below 800px: dashboard cards stack to a single column in the same top-to-bottom order described in §5; the data table becomes a stacked card list (one card per file) instead of columns.

## 13. Dark mode

- All tokens above have dark equivalents already implied by their semantic names (`--color-bg`, `--color-surface`, etc.) — implement via CSS variables so a dark theme only requires swapping the variable values (surface → near-black, bg → dark ink `#1C1B19`, border → low-contrast dark gray, text tokens inverted), keeping `--color-primary` (Golden Flame) as the one constant accent across both themes.
