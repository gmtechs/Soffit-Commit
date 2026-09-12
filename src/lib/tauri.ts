import { invoke, isTauri as isTauriRuntime } from "@tauri-apps/api/core";

/** True only when the UI is hosted by the Tauri desktop runtime. */
export const isTauri = () => isTauriRuntime();

// ── Core types ────────────────────────────────────────────────────────────────
export interface User { id: string; username: string; created_at: string; }
export interface Peer { id: string; node_id: string; display_name: string; public_key: string; trust_status: string; last_seen: string | null; is_online: boolean; }
export interface Share { id: string; path: string; display_name: string; is_owner: boolean; selective_sync: boolean; created_at: string; }
export interface SharePermission { id: string; share_id: string; peer_id: string; level: "none" | "view" | "edit"; updated_at: string; }
export interface Lock { id: string; file_path: string; held_by_peer_id: string; held_by_name: string; acquired_at: string; expires_at: string; }
export interface ActivityEntry { id: string; timestamp: string; actor: string; action: string; target: string; metadata: string | null; }
export interface FileIndex { id: string; share_id: string; relative_path: string; size_bytes: number; modified_at: string | null; content_hash: string | null; file_kind: FileKind; sync_status: SyncStatus; }
export type FileKind = "excel" | "sql" | "csv" | "sqlite" | "parquet" | "image" | "pdf" | "text" | "generic";
export type SyncStatus = "synced" | "syncing" | "conflict" | "locked" | "pending";
export interface DashboardStats {
  storage_used_bytes: number; files_synced: number; files_edited_this_month: number;
  conflicts_resolved: number; sync_health_percent: number;
  file_type_breakdown: { excel: number; sql: number; other: number };
  online_peers: number; total_peers: number;
  storage_delta_pct: number; files_synced_delta_pct: number;
  files_edited_delta_pct: number; conflicts_delta_pct: number;
}
export interface PairingCodeWithQr { id: string; code: string; short_code: string; created_at: string; expires_at: string; qr_base64: string; node_id: string | null; }

// Excel types
export type CellValue = string | number | boolean | null;
export interface SheetInfo { index: number; name: string; }
export interface MergeCell { r: number; c: number; rs: number; cs: number; }
export interface SheetData {
  sheet_name: string;
  rows: CellValue[][];
  max_row: number;
  max_col: number;
  merge_cells: MergeCell[];
  col_widths: [number, number][];
  row_heights: [number, number][];
}
export interface FormLayout {
  sheet_index: number; header_row: number; data_start_row: number; data_end_row: number;
  col_start: number; col_end: number; headers: string[]; total_records: number;
  detectable: boolean; disable_reason: string | null;
}
export interface FormField { col: number; header: string; value: CellValue; detected_type: "text" | "number" | "date" | "bool"; }
export interface FormRecord { record_index: number; row: number; fields: FormField[]; }

// SQL types
export interface ColumnInfo { name: string; data_type: string; }
export interface QueryResult { columns: ColumnInfo[]; rows: unknown[][]; rows_affected: number | null; error: string | null; execution_ms: number; }

// Version types
export interface Version { id: string; file_path: string; hash: string; size_bytes: number; created_at: string; actor: string; snapshot_path: string; }

// Conflict types
export interface Conflict { id: string; file_path: string; local_hash: string; remote_hash: string; remote_peer_id: string; detected_at: string; resolved: boolean; resolution: string | null; }

// Search types
export interface SearchResult { file_path: string; share_id: string; score: number; snippet: string | null; }

// ── IMPORTANT: Tauri 2 converts Rust snake_case params to camelCase on the JS side.
// All invoke() calls must use camelCase keys matching the Rust param names after conversion.

// ── Auth ───────────────────────────────────────────────────────────────────────
const WEB_USERS_KEY = "soffit-web-users";
type WebUser = User & { password_hash: string };
const webUsers = (): WebUser[] => {
  try { return JSON.parse(localStorage.getItem(WEB_USERS_KEY) ?? "[]") as WebUser[]; }
  catch { return []; }
};
const saveWebUsers = (users: WebUser[]) => localStorage.setItem(WEB_USERS_KEY, JSON.stringify(users));
const bytesToHex = (bytes: Uint8Array) =>
  Array.from(bytes).map(value => value.toString(16).padStart(2, "0")).join("");

/**
 * SHA-256 fallback for older/insecure WebViews where Web Crypto is absent.
 * It deliberately returns the same digest format as crypto.subtle so existing
 * browser-only accounts remain usable. Desktop accounts always use Argon2.
 */
const sha256Fallback = (message: string): string => {
  const input = new TextEncoder().encode(message);
  const bitLength = input.length * 8;
  const paddedLength = ((input.length + 9 + 63) >> 6) << 6;
  const bytes = new Uint8Array(paddedLength);
  bytes.set(input);
  bytes[input.length] = 0x80;
  const view = new DataView(bytes.buffer);
  view.setUint32(bytes.length - 4, bitLength >>> 0, false);
  view.setUint32(bytes.length - 8, Math.floor(bitLength / 0x1_0000_0000), false);

  const constants = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
  ];
  let h0 = 0x6a09e667, h1 = 0xbb67ae85, h2 = 0x3c6ef372, h3 = 0xa54ff53a;
  let h4 = 0x510e527f, h5 = 0x9b05688c, h6 = 0x1f83d9ab, h7 = 0x5be0cd19;
  const words = new Uint32Array(64);
  for (let offset = 0; offset < bytes.length; offset += 64) {
    for (let i = 0; i < 16; i++) words[i] = view.getUint32(offset + i * 4, false);
    for (let i = 16; i < 64; i++) {
      const a = words[i - 15]; const b = words[i - 2];
      words[i] = (((a >>> 7) | (a << 25)) ^ ((a >>> 18) | (a << 14)) ^ (a >>> 3))
        + words[i - 16] + (((b >>> 17) | (b << 15)) ^ ((b >>> 19) | (b << 13)) ^ (b >>> 10)) + words[i - 7];
    }
    let [a, b, c, d, e, f, g, h] = [h0, h1, h2, h3, h4, h5, h6, h7];
    for (let i = 0; i < 64; i++) {
      const s1 = ((e >>> 6) | (e << 26)) ^ ((e >>> 11) | (e << 21)) ^ ((e >>> 25) | (e << 7));
      const choice = (e & f) ^ (~e & g);
      const temp1 = (h + s1 + choice + constants[i] + words[i]) >>> 0;
      const s0 = ((a >>> 2) | (a << 30)) ^ ((a >>> 13) | (a << 19)) ^ ((a >>> 22) | (a << 10));
      const majority = (a & b) ^ (a & c) ^ (b & c);
      const temp2 = (s0 + majority) >>> 0;
      [h, g, f, e, d, c, b, a] = [g, f, e, (d + temp1) >>> 0, c, b, a, (temp1 + temp2) >>> 0];
    }
    h0 = (h0 + a) >>> 0; h1 = (h1 + b) >>> 0; h2 = (h2 + c) >>> 0; h3 = (h3 + d) >>> 0;
    h4 = (h4 + e) >>> 0; h5 = (h5 + f) >>> 0; h6 = (h6 + g) >>> 0; h7 = (h7 + h) >>> 0;
  }
  return [h0, h1, h2, h3, h4, h5, h6, h7].map(value => value.toString(16).padStart(8, "0")).join("");
};

const hashWebPassword = async (password: string) => {
  const bytes = new TextEncoder().encode(password);
  if (globalThis.crypto?.subtle) {
    const digest = await globalThis.crypto.subtle.digest("SHA-256", bytes);
    return bytesToHex(new Uint8Array(digest));
  }
  return sha256Fallback(password);
};

/** Browser mode intentionally keeps accounts in this browser only. Desktop accounts remain encrypted in its local database. */
export const userExists = () => isTauri()
  ? invoke<boolean>("cmd_user_exists")
  : Promise.resolve(webUsers().length > 0);
export const createUser = async (username: string, password: string): Promise<User> => {
  if (isTauri()) return invoke<User>("cmd_create_user", { username, password });
  const clean = username.trim();
  if (!clean) throw new Error("Enter a username");
  if (webUsers().some(user => user.username.toLowerCase() === clean.toLowerCase())) throw new Error("That username already exists");
  const id = globalThis.crypto?.randomUUID?.() ?? `web-${Date.now()}-${Math.random().toString(36).slice(2)}`;
  const user: WebUser = { id, username: clean, password_hash: await hashWebPassword(password), created_at: new Date().toISOString() };
  saveWebUsers([...webUsers(), user]);
  const { password_hash: _passwordHash, ...publicUser } = user;
  return publicUser;
};
export const login = async (username: string, password: string): Promise<User> => {
  if (isTauri()) return invoke<User>("cmd_login", { username, password });
  if (webUsers().length === 0) {
    throw new Error("No browser account exists on this device. Desktop accounts, including manuh, stay on the desktop app. Open Soffit Commit in desktop mode to sign in.");
  }
  const passwordHash = await hashWebPassword(password);
  const user = webUsers().find(candidate => candidate.username.toLowerCase() === username.trim().toLowerCase() && candidate.password_hash === passwordHash);
  if (!user) throw new Error("Invalid username or password");
  const { password_hash: _passwordHash, ...publicUser } = user;
  return publicUser;
};
export const listUsers = () => invoke<User[]>("cmd_list_users");
export const updateUser = (userId: string, username: string, password?: string) =>
  invoke<User>("cmd_update_user", { userId, username, password });

// ── Pairing ────────────────────────────────────────────────────────────────────
export const generatePairingCode = () => invoke<PairingCodeWithQr>("cmd_generate_pairing_code");
export const consumePairingCode = (code: string, displayName: string) => invoke<string | null>("cmd_consume_pairing_code", { code, displayName });
export const addPeer = (nodeId: string, displayName: string, publicKey: string, endpointAddr?: string) =>
  invoke<Peer>("cmd_add_peer", { nodeId, displayName, publicKey, endpointAddr });
export const reconnectPeers = () => invoke<number>("cmd_reconnect_peers");

// ── Peers ──────────────────────────────────────────────────────────────────────
export const listPeers = () => invoke<Peer[]>("cmd_list_peers");
export const removePeer = (peerId: string) => invoke<void>("cmd_remove_peer", { peerId });
export const renamePeer = (peerId: string, newName: string) => invoke<void>("cmd_rename_peer", { peerId, newName });
export const setPermission = (shareId: string, peerId: string, level: string) =>
  invoke<SharePermission>("cmd_set_permission", { shareId, peerId, level });

// ── Shares ─────────────────────────────────────────────────────────────────────
export const createShare = (path: string, displayName: string) =>
  invoke<Share>("cmd_create_share", { path, displayName });
export const listShares = () => invoke<Share[]>("cmd_list_shares");
export const deleteShare = (shareId: string) => invoke<void>("cmd_delete_share", { shareId });
export const listFiles = (shareId: string) => invoke<FileIndex[]>("cmd_list_files", { shareId });
export const refreshShare = (shareId: string) => invoke<void>("cmd_refresh_share", { shareId });
export const setSelectiveSync = (shareId: string, enabled: boolean) =>
  invoke<void>("cmd_set_selective_sync", { shareId, enabled });

// ── Locks ──────────────────────────────────────────────────────────────────────
export const acquireLock = (filePath: string, peerId: string, peerName: string) =>
  invoke<Lock>("cmd_acquire_lock", { filePath, peerId, peerName });
export const releaseLock = (filePath: string, peerId: string) =>
  invoke<boolean>("cmd_release_lock", { filePath, peerId });
export const getLock = (filePath: string) => invoke<Lock | null>("cmd_get_lock", { filePath });
export const listActiveLocks = () => invoke<Lock[]>("cmd_list_active_locks");

// ── Activity ───────────────────────────────────────────────────────────────────
export const listActivity = (limit?: number) => invoke<ActivityEntry[]>("cmd_list_activity", { limit });

// ── Dashboard ──────────────────────────────────────────────────────────────────
export const dashboardStats = () => invoke<DashboardStats>("cmd_dashboard_stats");

// ── Settings ───────────────────────────────────────────────────────────────────
export const getSetting = (key: string) => invoke<string | null>("cmd_get_setting", { key });
export const setSetting = (key: string, value: string) => invoke<void>("cmd_set_setting", { key, value });

// ── Excel ──────────────────────────────────────────────────────────────────────
export const excelOpen = (path: string) => invoke<SheetInfo[]>("cmd_excel_open", { path });
export const excelGetSheet = (path: string, sheetIndex: number) =>
  invoke<SheetData>("cmd_excel_get_sheet", { path, sheetIndex });
export const excelSetCell = (path: string, sheetIndex: number, row: number, col: number, value: string) =>
  invoke<void>("cmd_excel_set_cell", { path, sheetIndex, row, col, value });
export const excelDetectForm = (path: string, sheetIndex: number) =>
  invoke<FormLayout>("cmd_excel_detect_form", { path, sheetIndex });
export const excelGetRecord = (path: string, layout: FormLayout, recordIndex: number) =>
  invoke<FormRecord>("cmd_excel_get_record", { path, layout, recordIndex });
export const excelSaveRecord = (path: string, layout: FormLayout, recordIndex: number, updates: [number, string][]) =>
  invoke<string[]>("cmd_excel_save_record", { path, layout, recordIndex, updates });

// ── SQL ────────────────────────────────────────────────────────────────────────
export const sqlExecute = (sql: string) => invoke<QueryResult>("cmd_sql_execute", { sql });
export const sqlQueryFile = (filePath: string, sql: string) =>
  invoke<QueryResult>("cmd_sql_query_file", { filePath, sql });
export const sqlAttachFile = (filePath: string, alias: string) =>
  invoke<void>("cmd_sql_attach_file", { filePath, alias });
export const sqlGenerateEdit = (table: string, pkCol: string, pkVal: unknown, changes: [string, unknown][]) =>
  invoke<string>("cmd_sql_generate_edit", { table, pkCol, pkVal, changes });

// ── Versions ───────────────────────────────────────────────────────────────────
export const listVersions = (filePath: string) => invoke<Version[]>("cmd_list_versions", { filePath });
export const restoreVersion = (versionId: string) => invoke<string>("cmd_restore_version", { versionId });

// ── Conflicts ──────────────────────────────────────────────────────────────────
export const listConflicts = () => invoke<Conflict[]>("cmd_list_conflicts");
export const resolveConflict = (conflictId: string, resolution: "keep_mine" | "keep_theirs" | "keep_both") =>
  invoke<void>("cmd_resolve_conflict", { conflictId, resolution });

// ── Search ─────────────────────────────────────────────────────────────────────
export const search = (query: string, limit?: number) => invoke<SearchResult[]>("cmd_search", { query, limit });
export const indexFile = (shareId: string, relPath: string, absPath: string) =>
  invoke<void>("cmd_index_file", { shareId, relPath, absPath });

// ── Network ────────────────────────────────────────────────────────────────────
export const getNodeId = () => invoke<string | null>("cmd_get_node_id");
export const getNodeTicket = () => invoke<string | null>("cmd_get_node_ticket");

// ── File history & favourites ──────────────────────────────────────────────────
export interface FileHistoryEntry { id: string; file_path: string; opened_at: string; file_kind: string; }
export interface FileFavourite { id: string; file_path: string; display_name: string; file_kind: string; added_at: string; }
export const recordFileOpen = (filePath: string, fileKind: string) => invoke<void>("cmd_record_file_open", { filePath, fileKind });
export const getFileHistory = (limit?: number) => invoke<FileHistoryEntry[]>("cmd_get_file_history", { limit });
export const addFavourite = (filePath: string, displayName: string, fileKind: string) => invoke<void>("cmd_add_favourite", { filePath, displayName, fileKind });
export const removeFavourite = (filePath: string) => invoke<void>("cmd_remove_favourite", { filePath });
export const getFavourites = () => invoke<FileFavourite[]>("cmd_get_favourites");
export const readFileBase64 = (filePath: string) => invoke<string>("cmd_read_file_base64", { filePath });
export const readFileText = (filePath: string) => invoke<string>("cmd_read_file_text", { filePath });
export const writeFileBytes = (filePath: string, data: number[]) => invoke<void>("cmd_write_file_bytes", { filePath, data });

// ── SQL script / dump ──────────────────────────────────────────────────────────
export interface ScriptStatementResult { statement_index: number; line_number: number; sql_snippet: string; result: QueryResult; }
export interface ImportError { statement_index: number; original_line_number: number; snippet: string; message: string; }
export interface ImportResult { statements_executed: number; error: ImportError | null; }
export const sqlExecuteScript = (sql: string) => invoke<ScriptStatementResult[]>("cmd_sql_execute_script", { sql });
export const sqlImportDump = (filePath: string) => invoke<ImportResult>("cmd_sql_import_dump", { filePath });

// ── AI ─────────────────────────────────────────────────────────────────────────
export type ModelStatus = "not_downloaded" | { downloading: { bytes_done: number; bytes_total: number } } | "ready" | { error: string };
export interface AiStatus { chat_model: ModelStatus; embed_model: ModelStatus; chat_size_bytes: number; embed_size_bytes: number; }
export interface AiTextResult { text: string; guarded: boolean; }
export const aiStatus = () => invoke<AiStatus>("cmd_ai_status");
export const aiDownloadModels = () => invoke<void>("cmd_ai_download_models");
export const aiRemoveModels = () => invoke<void>("cmd_ai_remove_models");
export const aiExplainConflict = (filePath: string, localMeta: string, remoteMeta: string, diffSnippet?: string) =>
  invoke<AiTextResult>("cmd_ai_explain_conflict", { filePath, localMeta, remoteMeta, diffSnippet });
export const aiActivitySummary = (timeWindow: string) =>
  invoke<AiTextResult>("cmd_ai_activity_summary", { timeWindow });
export const aiDataInsights = (statsJson: string) =>
  invoke<AiTextResult>("cmd_ai_data_insights", { statsJson });
export const aiChatDocument = (filePath: string, fileKind: string, question: string, documentText?: string) =>
  invoke<AiTextResult>("cmd_ai_chat_document", { filePath, fileKind, question, documentText });
