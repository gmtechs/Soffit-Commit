import React, { useEffect, useRef, useState } from "react";

/* ── Premium shared primitives (§7 motion + global treatments) ── */

/** Animated number that eases toward `value` whenever it changes. */
export function useCountUp(value: number, durationMs = 700): number {
  const [display, setDisplay] = useState(value);
  const fromRef = useRef(value);
  const rafRef = useRef(0);
  useEffect(() => {
    const from = fromRef.current;
    if (from === value) { setDisplay(value); return; }
    const start = performance.now();
    const tick = (now: number) => {
      const t = Math.min(1, (now - start) / durationMs);
      const eased = 1 - Math.pow(1 - t, 3);
      setDisplay(from + (value - from) * eased);
      if (t < 1) rafRef.current = requestAnimationFrame(tick);
      else fromRef.current = value;
    };
    rafRef.current = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(rafRef.current);
  }, [value, durationMs]);
  useEffect(() => { fromRef.current = value; }, []);
  return display;
}

export function CountUp({ value, format }: { value: number; format: (n: number) => string }) {
  const display = useCountUp(value);
  return <>{format(display)}</>;
}

/** Sweep-animated SVG arc ring (0–100). */
export function AnimatedRing({ pct, size = 120, stroke = 10, track }: {
  pct: number; size?: number; stroke?: number; track: string;
}) {
  const r = (size - stroke) / 2;
  const c = 2 * Math.PI * r;
  const target = c * (1 - Math.max(0, Math.min(100, pct)) / 100);
  const gid = React.useId();
  return (
    <svg
      width={size} height={size / 2 + stroke}
      viewBox={`0 0 ${size} ${size / 2 + stroke}`}
      style={{ overflow: "visible", ["--ring-c" as any]: c, ["--ring-target" as any]: target }}
    >
      <defs>
        <linearGradient id={gid} x1="0" y1="0" x2="1" y2="0">
          <stop offset="0%" stopColor="#3B6BFF" />
          <stop offset="100%" stopColor="#34D399" />
        </linearGradient>
      </defs>
      <path d={`M ${stroke / 2} ${size / 2} A ${r} ${r} 0 0 1 ${size - stroke / 2} ${size / 2}`}
        fill="none" stroke={track} strokeWidth={stroke} strokeLinecap="round" />
      <path d={`M ${stroke / 2} ${size / 2} A ${r} ${r} 0 0 1 ${size - stroke / 2} ${size / 2}`}
        fill="none" stroke={`url(#${gid})`}
        strokeWidth={stroke} strokeLinecap="round"
        strokeDasharray={c} strokeDashoffset={target}
        style={{ animation: "soffit-ring-sweep 1.1s cubic-bezier(0.22, 1, 0.36, 1)" }} />
    </svg>
  );
}
/** Pill-shaped status chip with tinted background (§1.6). */
export function StatusChip({ tone, children, pulse }: {
  tone: "green" | "blue" | "red" | "amber" | "gray";
  children: React.ReactNode; pulse?: boolean;
}) {
  const tones: Record<string, { bg: string; fg: string; dot: string }> = {
    green: { bg: "rgba(52, 211, 153, 0.13)", fg: "var(--color-success)", dot: "var(--color-success)" },
    blue:  { bg: "rgba(59, 107, 255, 0.13)",  fg: "var(--color-primary)", dot: "var(--color-primary)" },
    red:   { bg: "rgba(255, 92, 92, 0.13)",   fg: "var(--color-danger)",  dot: "var(--color-danger)" },
    amber: { bg: "rgba(250, 204, 21, 0.14)",  fg: "var(--color-warning)", dot: "var(--color-warning)" },
    gray:  { bg: "var(--color-surface-raised)", fg: "var(--color-text-secondary)", dot: "var(--color-text-muted)" },
  };
  const t = tones[tone];
  return (
    <span style={{
      display: "inline-flex", alignItems: "center", gap: 6,
      padding: "3px 10px", borderRadius: 999, background: t.bg,
      fontSize: 12, fontWeight: 600, color: t.fg, whiteSpace: "nowrap",
      transition: "background 0.25s, color 0.25s",
    }}>
      <span style={{
        width: 6, height: 6, borderRadius: "50%", background: t.dot, flexShrink: 0,
        animation: pulse ? "soffit-pulse 1.4s ease-in-out infinite" : undefined,
      }} />
      {children}
    </span>
  );
}

/** 32–36px rounded-square icon container in tinted accent (§1.4). */
export function IconTile({ children, tone = "blue", size = 34 }: {
  children: React.ReactNode; tone?: "blue" | "green" | "violet" | "amber" | "red" | "gray"; size?: number;
}) {
  const tones: Record<string, { bg: string; fg: string }> = {
    blue:   { bg: "rgba(59, 107, 255, 0.13)",  fg: "var(--color-primary)" },
    green:  { bg: "rgba(52, 211, 153, 0.13)",  fg: "var(--color-success)" },
    violet: { bg: "rgba(139, 92, 246, 0.15)",  fg: "#A78BFA" },
    amber:  { bg: "rgba(250, 204, 21, 0.15)",  fg: "var(--color-warning)" },
    red:    { bg: "rgba(255, 92, 92, 0.13)",   fg: "var(--color-danger)" },
    gray:   { bg: "var(--color-surface-raised)", fg: "var(--color-text-secondary)" },
  };
  const t = tones[tone];
  return (
    <span style={{
      width: size, height: size, minWidth: size, borderRadius: size * 0.26,
      background: t.bg, color: t.fg,
      display: "inline-flex", alignItems: "center", justifyContent: "center", flexShrink: 0,
      transition: "background 0.2s, color 0.2s",
    }}>
      {children}
    </span>
  );
}

/** Device avatar with gradient presence ring when online (§4). */
export function PeerAvatar({ name, online, size = 34 }: { name: string; online: boolean; size?: number }) {
  const initials = name.slice(0, 2).toUpperCase() || "??";
  const inner = size - 4;
  return (
    <span title={name} style={{
      width: size, height: size, borderRadius: "50%", flexShrink: 0,
      background: online ? "conic-gradient(from 40deg, #3B6BFF, #34D399, #3B6BFF)" : "var(--color-border)",
      display: "inline-flex", alignItems: "center", justifyContent: "center",
      transition: "background 0.3s",
    }}>
      <span style={{
        width: inner, height: inner, borderRadius: "50%",
        background: "var(--accent-gradient)",
        display: "inline-flex", alignItems: "center", justifyContent: "center",
        color: "white", fontWeight: 700, fontSize: Math.max(10, size * 0.32),
      }}>
        {initials}
      </span>
    </span>
  );
}
/** Custom stepper replacing raw number inputs (§6). */
export function Stepper({ value, onChange, min = 1, max = 240, step = 5, marks }: {
  value: number; onChange: (v: number) => void; min?: number; max?: number; step?: number; marks?: number[];
}) {
  const clamp = (v: number) => Math.max(min, Math.min(max, v));
  const btn: React.CSSProperties = {
    width: 30, height: 30, borderRadius: 8, border: "1px solid var(--color-border)",
    background: "var(--color-surface-raised)", color: "var(--color-primary)",
    fontSize: 16, fontWeight: 700, lineHeight: 1, cursor: "pointer",
  };
  return (
    <div>
      <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
        <button style={btn} onClick={() => onChange(clamp(value - step))} aria-label="Decrease">−</button>
        <span style={{ fontSize: 22, fontWeight: 700, fontVariantNumeric: "tabular-nums", minWidth: 56, textAlign: "center" }}>
          {value}
        </span>
        <button style={btn} onClick={() => onChange(clamp(value + step))} aria-label="Increase">+</button>
        <span style={{ fontSize: 12, color: "var(--color-text-muted)" }}>minutes</span>
      </div>
      {marks && marks.length > 0 && (
        <div style={{ display: "flex", gap: 6, marginTop: 10 }}>
          {marks.map(m => (
            <button key={m} onClick={() => onChange(m)} style={{
              padding: "3px 10px", borderRadius: 999, fontSize: 11, fontWeight: value === m ? 700 : 500,
              border: "1px solid var(--color-border)", cursor: "pointer",
              background: value === m ? "var(--accent-glow)" : "transparent",
              color: value === m ? "var(--color-primary)" : "var(--color-text-secondary)",
            }}>
              {m}m
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

/** Segmented theme control with mini previews (§6). */
export function ThemeSegmented({ value, onChange }: {
  value: "dark" | "light" | "system"; onChange: (v: "dark" | "light" | "system") => void;
}) {
  const options: { key: "dark" | "light" | "system"; label: string; bg: string; fg: string; bar: string }[] = [
    { key: "dark", label: "Dark", bg: "#0A0B0F", fg: "#F5F6FA", bar: "#3B6BFF" },
    { key: "light", label: "Light", bg: "#FFFFFF", fg: "#16181D", bar: "#3B6BFF" },
    { key: "system", label: "System", bg: "linear-gradient(90deg, #0A0B0F 50%, #FFFFFF 50%)", fg: "#9497A8", bar: "#9497A8" },
  ];
  return (
    <div style={{ display: "flex", gap: 10 }} role="radiogroup" aria-label="Appearance">
      {options.map(o => {
        const active = value === o.key;
        return (
          <button key={o.key} role="radio" aria-checked={active} onClick={() => onChange(o.key)} style={{
            flex: 1, borderRadius: 10, overflow: "hidden", cursor: "pointer",
            border: active ? "2px solid var(--color-primary)" : "1px solid var(--color-border)",
            background: "transparent", padding: 0,
            boxShadow: active ? "0 0 10px var(--accent-glow)" : undefined,
            transition: "border-color 0.2s, box-shadow 0.2s",
          }}>
            <span style={{ display: "block", background: o.bg, padding: "12px 8px 8px" }}>
              <span style={{ display: "block", height: 6, borderRadius: 3, background: o.bar, opacity: 0.85, marginBottom: 6 }} />
              <span style={{ display: "block", height: 5, borderRadius: 3, background: o.fg, opacity: 0.35, width: "70%", marginBottom: 4 }} />
              <span style={{ display: "block", height: 5, borderRadius: 3, background: o.fg, opacity: 0.2, width: "50%" }} />
            </span>
            <span style={{
              display: "block", padding: "6px 0", fontSize: 12,
              fontWeight: active ? 700 : 500,
              color: active ? "var(--color-primary)" : "var(--color-text-secondary)",
              background: "var(--color-surface)",
            }}>
              {o.label}
            </span>
          </button>
        );
      })}
    </div>
  );
}
