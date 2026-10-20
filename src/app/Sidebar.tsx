import {
  ChevronsLeft,
  ChevronsRight,
  Gauge,
  Settings2,
  Trash2,
  UserRound,
  Users,
  Workflow,
} from "lucide-react";
import type { Account, Folder } from "@/generated/contracts";
import { Button } from "@/components/ui/button";
import { useUiPreference } from "@/hooks/useUiPreference";
import { useT } from "@/i18n";
import { useFormat } from "@/i18n/format";
import { FolderPanel } from "@/features/folders/FolderPanel";
import type { RunAction } from "@/lib/actions";
import type { FolderSelection, View } from "./navigation";

const VIEWS = [
  { id: "accounts", icon: Users, shortcut: "Ctrl+1" },
  { id: "dashboard", icon: Gauge, shortcut: "Ctrl+2" },
  { id: "friends", icon: UserRound, shortcut: "Ctrl+5" },
  { id: "automation", icon: Workflow, shortcut: "Ctrl+6" },
  { id: "settings", icon: Settings2, shortcut: "Ctrl+4" },
] as const;

/** Main navigation; in the accounts views also the folder tree and Trash. */
export function Sidebar({
  view,
  onView,
  accounts,
  folders,
  selectedFolder,
  onFolder,
  onMoveAccounts,
  palette,
  defaultFolderColor,
  onPalette,
  run,
}: {
  view: View;
  onView: (view: View) => void;
  accounts: Account[];
  folders: Folder[];
  selectedFolder: FolderSelection;
  onFolder: (id: FolderSelection) => void;
  onMoveAccounts: (ids: number[], folder: number | null) => void;
  palette: string[];
  defaultFolderColor: string;
  onPalette: (colors: string[]) => void;
  run: RunAction;
}) {
  const t = useT();
  const format = useFormat();
  const [collapsed, setCollapsed] = useUiPreference("sidebarCollapsed", false);
  const active = accounts.filter((account) => !account.deletedAt).length;
  const trash = accounts.length - active;
  // Folders and Trash only organize accounts, so they appear only there.
  const accountsContext = view === "accounts" || view === "trash";
  const toggleLabel = t(collapsed ? "sidebar.expand" : "sidebar.collapse");
  return (
    <aside className={`sidebar ${collapsed ? "collapsed" : ""}`}>
      <div className="workspace-label">
        <span>NOCTA</span>
        <Button
          variant="ghost"
          size="icon"
          className="sidebar-toggle"
          aria-label={toggleLabel}
          title={toggleLabel}
          onClick={() => setCollapsed(!collapsed)}
        >
          {collapsed ? <ChevronsRight /> : <ChevronsLeft />}
        </Button>
      </div>
      <nav aria-label={t("sidebar.navigation")}>
        {VIEWS.map((item) => {
          const label = t(`views.${item.id}`);
          return (
            <button
              key={item.id}
              className={`nav-item ${view === item.id ? "active" : ""}`}
              onClick={() => onView(item.id)}
              title={`${label} (${item.shortcut})`}
              aria-current={view === item.id ? "page" : undefined}
            >
              <item.icon size={15} />
              <span>{label}</span>
              {item.id === "accounts" && (
                <span className="nav-count">{format.number(active)}</span>
              )}
            </button>
          );
        })}
      </nav>
      {accountsContext && (
        <>
          <FolderPanel
            accounts={accounts}
            folders={folders}
            selected={view === "accounts" ? selectedFolder : undefined}
            collapsed={collapsed}
            palette={palette}
            defaultColor={defaultFolderColor}
            run={run}
            onSelect={onFolder}
            onMoveAccounts={onMoveAccounts}
            onPalette={onPalette}
          />
          <div className="sidebar-bottom">
            <button
              className={`nav-item ${view === "trash" ? "active" : ""}`}
              onClick={() => onView("trash")}
              title={t("accounts.trashTitle")}
              aria-current={view === "trash" ? "page" : undefined}
            >
              <Trash2 size={14} />
              <span>{t("accounts.trashTitle")}</span>
              <span className="nav-count">{trash}</span>
            </button>
          </div>
        </>
      )}
    </aside>
  );
}
