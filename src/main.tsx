if (import.meta.env.MODE === "e2e") {
  void import("@wdio/tauri-plugin");
}
import React from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import "./styles.css";
createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
