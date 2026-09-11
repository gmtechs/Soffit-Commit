import React from "react";

type Variant = "primary" | "secondary" | "dark" | "destructive" | "ghost";

interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  size?: "sm" | "md";
}

export function Button({ variant = "primary", size = "md", style, children, disabled, ...props }: ButtonProps) {
  const base: React.CSSProperties = {
    display: "inline-flex", alignItems: "center", justifyContent: "center", gap: 6,
    fontWeight: 500, fontFamily: "inherit", borderRadius: 10, border: "none",
    cursor: disabled ? "not-allowed" : "pointer",
    opacity: disabled ? 0.5 : 1,
    transition: "opacity 0.15s",
    fontSize: size === "sm" ? 12 : 13,
    padding: size === "sm" ? "5px 10px" : "8px 16px",
    lineHeight: 1,
  };

  const variants: Record<Variant, React.CSSProperties> = {
    primary:     { background: "var(--color-primary)", color: "white" },
    secondary:   { background: "var(--color-surface)", color: "var(--color-ink)", border: "1px solid var(--color-border)" },
    dark:        { background: "var(--color-ink)", color: "white" },
    destructive: { background: "transparent", color: "var(--color-danger)" },
    ghost:       { background: "transparent", color: "var(--color-ink)" },
  };

  return (
    <button disabled={disabled} style={{ ...base, ...variants[variant], ...style }} {...props}>
      {children}
    </button>
  );
}
