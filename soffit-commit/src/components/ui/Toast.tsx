import React, { createContext, useContext, useState, useCallback } from "react";
import { CheckCircle, AlertTriangle, XCircle, X } from "lucide-react";

type ToastType = "success" | "warning" | "danger" | "info";

interface Toast {
  id: string;
  type: ToastType;
  message: string;
}

interface ToastContextValue {
  toast: (type: ToastType, message: string) => void;
}

const ToastCtx = createContext<ToastContextValue>({ toast: () => {} });

export function useToast() { return useContext(ToastCtx); }

const icons: Record<ToastType, React.ReactNode> = {
  success: <CheckCircle size={16} />,
  warning: <AlertTriangle size={16} />,
  danger:  <XCircle size={16} />,
  info:    <CheckCircle size={16} />,
};
const accents: Record<ToastType, string> = {
  success: "var(--color-success)",
  warning: "var(--color-warning)",
  danger:  "var(--color-danger)",
  info:    "var(--color-info)",
};

export function ToastProvider({ children }: { children: React.ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);

  const toast = useCallback((type: ToastType, message: string) => {
    const id = Math.random().toString(36).slice(2);
    setToasts((prev) => [...prev, { id, type, message }]);
    setTimeout(() => setToasts((prev) => prev.filter((t) => t.id !== id)), 4000);
  }, []);

  const remove = (id: string) => setToasts((prev) => prev.filter((t) => t.id !== id));

  return (
    <ToastCtx.Provider value={{ toast }}>
      {children}
      <div style={{ position: "fixed", bottom: 24, right: 24, display: "flex", flexDirection: "column", gap: 8, zIndex: 2000 }}>
        {toasts.map((t) => (
          <div key={t.id} style={{
            background: "var(--color-surface)", borderRadius: 10, border: "1px solid var(--color-border)",
            borderLeft: `4px solid ${accents[t.type]}`, padding: "12px 16px",
            display: "flex", alignItems: "center", gap: 10, minWidth: 280, color: accents[t.type]
          }}>
            {icons[t.type]}
            <span style={{ flex: 1, color: "var(--color-ink)", fontSize: 13 }}>{t.message}</span>
            <button onClick={() => remove(t.id)} style={{ background: "none", border: "none", cursor: "pointer", color: "var(--color-text-muted)" }}>
              <X size={14} />
            </button>
          </div>
        ))}
      </div>
    </ToastCtx.Provider>
  );
}
