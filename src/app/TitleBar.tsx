import { Globe, Minus, Square, X } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useT } from "@/i18n";
import { api, desktop } from "@/lib/ipc";

const HOMEPAGE = "https://7moons.dev";

/** Opens the makers' website: through the backend in the app, as a new tab in the preview. */
function openHomepage() {
  if (desktop) void api.openHomepage();
  else window.open(HOMEPAGE, "_blank", "noopener,noreferrer");
}

export function TitleBar() {
  const t = useT();
  return (
    <div className="titlebar">
      <div className="brand-mark" aria-label="Nocta">
        <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
          <path
            d="M5 19 9 5l6 14 4-14M7 13l10-2"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
      </div>
      <div
        className="titlebar-drag"
        data-tauri-drag-region
        onDoubleClick={() => {
          if (desktop) void getCurrentWindow().toggleMaximize();
        }}
      >
        <span className="titlebar-title">
          Nocta
          <span className="titlebar-credit">
            {t("app.by")}
            <button
              type="button"
              className="titlebar-link"
              title={t("app.openWebsite")}
              onClick={openHomepage}
              onDoubleClick={(event) => event.stopPropagation()}
            >
              <Globe size={11} aria-hidden="true" />
              7moons.dev
            </button>
          </span>
        </span>
      </div>
      <div className="window-controls">
        <button
          aria-label={t("app.window.minimize")}
          disabled={!desktop}
          onClick={() => void getCurrentWindow().minimize()}
        >
          <Minus size={13} />
        </button>
        <button
          aria-label={t("app.window.maximize")}
          disabled={!desktop}
          onClick={() => void getCurrentWindow().toggleMaximize()}
        >
          <Square size={11} />
        </button>
        <button
          className="window-close"
          aria-label={t("app.window.close")}
          disabled={!desktop}
          onClick={() => void getCurrentWindow().close()}
        >
          <X size={15} />
        </button>
      </div>
    </div>
  );
}
