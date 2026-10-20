import { useEffect, useMemo, useRef, useState } from "react";
import {
  CornerDownLeft,
  EyeOff,
  Gauge,
  LayoutGrid,
  Moon,
  Play,
  Plus,
  RefreshCw,
  RotateCcw,
  ScrollText,
  Rows3,
  Search,
  Settings2,
  Trash2,
  UserRound,
  Users,
  Workflow,
  XCircle,
  type LucideIcon,
} from "lucide-react";
import type { Account, AppConfig, AppSnapshot } from "@/generated/contracts";
import { useT } from "@/i18n";
import type { View } from "./navigation";
import { usePrivacy } from "@/lib/privacy";

interface Command {
  id: string;
  icon: LucideIcon;
  label: string;
  hint?: string;
  run: () => void;
}

const VIEW_ICONS: Record<Exclude<View, "trash">, LucideIcon> = {
  accounts: Users,
  dashboard: Gauge,
  friends: UserRound,
  automation: Workflow,
  settings: Settings2,
};
const ACCOUNT_LIMIT = 8;

/** Ctrl+K: jump to a view, an account or a preset, or run a common action by typing. */
export function CommandPalette({
  snapshot,
  onView,
  onAdd,
  onAccount,
  onRunPreset,
  onConfig,
  onRefreshPresence,
  onCloseAllRoblox,
  onArrange,
  onChangelog,
}: {
  snapshot: AppSnapshot;
  onView: (view: View) => void;
  onAdd: () => void;
  onAccount: (account: Account) => void;
  onRunPreset: (id: string, name: string) => void;
  onConfig: (patch: Partial<AppConfig>) => void;
  onRefreshPresence: () => void;
  onCloseAllRoblox: () => void;
  onArrange: () => void;
  onChangelog: () => void;
}) {
  const t = useT();
  const privacy = usePrivacy();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const list = useRef<HTMLUListElement>(null);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (
        (event.ctrlKey || event.metaKey) &&
        !event.altKey &&
        event.key.toLowerCase() === "k"
      ) {
        event.preventDefault();
        setQuery("");
        setActive(0);
        setOpen((value) => !value);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const config = snapshot.config;
  const commands = useMemo<Command[]>(() => {
    const views = (Object.keys(VIEW_ICONS) as (keyof typeof VIEW_ICONS)[]).map(
      (view) => ({
        id: `view:${view}`,
        icon: VIEW_ICONS[view],
        label: t("palette.goTo", { view: t(`views.${view}`) }),
        run: () => onView(view),
      }),
    );
    return [
      ...views,
      {
        id: "view:trash",
        icon: Trash2,
        label: t("palette.goTo", { view: t("accounts.trashTitle") }),
        run: () => onView("trash"),
      },
      {
        id: "add",
        icon: Plus,
        label: t("accounts.add"),
        run: onAdd,
      },
      ...config.launchPresets.map((preset) => ({
        id: `preset:${preset.id}`,
        icon: Play,
        label: t("palette.runPreset", { name: preset.name }),
        run: () => onRunPreset(preset.id, preset.name),
      })),
      {
        id: "presence",
        icon: RefreshCw,
        label: t("palette.refreshPresence"),
        run: onRefreshPresence,
      },
      {
        id: "afk",
        icon: Moon,
        label: t(config.afkMode ? "palette.afkOff" : "palette.afkOn"),
        run: () => onConfig({ afkMode: !config.afkMode }),
      },
      {
        id: "rejoin",
        icon: RotateCcw,
        label: t(config.autoRejoin ? "palette.rejoinOff" : "palette.rejoinOn"),
        run: () => onConfig({ autoRejoin: !config.autoRejoin }),
      },
      {
        id: "compact",
        icon: Rows3,
        label: t(
          config.compactList ? "palette.compactOff" : "palette.compactOn",
        ),
        run: () => onConfig({ compactList: !config.compactList }),
      },
      {
        id: "streamer",
        icon: EyeOff,
        label: t(
          config.streamerMode ? "palette.streamerOff" : "palette.streamerOn",
        ),
        run: () => onConfig({ streamerMode: !config.streamerMode }),
      },
      {
        id: "arrange",
        icon: LayoutGrid,
        label: t("palette.arrange"),
        run: onArrange,
      },
      {
        id: "close-roblox",
        icon: XCircle,
        label: t("palette.closeRoblox"),
        run: onCloseAllRoblox,
      },
      {
        id: "changelog",
        icon: ScrollText,
        label: t("changelog.title"),
        run: onChangelog,
      },
    ];
  }, [
    t,
    onChangelog,
    config.launchPresets,
    config.afkMode,
    config.autoRejoin,
    config.compactList,
    config.streamerMode,
    onView,
    onAdd,
    onRunPreset,
    onRefreshPresence,
    onConfig,
    onCloseAllRoblox,
    onArrange,
  ]);

  const text = query.trim().toLowerCase();
  const matches = useMemo(() => {
    const found = commands.filter((command) =>
      command.label.toLowerCase().includes(text),
    );
    if (!text) return found;
    const accounts: Command[] = snapshot.accounts
      .filter(
        (account) =>
          !account.deletedAt &&
          (account.username.toLowerCase().includes(text) ||
            account.alias.toLowerCase().includes(text)),
      )
      .slice(0, ACCOUNT_LIMIT)
      .map((account) => ({
        id: `account:${account.id}`,
        icon: UserRound,
        label: privacy.name(account),
        hint: t("palette.account"),
        run: () => onAccount(account),
      }));
    return [...found, ...accounts];
  }, [commands, snapshot.accounts, text, t, onAccount, privacy]);
  const selected = Math.min(active, Math.max(0, matches.length - 1));

  useEffect(() => {
    list.current
      ?.querySelector<HTMLElement>(`[data-index="${selected}"]`)
      ?.scrollIntoView({ block: "nearest" });
  }, [selected]);

  if (!open) return null;
  const choose = (command: Command | undefined) => {
    if (!command) return;
    setOpen(false);
    command.run();
  };
  return (
    <div className="palette-backdrop" onMouseDown={() => setOpen(false)}>
      <div
        className="palette"
        role="dialog"
        aria-modal="true"
        aria-label={t("palette.title")}
        onMouseDown={(event) => event.stopPropagation()}
      >
        <label className="palette-search">
          <Search size={15} />
          <input
            autoFocus
            role="combobox"
            aria-expanded="true"
            aria-controls="palette-list"
            aria-activedescendant={matches[selected]?.id}
            aria-label={t("palette.placeholder")}
            placeholder={t("palette.placeholder")}
            value={query}
            onChange={(event) => {
              setQuery(event.target.value);
              setActive(0);
            }}
            onKeyDown={(event) => {
              if (event.key === "ArrowDown") {
                event.preventDefault();
                setActive((selected + 1) % Math.max(1, matches.length));
              } else if (event.key === "ArrowUp") {
                event.preventDefault();
                setActive(
                  (selected - 1 + matches.length) % Math.max(1, matches.length),
                );
              } else if (event.key === "Enter") {
                event.preventDefault();
                choose(matches[selected]);
              } else if (event.key === "Escape") {
                event.preventDefault();
                setOpen(false);
              }
            }}
          />
          <kbd>Esc</kbd>
        </label>
        <ul id="palette-list" role="listbox" ref={list}>
          {matches.map((command, index) => (
            <li
              key={command.id}
              id={command.id}
              role="option"
              data-index={index}
              aria-selected={index === selected}
              className={index === selected ? "active" : ""}
              onMouseMove={() => setActive(index)}
              onClick={() => choose(command)}
            >
              <command.icon size={15} />
              <span>{command.label}</span>
              {command.hint && <small>{command.hint}</small>}
              {index === selected && <CornerDownLeft size={13} />}
            </li>
          ))}
          {matches.length === 0 && (
            <li className="palette-empty">{t("palette.empty")}</li>
          )}
        </ul>
      </div>
    </div>
  );
}
