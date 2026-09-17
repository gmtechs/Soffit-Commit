import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";
import { initializeTheme } from "./lib/theme";

// The HTML shell applies the cached preference before paint; migrate native settings once.
void initializeTheme();

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
