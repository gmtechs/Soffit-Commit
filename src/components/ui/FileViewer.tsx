import React, { useEffect, useState } from "react";
import Editor from "@monaco-editor/react";
import { FileText, Image, FileCode, File, ExternalLink } from "lucide-react";
import { readFileBase64, readFileText, recordFileOpen } from "../../lib/tauri";
import { openPath } from "@tauri-apps/plugin-opener";
import { Button } from "./Button";

interface FileViewerProps {
  filePath: string;
  fileKind: string;
  fileName: string;
}

const TEXT_KINDS = ["sql", "text", "csv"];
const IMAGE_KINDS = ["image"];
const CODE_EXTS = ["rs", "ts", "tsx", "js", "jsx", "py", "json", "toml", "yaml", "yml", "md", "html", "css"];

function getMonacoLang(path: string): string {
  const ext = path.split(".").pop()?.toLowerCase() ?? "";
  const map: Record<string, string> = {
    ts: "typescript", tsx: "typescript", js: "javascript", jsx: "javascript",
    rs: "rust", py: "python", json: "json", toml: "toml",
    yaml: "yaml", yml: "yaml", md: "markdown", html: "html",
    css: "css", sql: "sql", sh: "shell",
  };
  return map[ext] ?? "plaintext";
}

function getMimeType(path: string): string {
  const ext = path.split(".").pop()?.toLowerCase() ?? "";
  const map: Record<string, string> = {
    png: "image/png", jpg: "image/jpeg", jpeg: "image/jpeg",
    gif: "image/gif", webp: "image/webp", svg: "image/svg+xml",
    pdf: "application/pdf",
  };
  return map[ext] ?? "application/octet-stream";
}

export function FileViewer({ filePath, fileKind, fileName }: FileViewerProps) {
  const [content, setContent] = useState<string | null>(null);
  const [b64, setB64] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const ext = filePath.split(".").pop()?.toLowerCase() ?? "";
  const isPDF = ext === "pdf";
  const isImage = IMAGE_KINDS.includes(fileKind) || ["png","jpg","jpeg","gif","webp","svg"].includes(ext);
  const isText = TEXT_KINDS.includes(fileKind) || CODE_EXTS.includes(ext);
  const isMarkdown = ext === "md";

  useEffect(() => {
    setLoading(true); setError(null); setContent(null); setB64(null);
    recordFileOpen(filePath, fileKind).catch(() => {});

    if (isImage || isPDF) {
      readFileBase64(filePath)
        .then(setB64)
        .catch(err => setError(String(err)))
        .finally(() => setLoading(false));
    } else if (isText) {
      readFileText(filePath)
        .then(setContent)
        .catch(err => setError(String(err)))
        .finally(() => setLoading(false));
    } else {
      setLoading(false);
    }
  }, [filePath]);

  if (loading) {
    return (
      <div style={{ flex: 1, minWidth: 0, padding: 40, display: "grid", placeItems: "center", color: "var(--color-text-muted)" }}>
        <div style={{ fontSize: 13 }}>Loading {fileName}…</div>
      </div>
    );
  }

  if (error) {
    return (
      <div style={{ flex: 1, minWidth: 0, padding: 40, textAlign: "center", color: "var(--color-danger)" }}>
        <p style={{ fontWeight: 600, marginBottom: 8 }}>Could not read file</p>
        <p style={{ fontSize: 12 }}>{error}</p>
      </div>
    );
  }

  // Image
  if (isImage && b64) {
    const mime = getMimeType(filePath);
    return (
      <div style={{ flex: 1, minWidth: 0, padding: 28, textAlign: "center", overflow: "auto", background: "var(--color-bg)", height: "100%" }}>
        <img
          src={`data:${mime};base64,${b64}`}
          alt={fileName}
          style={{ maxWidth: "100%", maxHeight: "70vh", borderRadius: 8, border: "1px solid var(--color-border)" }}
        />
      </div>
    );
  }

  // PDF
  if (isPDF && b64) {
    return (
      <div style={{ flex: 1, minWidth: 0, padding: 20, background: "var(--color-bg)", overflow: "auto" }}>
        <div style={{ height: "100%", minHeight: 620, maxWidth: 1040, margin: "0 auto", background: "var(--color-surface)", border: "1px solid var(--color-border)", boxShadow: "0 12px 32px rgba(45,44,42,.08)" }}>
          <embed src={`data:application/pdf;base64,${b64}`} type="application/pdf" style={{ height: "100%", width: "100%", border: "none" }} />
        </div>
      </div>
    );
  }

  // Text / code / markdown
  if (isText && content !== null) {
    return (
      <div style={{ flex: 1, minWidth: 0, overflow: "hidden" }}><Editor height="100%" language={getMonacoLang(filePath)} value={content} theme="vs" options={{ readOnly: false, minimap: { enabled: false }, fontSize: 13, wordWrap: "on", scrollBeyondLastLine: false, padding: { top: 18, bottom: 18 }, fontFamily: "'JetBrains Mono', 'Cascadia Code', monospace" }} /></div>
    );
  }

  // Fallback — unknown type
  return (
    <div style={{ flex: 1, minWidth: 0, padding: 48, textAlign: "center", color: "var(--color-text-muted)" }}>
      <File size={44} style={{ margin: "0 auto 16px" }} />
      <p style={{ fontWeight: 600, marginBottom: 6, color: "var(--color-ink)" }}>{fileName}</p>
      <p style={{ fontSize: 13, marginBottom: 20 }}>
        No built-in preview for .{ext} files.
      </p>
      <Button variant="primary" onClick={() => openPath(filePath).catch(() => {})}>
        <ExternalLink size={14} /> Open with system app
      </Button>
    </div>
  );
}
