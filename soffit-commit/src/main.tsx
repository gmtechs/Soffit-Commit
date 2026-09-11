import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";
import { getSetting } from "./lib/tauri";

// Apply persisted theme before first render
getSetting("theme").then(t => {
  if (t) document.documentElement.setAttribute("data-theme", t);
}).catch(() => {
  // Fallback to OS preference
  if (window.matchMedia("(prefers-color-scheme: dark)").matches) {
    document.documentElement.setAttribute("data-theme", "dark");
  }
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);

// Keep the static local launch screen visible until React has committed a frame.
// This prevents a white WebKit surface while the app shell and settings load.
requestAnimationFrame(() => {
  document.getElementById("launch-screen")?.classList.add("is-ready");
});
