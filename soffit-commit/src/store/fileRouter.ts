import { create } from "zustand";

type FileKind = "excel" | "sql" | "csv" | "sqlite" | "parquet" | "db" | "generic";

const EXT_TO_KIND: Record<string, FileKind> = {
  xlsx: "excel", xls: "excel",
  sql:  "sql",
  csv:  "csv",
  sqlite: "sqlite", db: "sqlite",
  parquet: "parquet",
};

export function extToKind(path: string): FileKind {
  const ext = path.split(".").pop()?.toLowerCase() ?? "";
  return EXT_TO_KIND[ext] ?? "generic";
}

interface FileRouterState {
  pendingFile: { path: string; kind: FileKind } | null;
  setPendingFile: (file: { path: string; kind: FileKind } | null) => void;
  activateFile: (path: string) => { path: string; kind: FileKind };
}

export const useFileRouter = create<FileRouterState>((set) => ({
  pendingFile: null,
  setPendingFile: (file) => set({ pendingFile: file }),
  activateFile: (path: string) => {
    const kind = extToKind(path);
    const file = { path, kind };
    set({ pendingFile: file });
    return file;
  },
}));
