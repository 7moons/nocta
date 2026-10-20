import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "@/app/App";
import { languageStore } from "@/i18n";
import { themeStore } from "@/theme/store";
import "./styles/index.css";
// Apply the saved theme before the first render so the window never flashes the default.
themeStore.init();
// The saved language loads before the first render, so the window never flashes English.
const languageReady = languageStore.init();
// No browser context menu (Back, Reload, Inspect…) in a desktop app. Text fields keep it for
// copy and paste; the app's own right-click menus call preventDefault themselves.
window.addEventListener("contextmenu", (event) => {
  const target = event.target as HTMLElement | null;
  if (!target?.closest("input, textarea, [contenteditable='true']"))
    event.preventDefault();
});
void languageReady.finally(() =>
  createRoot(document.getElementById("root")!).render(
    <StrictMode>
      <App />
    </StrictMode>,
  ),
);
