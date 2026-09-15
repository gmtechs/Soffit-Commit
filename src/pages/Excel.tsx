import React, { useState, useCallback, useEffect, useMemo, useRef } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { Workbook } from "@fortune-sheet/react";
import "@fortune-sheet/react/dist/index.css";
import { FortuneExcelHelper, transformExcelToFortune, transformFortuneToExcel } from "@corbe30/fortune-excel";
import {
  FileSpreadsheet, FormInput, ChevronLeft, ChevronRight,
  Save, Lock, History, AlertTriangle, Grid3x3, MessageCircle,
} from "lucide-react";
import {
  excelOpen, excelDetectForm, excelGetRecord, excelSaveRecord,
  listVersions, restoreVersion,
  getLock, acquireLock, releaseLock, recordFileOpen,
  excelGetSheet, excelSetCell, writeFileBytes, readFileBase64,
  type FormLayout, type FormRecord, type CellValue, type Version, type SheetData, type SheetInfo,
} from "../lib/tauri";
import { useAuthStore } from "../store/auth";
import { useFileRouter } from "../store/fileRouter";
import { Button } from "../components/ui/Button";
import { useToast } from "../components/ui/Toast";
import { DocumentChat } from "../components/ui/DocumentChat";

type Mode = "grid" | "form";

function workbookText(sheets: any[] | null): string {
  if (!sheets) return "";
  return sheets.map((sheet, index) => {
    const cells = sheet.celldata ?? [];
    const values = cells.map((cell: any) => cell?.v?.m ?? cell?.v?.v ?? "").filter(Boolean).join(" | ");
    return `Sheet: ${sheet.name ?? `Sheet ${index + 1}`}\n${values}`;
  }).join("\n\n");
}

function cellStr(v: CellValue): string {
  if (v === null || v === undefined) return "";
  return String(v);
}

// Convert native SheetData (rows of CellValue) into FortuneSheet `celldata`.
// FortuneSheet stores cells as an array of { r, c, v: { v, m } } — one entry
// per non-empty cell, indexed from (0,0).
function sheetDataToFortune(d: SheetData): { r: number; c: number; v: { v: CellValue; m: string } }[] {
  const celldata: { r: number; c: number; v: { v: CellValue; m: string } }[] = [];
  for (let r = 0; r < d.rows.length; r++) {
    const row = d.rows[r] ?? [];
    for (let c = 0; c < row.length; c++) {
      const value = row[c];
      if (value !== null && value !== undefined && value !== "") {
        celldata.push({ r, c, v: { v: value, m: String(value) } });
      }
    }
  }
  return celldata;
}

function GridView({ path, sheetIndex, sheet, onSaved }: { path: string; sheetIndex: number; sheet: SheetData | null; onSaved: () => void }) {
  const [editing, setEditing] = useState<{ row: number; col: number; value: string } | null>(null);
  const [savingCell, setSavingCell] = useState(false);
  const { toast } = useToast();

  if (!sheet) return <div style={{ padding: 32, color: "var(--color-text-muted)" }}>Loading sheet…</div>;
  const displayRows = sheet.rows.slice(0, 200);
  const displayCols = Math.min(sheet.max_col, 50);
  const getValue = (value: CellValue) => value == null ? "" : String(value);
  const saveCell = async () => {
    if (!editing) return;
    setSavingCell(true);
    try {
      await excelSetCell(path, sheetIndex, editing.row + 1, editing.col + 1, editing.value);
      setEditing(null); onSaved();
    } catch (error) { toast("danger", `Could not save cell: ${error}`); }
    finally { setSavingCell(false); }
  };
  return <div style={{ flex: 1, overflow: "auto", background: "#fff" }}>
    <table style={{ borderCollapse: "collapse", minWidth: "100%", fontSize: 12 }}>
      <thead><tr><th style={gridHeaderStyle} />{Array.from({ length: displayCols }, (_, i) => <th key={i} style={gridHeaderStyle}>{String.fromCharCode(65 + (i % 26))}</th>)}</tr></thead>
      <tbody>{displayRows.map((row, rowIndex) => <tr key={rowIndex}>
        <th style={gridHeaderStyle}>{rowIndex + 1}</th>
        {Array.from({ length: displayCols }, (_, colIndex) => {
          const value = getValue(row[colIndex]); const active = editing?.row === rowIndex && editing.col === colIndex;
          return <td key={colIndex} style={{ border: "1px solid #e5e7eb", minWidth: 92, height: 28, padding: 0, background: active ? "#fffaf3" : "#fff" }} onDoubleClick={() => setEditing({ row: rowIndex, col: colIndex, value })}>
            {active ? <input autoFocus value={editing.value} onChange={e => setEditing({ ...editing, value: e.target.value })} onBlur={saveCell} onKeyDown={e => { if (e.key === "Enter") saveCell(); if (e.key === "Escape") setEditing(null); }} disabled={savingCell} style={{ width: "100%", height: "100%", minHeight: 27, border: "2px solid var(--color-primary)", padding: "3px 6px", outline: "none" }} /> : <span style={{ display: "block", padding: "5px 7px", whiteSpace: "pre-wrap", overflow: "hidden", textOverflow: "ellipsis", maxWidth: 220 }}>{value}</span>}
          </td>;
        })}</tr>)}</tbody>
    </table>
    {sheet.max_row > 200 && <p style={{ padding: 10, color: "var(--color-text-muted)" }}>Showing the first 200 rows.</p>}
  </div>;
}
const gridHeaderStyle: React.CSSProperties = { position: "sticky", top: 0, zIndex: 1, border: "1px solid #d1d5db", background: "#f6f6f4", color: "#6b6b66", fontWeight: 500, minWidth: 42, padding: "5px 7px", textAlign: "center" };

// ── Form view ─────────────────────────────────────────────────────────────────
function FormView({ path, layout }: { path: string; layout: FormLayout }) {
  const [recordIdx, setRecordIdx] = useState(0);
  const [record, setRecord] = useState<FormRecord | null>(null);
  const [edits, setEdits] = useState<Record<number, string>>({});
  const [errors, setErrors] = useState<string[]>([]);
  const [saving, setSaving] = useState(false);
  const { toast } = useToast();

  const loadRecord = useCallback(async (idx: number) => {
    try {
      const r = await excelGetRecord(path, layout, idx);
      setRecord(r); setEdits({}); setErrors([]);
    } catch (err: any) { toast("danger", String(err)); }
  }, [path, layout]);

  useEffect(() => { if (layout.detectable) loadRecord(recordIdx); }, [recordIdx, loadRecord]);

  const handleSave = async () => {
    if (!record) return;
    setSaving(true);
    try {
      const updates: [number, string][] = Object.entries(edits).map(([col, val]) => [Number(col), val]);
      const errs = await excelSaveRecord(path, layout, recordIdx, updates);
      if (errs.length > 0) setErrors(errs);
      else { toast("success", "Record saved"); setEdits({}); loadRecord(recordIdx); }
    } catch (err: any) { toast("danger", String(err)); }
    finally { setSaving(false); }
  };

  const jumpRef = useRef<HTMLInputElement>(null);

  if (!layout.detectable) {
    return (
      <div style={{ padding: 40, textAlign: "center", color: "var(--color-text-muted)" }}>
        <AlertTriangle size={32} style={{ margin: "0 auto 12px", color: "var(--color-warning)" }} />
        <p style={{ fontWeight: 600, marginBottom: 6 }}>Form mode unavailable</p>
        <p style={{ fontSize: 13 }}>{layout.disable_reason}</p>
      </div>
    );
  }

  return (
    <div style={{ maxWidth: 540, margin: "0 auto", padding: 24 }}>
      {/* Navigation */}
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 20, gap: 10 }}>
        <Button size="sm" variant="secondary" onClick={() => setRecordIdx(i => Math.max(0, i - 1))} disabled={recordIdx === 0}>
          <ChevronLeft size={14} /> Prev
        </Button>
        <div style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 13, color: "var(--color-text-secondary)" }}>
          <span>Record {recordIdx + 1} of {layout.total_records}</span>
          <input
            ref={jumpRef}
            type="number" min={1} max={layout.total_records}
            placeholder="Jump to…"
            onKeyDown={e => {
              if (e.key === "Enter") {
                const v = parseInt((e.target as HTMLInputElement).value);
                if (!isNaN(v)) setRecordIdx(Math.min(layout.total_records - 1, Math.max(0, v - 1)));
              }
            }}
            style={{ width: 80, padding: "4px 8px", borderRadius: 6, border: "1px solid var(--color-border)", fontSize: 12, background: "var(--color-bg)", outline: "none" }}
          />
        </div>
        <Button size="sm" variant="secondary" onClick={() => setRecordIdx(i => Math.min(layout.total_records - 1, i + 1))} disabled={recordIdx >= layout.total_records - 1}>
          Next <ChevronRight size={14} />
        </Button>
      </div>

      {/* Fields */}
      <div style={{ background: "var(--color-surface)", borderRadius: 12, padding: 20, border: "1px solid var(--color-border)", display: "flex", flexDirection: "column", gap: 14 }}>
        {record?.fields.map(field => (
          <div key={field.col}>
            <label style={{ fontSize: 12, fontWeight: 500, display: "block", marginBottom: 5, color: "var(--color-text-secondary)" }}>
              {field.header}
            </label>
            <input
              type={field.detected_type === "number" ? "number" : field.detected_type === "date" ? "date" : "text"}
              value={edits[field.col] !== undefined ? edits[field.col] : cellStr(field.value)}
              onChange={e => setEdits(prev => ({ ...prev, [field.col]: e.target.value }))}
              style={{
                width: "100%", padding: "8px 10px", borderRadius: 8, fontSize: 13, outline: "none", color: "var(--color-ink)",
                background: "var(--color-bg)",
                border: `1px solid ${errors.some(err => err.startsWith(field.header)) ? "var(--color-danger)" : "var(--color-border)"}`,
              }}
            />
            {errors.filter(err => err.startsWith(field.header)).map((err, i) => (
              <p key={i} style={{ fontSize: 11, color: "var(--color-danger)", marginTop: 3 }}>{err}</p>
            ))}
          </div>
        ))}
      </div>

      {Object.keys(edits).length > 0 && (
        <div style={{ marginTop: 16, display: "flex", gap: 10 }}>
          <Button variant="primary" onClick={handleSave} disabled={saving}>
            <Save size={14} /> {saving ? "Saving…" : "Save record"}
          </Button>
          <Button variant="secondary" onClick={() => { setEdits({}); setErrors([]); }}>Discard</Button>
        </div>
      )}
    </div>
  );
}

// ── Version history panel ─────────────────────────────────────────────────────
function VersionPanel({ path, onRestore }: { path: string; onRestore: () => void }) {
  const [versions, setVersions] = useState<Version[]>([]);
  const { toast } = useToast();

  const reload = () => listVersions(path).then(setVersions).catch(() => {});
  useEffect(() => { reload(); }, [path]);

  const doRestore = async (id: string) => {
    try {
      await restoreVersion(id);
      toast("success", "Version restored");
      onRestore();
      reload(); // refresh the list to show current state
    } catch (err: any) { toast("danger", String(err)); }
  };

  return (
    <div style={{ width: 220, borderLeft: "1px solid var(--color-border)", padding: 14, overflowY: "auto", background: "var(--color-surface)", display: "flex", flexDirection: "column", gap: 0 }}>
      <p style={{ fontWeight: 600, fontSize: 13, marginBottom: 12 }}>Version history</p>
      {versions.length === 0 && (
        <p style={{ fontSize: 11, color: "var(--color-text-muted)" }}>No snapshots yet — save the file to create one.</p>
      )}
      {versions.map(v => (
        <div key={v.id} style={{ marginBottom: 8, padding: "8px 10px", borderRadius: 6, background: "var(--color-bg)", fontSize: 11, border: "1px solid var(--color-border)" }}>
          <p style={{ fontWeight: 500, marginBottom: 2 }}>{new Date(v.created_at).toLocaleString()}</p>
          <p style={{ color: "var(--color-text-muted)", marginBottom: 6 }}>{(v.size_bytes / 1024).toFixed(1)} KB · {v.actor}</p>
          <button onClick={() => doRestore(v.id)}
            style={{ fontSize: 11, color: "var(--color-primary)", background: "none", border: "none", cursor: "pointer", padding: 0, fontWeight: 600 }}>
            Restore this version
          </button>
        </div>
      ))}
    </div>
  );
}

// ── Main Excel page ───────────────────────────────────────────────────────────
export function ExcelPage() {
  const [mode, setMode] = useState<Mode>("grid");
  const [openPath, setOpenPath] = useState<string | null>(null);
  const [sheets, setSheets] = useState<SheetInfo[]>([]);
  const [fortuneSheets, setFortuneSheets] = useState<any[] | null>(null);
  const [workbookKey, setWorkbookKey] = useState(0);
  const [loading, setLoading] = useState(false);
  const [sheetData, setSheetData] = useState<SheetData | null>(null);
  const [activeSheet, setActiveSheet] = useState(0);
  const [formLayout, setFormLayout] = useState<FormLayout | null>(null);
  const [lockInfo, setLockInfo] = useState<{ held_by_name: string } | null>(null);
  const [showHistory, setShowHistory] = useState(false);
  const [showChat, setShowChat] = useState(false);
  const [saving, setSaving] = useState(false);
  const { user } = useAuthStore();
  const { pendingFile, setPendingFile } = useFileRouter();
  const { toast } = useToast();

  const activeSheetRef = useRef(0);
  const workbookRef = useRef<any>(null);

    // Auto-open from Files page
  useEffect(() => {
    if (pendingFile?.kind === "excel" && pendingFile.path) {
      openFile(pendingFile.path);
      setPendingFile(null);
    }
  }, [pendingFile]);

  const pickFile = async () => {
    try {
      const selected = await openDialog({
        multiple: false,
        filters: [{ name: "Excel files", extensions: ["xlsx", "xls"] }],
        title: "Open Excel file",
      });
      if (typeof selected === "string" && selected) openFile(selected);
    } catch { /* cancelled */ }
  };

  const openFile = async (path: string) => {
    let loadOk = false;
    try {
      setLoading(true);
      setSheetData(null);
      setFortuneSheets(null);
      setSheets([]);
      setOpenPath(path);
      setActiveSheet(0);
      activeSheetRef.current = 0;

      // Get sheet names/metadata from the native backend
      const sheetInfos = await excelOpen(path);
      setSheets(sheetInfos.length > 0 ? sheetInfos : []);

      // Load file bytes via the native reader (works in Tauri; web fallback would differ)
      const base64 = await readFileBase64(path);
      const bytes = Uint8Array.from(atob(base64), char => char.charCodeAt(0));
      const file = new File([bytes], path.split("/").pop() ?? "workbook.xlsx", { type: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" });

      // Convert to FortuneSheet-compatible data
      await transformExcelToFortune(file, (convertedSheets: any[]) => {
        setFortuneSheets(convertedSheets);
        setWorkbookKey(k => k + 1);
        if (convertedSheets.length > 0 && sheetInfos.length === 0) {
          setSheets(convertedSheets.map((sheet, index) => ({
            index,
            name: sheet.name ?? `Sheet ${index + 1}`,
          })));
        }
      }, setWorkbookKey, workbookRef);

      // Load data for the first sheet into the grid view
      if (sheetInfos.length > 0) {
        try {
          const first = await excelGetSheet(path, 0);
          setSheetData(first);
          // If the Fortune conversion produced an empty grid, feed real cells
          // into the Workbook directly from the native engine so the grid is
          // never blank.
          const fromNative = sheetDataToFortune(first);
          const hasNativeData = first.rows.some(r => r.some(c => c !== null && c !== undefined));
          if (hasNativeData && fromNative.length === 0) {
            console.warn("[ExcelPage] sheetData is non-empty but celldata resolved to empty array");
          } else if (fromNative.length > 0) {
            setFortuneSheets(prev => {
              if (!prev || prev.length === 0) return prev;
              return prev.map((s, i) => {
                if (i !== 0) return s;
                const existing = Array.isArray(s?.celldata) ? s.celldata.length : 0;
                return existing === 0 ? { ...s, celldata: fromNative } : s;
              });
            });
            setWorkbookKey(k => k + 1);
          }
        } catch { /* form mode still available */ }
      }

      const lock = await getLock(path);
      setLockInfo(lock ? { held_by_name: lock.held_by_name } : null);
      if (!lock) acquireLock(path, user?.id ?? "local", user?.username ?? "local").catch(() => {});
      recordFileOpen(path, "excel").catch(() => {});
      // The native reader is optional: a workbook can still open and retain
      // styles even if it is not compatible with form mode.
      loadFormLayout(path, 0);
      loadOk = true;
    } catch (err: any) {
      if (!loadOk) {
        setOpenPath(null);
        setFortuneSheets(null);
        setSheets([]);
        toast("danger", `Failed to open Excel file: ${err?.message ?? err}`);
      }
    } finally {
      setLoading(false);
    }
  };

  const loadFormLayout = async (path: string, idx: number) => {
    try { const l = await excelDetectForm(path, idx); setFormLayout(l); }
    catch { /* form detect optional */ }
  };

  const handleSave = async () => {
    if (!openPath) return;
    setSaving(true);
    try {
      const blob: Blob = await transformFortuneToExcel(workbookRef, "xlsx" as any, false);
      const bytes = new Uint8Array(await blob.arrayBuffer());
      await writeFileBytes(openPath, Array.from(bytes));
      toast("success", "Saved with workbook formatting preserved.");
    } catch (err: any) {
      toast("danger", `Save failed: ${err}`);
    } finally {
      setSaving(false);
    }
  };

  const handleClose = async () => {
    if (openPath) releaseLock(openPath, user?.id ?? "local").catch(() => {});
    setOpenPath(null); setSheets([]); setSheetData(null); setFortuneSheets(null); setFormLayout(null); setLockInfo(null); setShowChat(false);
  };

  const handleRestore = async () => {
    if (!openPath) return;
    try {
      setSheetData(await excelGetSheet(openPath, activeSheetRef.current));
    } catch (err: any) {
      toast("danger", `Restore failed: ${err}`);
    } finally {}
  };

  const sheetNames = sheets.map(sheet => sheet.name);
  const chatContext = useMemo(() => workbookText(fortuneSheets), [fortuneSheets]);

  if (!openPath) {
    return (
      <div style={{ display: "flex", alignItems: "center", justifyContent: "center", height: "60vh" }}>
        <div style={{ textAlign: "center" }}>
          <FileSpreadsheet size={52} style={{ margin: "0 auto 16px", color: "var(--color-success)" }} />
          <p style={{ fontWeight: 600, fontSize: 16, marginBottom: 8 }}>Open an Excel file</p>
          <p style={{ fontSize: 13, color: "var(--color-text-secondary)", marginBottom: 20, maxWidth: 340 }}>
            Full fidelity rendering — styles, fonts, colors, formulas, merged cells, multiple sheets.
          </p>
          <Button variant="primary" onClick={pickFile}>Browse file…</Button>
        </div>
      </div>
    );
  }

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "calc(100vh - 150px)", minHeight: 520, gap: 0 }}>
      <FortuneExcelHelper setKey={setWorkbookKey} setSheets={setFortuneSheets} sheetRef={workbookRef} config={{ import: { xlsx: true, csv: true }, export: { xlsx: true, csv: true } }} />
      {/* Toolbar */}
      <div style={{ background: "var(--color-surface)", borderRadius: "var(--radius-card)", border: "1px solid var(--color-border)", padding: "10px 16px", display: "flex", alignItems: "center", gap: 10, marginBottom: 12 }}>
        <FileSpreadsheet size={16} color="var(--color-success)" />
        <span style={{ fontWeight: 600, fontSize: 13, flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
          {openPath.split("/").pop()}
        </span>
        {lockInfo && (
          <span style={{ fontSize: 12, color: "var(--color-warning)", display: "flex", alignItems: "center", gap: 4 }}>
            <Lock size={12} /> Locked by {lockInfo.held_by_name}
          </span>
        )}
        {/* Mode toggle */}
        <div style={{ display: "flex", background: "var(--color-bg)", borderRadius: 8, padding: 3 }}>
          {(["grid", "form"] as Mode[]).map(m => (
            <button key={m} onClick={() => setMode(m)}
              style={{ padding: "5px 14px", borderRadius: 6, border: "none", fontSize: 12, fontWeight: 500, cursor: "pointer",
                background: mode === m ? "var(--color-primary)" : "transparent",
                color: mode === m ? "white" : "var(--color-text-secondary)" }}>
              {m === "grid" ? <><Grid3x3 size={12} style={{ display: "inline", marginRight: 4 }} />Grid</>
                           : <><FormInput size={12} style={{ display: "inline", marginRight: 4 }} />Form</>}
            </button>
          ))}
        </div>
        <Button size="sm" variant="primary" onClick={handleSave} disabled={saving}>
          <Save size={12} /> {saving ? "Saving…" : "Save"}
        </Button>
        <Button size="sm" variant="secondary" onClick={() => setShowChat(value => !value)}><MessageCircle size={13} /> Chat</Button>
        <Button size="sm" variant="ghost" onClick={() => setShowHistory(h => !h)}><History size={14} /></Button>
        <Button size="sm" variant="secondary" onClick={handleClose}>Close</Button>
      </div>

      {/* Content */}
      <div style={{ flex: 1, display: "flex", background: "var(--color-surface)", borderRadius: "var(--radius-card)", border: "1px solid var(--color-border)", overflow: "hidden" }}>
        <div style={{ flex: 1, overflow: "hidden", display: "flex", flexDirection: "column" }}>
          {mode === "grid" ? (
            <div style={{ flex: 1, minHeight: 0, position: "relative" }}>
              {fortuneSheets && <Workbook key={workbookKey} ref={workbookRef} data={fortuneSheets} showToolbar showFormulaBar showSheetTabs />}
              {loading && <div style={{ position: "absolute", inset: 0, zIndex: 10, display: "flex", flexDirection: "column", alignItems: "center", justifyContent: "center", gap: 12, background: "var(--color-surface)" }}><span style={{ width: 28, height: 28, border: "3px solid var(--color-border)", borderTopColor: "var(--color-primary)", borderRadius: "50%", animation: "soffit-spin 0.8s linear infinite" }} /><span style={{ color: "var(--color-text-secondary)", fontSize: 13 }}>Loading workbook…</span></div>}
              {!loading && !fortuneSheets && <div style={{ display: "grid", placeItems: "center", height: "100%", color: "var(--color-text-muted)" }}>Unable to render this workbook.</div>}
            </div>
          ) : (
            <div style={{ overflowY: "auto", flex: 1 }}>
              {formLayout
                ? <FormView path={openPath} layout={formLayout} />
                : <div style={{ padding: 40, textAlign: "center", color: "var(--color-text-muted)", fontSize: 13 }}>No form layout detected for this sheet.</div>
              }
            </div>
          )}

          {/* Sheet tabs */}
          {sheetNames.length > 1 && (
            <div style={{ display: "flex", gap: 2, padding: "8px 12px", borderTop: "1px solid var(--color-border)", background: "var(--color-bg)", overflowX: "auto" }}>
              {sheetNames.map((name, idx) => (
                <button key={idx} onClick={async () => { setActiveSheet(idx); activeSheetRef.current = idx; if (openPath && mode === "form") { try { setSheetData(await excelGetSheet(openPath, idx)); } catch { /* styled grid stays available */ } loadFormLayout(openPath, idx); } }}
                  style={{ padding: "4px 12px", borderRadius: "6px 6px 0 0", border: "1px solid var(--color-border)", fontSize: 12, cursor: "pointer", whiteSpace: "nowrap",
                    fontWeight: activeSheet === idx ? 600 : 400,
                    background: activeSheet === idx ? "var(--color-surface)" : "var(--color-bg)",
                    color: activeSheet === idx ? "var(--color-primary)" : "var(--color-ink)",
                    borderBottom: activeSheet === idx ? "2px solid var(--color-primary)" : "none" }}>
                  {name}
                </button>
              ))}
            </div>
          )}
        </div>

        {showHistory && openPath && (
          <VersionPanel path={openPath} onRestore={handleRestore} />
        )}
        {showChat && openPath && <DocumentChat filePath={openPath} fileKind="excel" fileName={openPath.split("/").pop() ?? "Workbook"} documentText={chatContext} />}
      </div>
    </div>
  );
}
