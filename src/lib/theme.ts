import { useEffect, useState } from "react";
import { getSetting } from "./tauri";

export type Theme = "dark" | "light";
const isTheme = (value: unknown): value is Theme => value === "dark" || value === "light";

export function currentTheme(): Theme {
  return document.documentElement.dataset.theme === "light" ? "light" : "dark";
}

export function applyTheme(theme: Theme) {
  document.documentElement.dataset.theme = theme;
  try { localStorage.setItem("soffit-theme", theme); } catch { /* Storage may be unavailable. */ }
}

/** The first-paint cache wins; consult native settings only on older installations. */
export async function initializeTheme() {
  try {
    if (isTheme(localStorage.getItem("soffit-theme"))) return;
  } catch { /* Keep the dark default if storage is unavailable. */ }
  const initialTheme = currentTheme();
  try {
    const saved = await getSetting("theme");
    // Do not overwrite a preference changed while the native read was in flight.
    try {
      if (isTheme(localStorage.getItem("soffit-theme"))) return;
    } catch { /* Fall back to the current document theme. */ }
    if (isTheme(saved) && currentTheme() === initialTheme) applyTheme(saved);
  } catch { /* Missing settings never override the dark-first shell. */ }
}

/** Keep Settings and the top-bar toggle in sync with the same document theme. */
export function useTheme(): Theme {
  const [theme, setTheme] = useState<Theme>(currentTheme);
  useEffect(() => {
    const update = () => setTheme(currentTheme());
    const observer = new MutationObserver(update);
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
    update();
    return () => observer.disconnect();
  }, []);
  return theme;
}
