import React, { useState, useEffect, useMemo } from "react";
import Editor from "@monaco-editor/react";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";
import { Database, Play, AlignLeft, Upload, Download, BarChart2, Save, X, Check, ChevronUp, ChevronDown } from "lucide-react";
import {
  sqlExecute, sqlExecuteScript, sqlQueryFile,
  sqlGenerateEdit, sqlImportDump, recordFileOpen,
  type QueryResult, type ScriptStatementResult, type ImportResult,
} from "../lib/tauri";
import { Button } from "../components/ui/Button";
import { Modal } from "../components/ui/Modal";
import { useToast } from "../components/ui/Toast";
import { useFileRouter } from "../store/fileRouter";
import { BarChart, Bar, XAxis, YAxis, Tooltip, ResponsiveContainer } from "recharts";

const DEFAULT_SQL = "-- Write SQL here. Use __FILE__ as a placeholder for a file path.\nSELECT 1 + 1 AS result;";

// ── Helpers ───────────────────────────────────────────────────────────────────

function generateCsv(result: QueryResult): string {
  const header = result.columns.map(c => `"${c.name.replace(/"/g, '""')}"`).join(",");
  const rows = result.rows.map(row =>
    row.map(cell => cell === null ? "" : `"${String(cell).replace(/"/g, '""')}"`).join(",")
  );
  return [header, ...rows].join("\n");
}

function generateExportSql(result: QueryResult, tableName = "exported_table"): string {
  const cols = result.columns.map(c => `"${c.name}" TEXT`).join(", ");
  const inserts = result.rows.map(row => {
    const vals = row.map(cell => cell === null ? "NULL" : `'${String(cell).replace(/'/g, "''")}'`).join(", ");
    return `INSERT INTO "${tableName}" VALUES (${vals});`;
  }).join("\n");
  return `CREATE TABLE IF NOT EXISTS "${tableName}" (${cols});\n${inserts}`;
}

function isNumericCol(rows: unknown[][], colIdx: number): boolean {
  return rows.every(r => r[colIdx] === null || !isNaN(Number(r[colIdx])));
}

type SortDir = "asc" | "desc";

// ── Results grid ──────────────────────────────────────────────────────────────
function ResultsGrid({ result, onApplyEdit }: { result: QueryResult; onApplyEdit?: () => void }) {
  const [editedCells, setEditedCells] = useState<Record<string, string>>({});
  const [diffSql, setDiffSql] = useState("");
  const [diffOpen, setDiffOpen] = useState(false);
  const [sortCol, setSortCol] = useState<number | null>(null);
  const [sortDir, setSortDir] = useState<SortDir>("asc");
  const [chartCol, setChartCol] = useState<number | null>(null);
  const { toast } = useToast();

  const sortedRows = useMemo(() => {
    if (sortCol === null) return result.rows;
    return [...result.rows].sort((a, b) => {
      const av = a[sortCol] ?? "";
      const bv = b[sortCol] ?? "";
      const an = Number(av), bn = Number(bv);
      const cmp = !isNaN(an) && !isNaN(bn) ? an - bn : String(av).localeCompare(String(bv));
      return sortDir === "asc" ? cmp : -cmp;
    });
  }, [result.rows, sortCol, sortDir]);

  const numericCols = result.columns.map((_, i) => i).filter(i => isNumericCol(result.rows, i));

  const handleSort = (ci: number) => {
    if (sortCol === ci) setSortDir(d => d === "asc" ? "desc" : "asc");
    else { setSortCol(ci); setSortDir("asc"); }
  };

  const handleApply = async () => {
    const pkCol = result.columns[0]?.name ?? "id";
    const changes: [string, unknown][] = [];
    let pkVal: unknown = null;
    for (const [key, val] of Object.entries(editedCells)) {
      const [ri, ci] = key.split(":").map(Number);
      pkVal = sortedRows[ri]?.[0];
      changes.push([result.columns[ci]?.name ?? String(ci), val]);
    }
    if (!changes.length) return;
    const sql = await sqlGenerateEdit("your_table", pkCol, pkVal, changes);
    setDiffSql(sql); setDiffOpen(true);
  };

  const handleExportCsv = async () => {
    const csv = generateCsv(result);
    const path = await saveDialog({ filters: [{ name: "CSV", extensions: ["csv"] }], defaultPath: "results.csv" }).catch(() => null);
    if (!path) return;
    await writeTextFile(path as string, csv);
    toast("success", "Exported to CSV");
  };

  const handleExportSql = async () => {
    const sql = generateExportSql(result);
    const path = await saveDialog({ filters: [{ name: "SQL", extensions: ["sql"] }], defaultPath: "results.sql" }).catch(() => null);
    if (!path) return;
    await writeTextFile(path as string, sql);
    toast("success", "Exported to SQL");
  };

  if (result.error) return (
    <div style={{ padding: 14, color: "var(--color-danger)", fontSize: 13, fontFamily: "monospace", whiteSpace: "pre-wrap" }}>
      {result.error}
    </div>
  );

  if (result.rows_affected !== null) return (
    <div style={{ padding: 14, color: "var(--color-success)", fontSize: 13 }}>
      ✓ {result.rows_affected} row{result.rows_affected !== 1 ? "s" : ""} affected — {result.execution_ms}ms
    </div>
  );

  if (!result.columns.length) return (
    <div style={{ padding: 14, color: "var(--color-text-muted)", fontSize: 13 }}>No results.</div>
  );

  // Chart view
  if (chartCol !== null) {
    const chartData = sortedRows.slice(0, 100).map((row, i) => ({
      i: i + 1,
      val: Number(row[chartCol] ?? 0),
    }));
    return (
      <div style={{ padding: 14 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 10, marginBottom: 10 }}>
          <select value={chartCol} onChange={e => setChartCol(Number(e.target.value))}
            style={{ padding: "4px 8px", borderRadius: 4, border: "1px solid var(--color-border)", fontSize: 12, background: "var(--color-bg)" }}>
            {numericCols.map(i => <option key={i} value={i}>{result.columns[i].name}</option>)}
          </select>
          <Button size="sm" variant="secondary" onClick={() => setChartCol(null)}>Table view</Button>
        </div>
        <ResponsiveContainer width="100%" height={180}>
          <BarChart data={chartData}>
            <XAxis dataKey="i" tick={{ fontSize: 10 }} />
            <YAxis tick={{ fontSize: 10 }} />
            <Tooltip />
            <Bar dataKey="val" fill="var(--color-info)" />
          </BarChart>
        </ResponsiveContainer>
      </div>
    );
  }

  return (
    <>
      <div style={{ overflowX: "auto" }}>
        <table style={{ borderCollapse: "collapse", fontSize: 12, width: "100%" }}>
          <thead>
            <tr style={{ background: "var(--color-bg)", borderBottom: "1px solid var(--color-border)" }}>
              {result.columns.map((col, ci) => (
                <th key={ci} onClick={() => handleSort(ci)}
                  style={{ padding: "7px 10px", textAlign: "left", fontWeight: 600, color: "var(--color-ink)", cursor: "pointer", userSelect: "none", whiteSpace: "nowrap" }}>
                  <span style={{ display: "flex", alignItems: "center", gap: 4 }}>
                    {col.name}
                    {sortCol === ci ? (sortDir === "asc" ? <ChevronUp size={11} /> : <ChevronDown size={11} />) : <ChevronDown size={11} style={{ opacity: 0.2 }} />}
                  </span>
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {sortedRows.map((row, ri) => (
              <tr key={ri} style={{ borderBottom: "1px solid var(--color-border)" }}
                onMouseEnter={e => (e.currentTarget.style.background = "var(--color-bg)")}
                onMouseLeave={e => (e.currentTarget.style.background = "transparent")}>
                {row.map((cell, ci) => {
                  const key = `${ri}:${ci}`;
                  const isEdited = key in editedCells;
                  return (
                    <td key={ci} contentEditable={ci > 0} suppressContentEditableWarning
                      onBlur={e => {
                        const v = e.currentTarget.textContent ?? "";
                        if (v !== String(cell ?? "")) setEditedCells(p => ({ ...p, [key]: v }));
                      }}
                      style={{ padding: "6px 10px", maxWidth: 200, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", background: isEdited ? "rgba(226,113,0,0.06)" : "transparent", outline: "none" }}>
                      {cell === null ? <span style={{ color: "var(--color-text-muted)" }}>NULL</span> : String(cell)}
                    </td>
                  );
                })}
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <div style={{ padding: "7px 10px", borderTop: "1px solid var(--color-border)", display: "flex", alignItems: "center", gap: 8, fontSize: 11, color: "var(--color-text-secondary)" }}>
        <span>{result.rows.length} rows — {result.execution_ms}ms</span>
        <span style={{ flex: 1 }} />
        {numericCols.length > 0 && (
          <Button size="sm" variant="ghost" onClick={() => setChartCol(numericCols[0])}><BarChart2 size={12} /> Chart</Button>
        )}
        {Object.keys(editedCells).length > 0 && (
          <Button size="sm" variant="primary" onClick={handleApply}><Check size={12} /> Apply changes</Button>
        )}
        <Button size="sm" variant="secondary" onClick={handleExportCsv}><Download size={12} /> CSV</Button>
        <Button size="sm" variant="secondary" onClick={handleExportSql}><Download size={12} /> SQL</Button>
      </div>

      <Modal open={diffOpen} onClose={() => setDiffOpen(false)} title="Confirm SQL change">
        <p style={{ fontSize: 13, color: "var(--color-text-secondary)", marginBottom: 12 }}>Review before writing:</p>
        <pre style={{ background: "var(--color-bg)", borderRadius: 6, padding: 12, fontSize: 12, fontFamily: "monospace", overflowX: "auto", border: "1px solid var(--color-border)", marginBottom: 16 }}>{diffSql}</pre>
        <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
          <Button variant="secondary" onClick={() => setDiffOpen(false)}><X size={13} /> Cancel</Button>
          <Button variant="primary" onClick={async () => {
            await sqlExecute(diffSql);
            setEditedCells({}); setDiffOpen(false); onApplyEdit?.();
          }}><Check size={13} /> Run SQL</Button>
        </div>
      </Modal>
    </>
  );
}

// ── Script execution results display ─────────────────────────────────────────
function ScriptResults({ results }: { results: ScriptStatementResult[] }) {
  const last = results[results.length - 1];
  const hasError = last?.result.error;

  return (
    <div>
      {hasError && (
        <div style={{ padding: "10px 14px", background: "#FEF2F2", border: "1px solid var(--color-danger)", borderRadius: 6, marginBottom: 8, fontSize: 12 }}>
          <p style={{ fontWeight: 700, color: "var(--color-danger)", marginBottom: 4 }}>
            Error at statement {last.statement_index} (line {last.line_number})
          </p>
          <pre style={{ fontFamily: "monospace", color: "var(--color-danger)", marginBottom: 4, whiteSpace: "pre-wrap" }}>{last.sql_snippet.slice(0, 120)}</pre>
          <p style={{ color: "var(--color-text-secondary)" }}>{last.result.error}</p>
        </div>
      )}
      {results.filter(r => !r.result.error).map(r => (
        <div key={r.statement_index} style={{ padding: "6px 10px", fontSize: 11, color: "var(--color-text-secondary)", borderBottom: "1px solid var(--color-border)" }}>
          Stmt {r.statement_index}: {r.result.rows_affected !== null
            ? `${r.result.rows_affected} rows affected`
            : `${r.result.rows.length} rows returned`} — {r.result.execution_ms}ms
        </div>
      ))}
    </div>
  );
}

// ── Main SQL page ─────────────────────────────────────────────────────────────
export function SQLPage() {
  const [sql, setSql] = useState(DEFAULT_SQL);
  const [filePath, setFilePath] = useState("");
  const [result, setResult] = useState<QueryResult | null>(null);
  const [scriptResults, setScriptResults] = useState<ScriptStatementResult[] | null>(null);
  const [importResult, setImportResult] = useState<ImportResult | null>(null);
  const [running, setRunning] = useState(false);
  const [saveOpen, setSaveOpen] = useState(false);
  const [saveName, setSaveName] = useState("");
  const { toast } = useToast();
  const { pendingFile, setPendingFile } = useFileRouter();

  // Auto-load file from Files page
  useEffect(() => {
    if (pendingFile && ["sql","csv","parquet","sqlite","db"].includes(pendingFile.kind)) {
      setFilePath(pendingFile.path);
      recordFileOpen(pendingFile.path, pendingFile.kind).catch(() => {});
      setPendingFile(null);
    }
  }, [pendingFile]);

  const pickFile = async () => {
    const sel = await openDialog({
      multiple: false,
      filters: [{ name: "SQL & data files", extensions: ["sql","csv","parquet","sqlite","db","json"] }],
    }).catch(() => null);
    if (typeof sel === "string") setFilePath(sel);
  };

  const runQuery = async () => {
    setRunning(true); setResult(null); setScriptResults(null); setImportResult(null);
    try {
      const stmts = await sqlExecuteScript(sql);
      if (stmts.length === 1 && !stmts[0].result.error) {
        setResult(stmts[0].result);
      } else if (stmts.length > 0) {
        setScriptResults(stmts);
        // If last statement succeeded and returned rows, also set as main result
        const last = stmts[stmts.length - 1];
        if (!last.result.error && last.result.columns.length > 0) setResult(last.result);
      }
    } catch (err: any) {
      setResult({ columns: [], rows: [], rows_affected: null, error: String(err), execution_ms: 0 });
    } finally { setRunning(false); }
  };

  const runWithFile = async () => {
    if (!filePath.trim()) { runQuery(); return; }
    setRunning(true); setResult(null); setScriptResults(null); setImportResult(null);
    try {
      const res = await sqlQueryFile(filePath.trim(), sql);
      setResult(res);
    } catch (err: any) {
      setResult({ columns: [], rows: [], rows_affected: null, error: String(err), execution_ms: 0 });
    } finally { setRunning(false); }
  };

  const importDump = async () => {
    const sel = await openDialog({
      multiple: false,
      filters: [{ name: "SQL dump", extensions: ["sql"] }],
      title: "Import SQL dump (MySQL/phpMyAdmin)",
    }).catch(() => null);
    if (!sel) return;
    setRunning(true);
    try {
      const res = await sqlImportDump(sel as string);
      setImportResult(res);
      if (!res.error) toast("success", `Imported ${res.statements_executed} statements`);
      else toast("danger", `Failed at line ${res.error.original_line_number}: ${res.error.message}`);
    } catch (err: any) { toast("danger", String(err)); }
    finally { setRunning(false); }
  };

  const saveQuery = async () => {
    if (!saveName.trim()) return;
    const path = await saveDialog({
      defaultPath: `${saveName.trim()}.sql`,
      filters: [{ name: "SQL", extensions: ["sql"] }],
    }).catch(() => null);
    if (!path) { setSaveOpen(false); return; }
    await writeTextFile(path as string, sql);
    await recordFileOpen(path as string, "sql").catch(() => {});
    toast("success", "Query saved");
    setSaveOpen(false); setSaveName("");
  };

  const formatSql = () => {
    const kws = ["SELECT","FROM","WHERE","JOIN","LEFT","RIGHT","INNER","ON","GROUP BY","ORDER BY","HAVING","LIMIT","OFFSET","INSERT","INTO","VALUES","UPDATE","SET","DELETE","CREATE","TABLE","DROP","WITH","AND","OR","AS","DISTINCT"];
    let f = sql;
    kws.forEach(k => { f = f.replace(new RegExp(`\\b${k}\\b`, "gi"), k); });
    setSql(f);
  };

  const inputStyle: React.CSSProperties = { width: "100%", padding: "7px 10px", borderRadius: 6, border: "1px solid var(--color-border)", fontSize: 12, background: "var(--color-bg)", outline: "none", color: "var(--color-ink)" };

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100%", gap: 10 }}>
      {/* Toolbar */}
      <div style={{ background: "var(--color-surface)", borderRadius: "var(--radius-card)", border: "1px solid var(--color-border)", padding: "9px 14px", display: "flex", alignItems: "center", gap: 8 }}>
        <Database size={15} color="var(--color-info)" />
        <span style={{ fontWeight: 600, fontSize: 13, color: "var(--color-ink)" }}>SQL Console</span>
        <span style={{ flex: 1 }} />

        {/* File picker */}
        <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
          <div style={{ fontSize: 11, color: filePath ? "var(--color-ink)" : "var(--color-text-muted)", maxWidth: 200, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
            {filePath ? filePath.split("/").pop() : "No file"}
          </div>
          <Button size="sm" variant="secondary" onClick={pickFile}>Open file</Button>
          {filePath && <button onClick={() => setFilePath("")} style={{ background: "none", border: "none", cursor: "pointer", color: "var(--color-text-muted)" }}><X size={12} /></button>}
        </div>

        <Button size="sm" variant="secondary" onClick={formatSql}><AlignLeft size={12} /> Format</Button>
        <Button size="sm" variant="secondary" onClick={() => setSaveOpen(true)}><Save size={12} /> Save</Button>
        <Button size="sm" variant="secondary" onClick={importDump} disabled={running}><Upload size={12} /> Import dump</Button>
        <Button size="sm" variant="primary" onClick={filePath ? runWithFile : runQuery} disabled={running}>
          <Play size={12} /> {running ? "Running…" : "Run"}
        </Button>
      </div>

      {/* Monaco editor */}
      <div style={{ flex: 1, background: "var(--color-surface)", borderRadius: "var(--radius-card)", border: "1px solid var(--color-border)", overflow: "hidden", minHeight: 180 }}>
        <Editor
          height="100%"
          defaultLanguage="sql"
          value={sql}
          onChange={v => setSql(v ?? "")}
          theme="vs"
          options={{
            minimap: { enabled: false }, fontSize: 13, lineNumbers: "on",
            scrollBeyondLastLine: false, wordWrap: "on", padding: { top: 10, bottom: 10 },
            fontFamily: "'JetBrains Mono','Cascadia Code','Fira Code',monospace",
          }}
          onMount={editor => {
            // Ctrl/Cmd+Enter to run
            editor.addCommand(2051, () => filePath ? runWithFile() : runQuery());
          }}
        />
      </div>

      {/* Results */}
      <div style={{ background: "var(--color-surface)", borderRadius: "var(--radius-card)", border: "1px solid var(--color-border)", minHeight: 100, maxHeight: 340, overflow: "auto" }}>
        {!result && !scriptResults && !importResult && (
          <div style={{ padding: 16, color: "var(--color-text-muted)", fontSize: 12 }}>
            Run a query (Ctrl+Enter) · Import a MySQL dump · Open a .csv or .sqlite file
          </div>
        )}
        {result && <ResultsGrid result={result} onApplyEdit={() => filePath ? runWithFile() : runQuery()} />}
        {scriptResults && !result && <ScriptResults results={scriptResults} />}
        {importResult && (
          <div style={{ padding: "10px 14px", fontSize: 12 }}>
            {importResult.error ? (
              <p style={{ color: "var(--color-danger)" }}>
                ✕ Failed at line {importResult.error.original_line_number} (stmt {importResult.error.statement_index}): {importResult.error.message}<br />
                <code style={{ fontFamily: "monospace" }}>{importResult.error.snippet}</code>
              </p>
            ) : (
              <p style={{ color: "var(--color-success)" }}>
                ✓ Imported {importResult.statements_executed} statements successfully
              </p>
            )}
          </div>
        )}
      </div>

      {/* Save query modal */}
      <Modal open={saveOpen} onClose={() => setSaveOpen(false)} title="Save query">
        <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
          <div>
            <label style={{ fontSize: 12, fontWeight: 500, display: "block", marginBottom: 5 }}>Script name</label>
            <input style={inputStyle} value={saveName} onChange={e => setSaveName(e.target.value)}
              placeholder="my-query" autoFocus onKeyDown={e => e.key === "Enter" && saveQuery()} />
          </div>
          <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
            <Button variant="secondary" onClick={() => setSaveOpen(false)}>Cancel</Button>
            <Button variant="primary" onClick={saveQuery}>Save</Button>
          </div>
        </div>
      </Modal>
    </div>
  );
}
