import { useEffect, useLayoutEffect, useRef, useState } from "react";
import {
  Archive,
  Eye,
  HardDrive,
  Search,
  Settings2,
  Shield,
  SlidersHorizontal,
  X,
  type LucideIcon,
} from "lucide-react";
import type {
  RuntimeInfo,
  AppConfig,
  ClientState,
  LauncherState,
} from "@/generated/contracts";
import { useUiPreference } from "@/hooks/useUiPreference";
import { useT, type MessageKey } from "@/i18n";
import { AppearanceSettings } from "./appearance/AppearanceSettings";
import { DataSettings } from "./categories/DataSettings";
import { SessionChecksSettings } from "./categories/SessionChecksSettings";
import { GeneralSettings } from "./categories/GeneralSettings";
import { ClientCareSettings } from "./categories/ClientCareSettings";
import { LaunchSettings } from "./categories/LaunchSettings";
import { ClientSettings } from "./ClientSettings";
import type { SettingsCategory } from "./settingsCategory";
import { SettingsSearchProvider, sectionAnchor } from "./SettingControls";

type CategoryId = SettingsCategory;

// Sections are listed under their category for jumping straight to them; they are the
// `id`s of the SettingSections the category renders.
const CATEGORIES: {
  id: CategoryId;
  icon: LucideIcon;
  sections: { id: string; title: MessageKey }[];
}[] = [
  {
    id: "general",
    icon: Settings2,
    sections: [
      { id: "general", title: "settings.general.title" },
      { id: "clipboard", title: "clipboard.title" },
      { id: "system", title: "settings.system.title" },
      { id: "hotkeys", title: "hotkeys.title" },
    ],
  },
  {
    id: "appearance",
    icon: Eye,
    sections: [
      { id: "theme", title: "settings.appearance.theme" },
      { id: "display", title: "settings.appearance.display" },
      { id: "quick-actions", title: "settings.appearance.quickActions" },
    ],
  },
  {
    id: "checks",
    icon: SlidersHorizontal,
    sections: [
      { id: "concurrency", title: "settings.checks.concurrency" },
      { id: "timing", title: "settings.checks.timing" },
    ],
  },
  {
    id: "launch",
    icon: Shield,
    sections: [
      { id: "launch", title: "settings.launch.title" },
      { id: "performance", title: "settings.performance.title" },
      { id: "anti-afk", title: "settings.antiAfk.title" },
      { id: "clients", title: "settings.clients.title" },
      { id: "profiles", title: "settings.profiles.title" },
      { id: "recovery", title: "settings.recovery.title" },
    ],
  },
  {
    id: "client",
    icon: HardDrive,
    sections: [
      { id: "client-install", title: "settings.client.installation" },
      { id: "client-performance", title: "settings.client.performance" },
      { id: "client-updates", title: "settings.client.updates" },
      { id: "client-flags", title: "client.flags.title" },
      { id: "client-versions", title: "settings.client.versions" },
    ],
  },
  {
    id: "data",
    icon: Archive,
    sections: [
      { id: "backup", title: "settings.data.backup" },
      { id: "legacy", title: "settings.data.legacy" },
    ],
  },
];

/**
 * Settings by category, with a search across all of them: while searching, every category
 * renders and rows that do not match hide themselves.
 */
export function SettingsView({
  config,
  launcher,
  client,
  runtime,
  onChange,
}: {
  config: AppConfig;
  launcher: LauncherState;
  client: ClientState;
  runtime: RuntimeInfo;
  onChange: (patch: Partial<AppConfig>) => void;
}) {
  const t = useT();
  const [storedCategory, setCategory] = useUiPreference<string>(
    "settingsCategory",
    "general",
  );
  const category =
    CATEGORIES.find((item) => item.id === storedCategory)?.id ?? "general";
  const [section, setSection] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [noResults, setNoResults] = useState(false);
  const searchRef = useRef<HTMLInputElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const searching = query.trim().length > 0;
  const show = (id: CategoryId) => searching || category === id;
  const open = (id: CategoryId, sectionId: string | null = null) => {
    setQuery("");
    setCategory(id);
    setSection(sectionId);
  };

  // Scroll to the chosen section once its category has rendered.
  useEffect(() => {
    const content = contentRef.current;
    if (!content || searching) return;
    if (section)
      document
        .getElementById(sectionAnchor(section))
        ?.scrollIntoView({ block: "start" });
    else content.scrollTop = 0;
  }, [category, section, searching]);
  // Sections hide themselves when nothing in them matches, so count what is left in the DOM.
  useLayoutEffect(() => {
    setNoResults(
      searching &&
        ![
          ...(contentRef.current?.querySelectorAll(".settings-section") ?? []),
        ].some((element) => (element as HTMLElement).offsetParent !== null),
    );
  }, [query, searching]);
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "f") {
        event.preventDefault();
        searchRef.current?.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  return (
    <div className="settings-layout">
      <aside className="settings-navigation">
        <label className="search-field settings-search">
          <Search size={13} />
          <input
            ref={searchRef}
            aria-label={t("settings.search")}
            placeholder={t("settings.search")}
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Escape") setQuery("");
            }}
          />
          {query && (
            <button
              aria-label={t("settings.clearSearch")}
              onClick={() => setQuery("")}
            >
              <X size={12} />
            </button>
          )}
        </label>
        <nav aria-label={t("settings.categories")}>
          {CATEGORIES.map((item) => {
            const current = !searching && category === item.id;
            return (
              <div key={item.id}>
                <button
                  className={current ? "active" : ""}
                  aria-current={current ? "page" : undefined}
                  onClick={() => open(item.id)}
                >
                  <item.icon size={14} />
                  {t(`settings.category.${item.id}`)}
                </button>
                {item.sections.length > 0 && (
                  <div className="settings-subnav">
                    {item.sections.map((entry) => (
                      <button
                        key={entry.id}
                        className={
                          current && section === entry.id ? "active" : ""
                        }
                        onClick={() => open(item.id, entry.id)}
                      >
                        {t(entry.title)}
                      </button>
                    ))}
                  </div>
                )}
              </div>
            );
          })}
        </nav>
      </aside>
      <div
        ref={contentRef}
        className={`settings-content ${searching ? "searching" : ""}`}
      >
        <SettingsSearchProvider value={searching ? query.trim() : ""}>
          {noResults && (
            <div className="panel-empty settings-no-results">
              {t("settings.noResults", { query: query.trim() })}
            </div>
          )}
          {show("general") && (
            <GeneralSettings config={config} onChange={onChange} />
          )}
          {show("appearance") && (
            <AppearanceSettings
              config={config}
              runtime={runtime}
              onChange={onChange}
            />
          )}
          {show("checks") && (
            <SessionChecksSettings config={config} onChange={onChange} />
          )}
          {show("launch") && (
            <>
              <LaunchSettings
                config={config}
                launcher={launcher}
                onChange={onChange}
              />
              <ClientCareSettings config={config} onChange={onChange} />
            </>
          )}
          {show("client") && (
            <ClientSettings
              config={config}
              client={client}
              onChange={onChange}
            />
          )}
          {show("data") && <DataSettings />}
        </SettingsSearchProvider>
      </div>
    </div>
  );
}
