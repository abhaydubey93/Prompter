import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import OverlayApp from "./overlay/OverlayApp";
import "./index.css";

// Two Tauri windows share this bundle, distinguished by URL hash.
//   index.html        -> main window (Settings + Library)
//   index.html#/overlay -> overlay window
const isOverlay = window.location.hash.includes("overlay");

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>{isOverlay ? <OverlayApp /> : <App />}</React.StrictMode>,
);
