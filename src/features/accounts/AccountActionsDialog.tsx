import { useState, type ReactNode } from "react";
import {
  Clipboard,
  Globe,
  KeyRound,
  Pencil,
  Play,
  RefreshCw,
  RotateCcw,
  Settings2,
  ShieldCheck,
  Trash2,
  type LucideIcon,
} from "lucide-react";
import type { Account, Folder } from "@/generated/contracts";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { useT, type MessageKey } from "@/i18n";
import { useFormat } from "@/i18n/format";
import type { RestoreDestination } from "./accountActions";
import { ClipboardFormatManager } from "./ClipboardFormatManager";
import {
  clipboardFormatStore,
  formatName,
  useClipboardFormats,
} from "./clipboardFormats";
import { previewTemplate } from "./clipboardTemplate";
import { AccountAvatar } from "./AccountName";
import { HealthBadge } from "./HealthBadge";
import { TotpPanel } from "./TotpPanel";
import { usePrivacy } from "@/lib/privacy";
import { RobloxAccountTools } from "./RobloxAccountTools";

export type SessionAction =
  "check_health" | "validate_cookie" | "open_browser" | "refresh_cookie";

const SESSION_TILES: { action: SessionAction; icon: LucideIcon }[] = [
  { action: "open_browser", icon: Globe },
  { action: "check_health", icon: ShieldCheck },
  { action: "validate_cookie", icon: KeyRound },
  { action: "refresh_cookie", icon: RefreshCw },
];

function cookieState(account: Account): MessageKey {
  if (!account.hasCookie) return "accounts.actions.cookie.missing";
  if (account.cookieValid === null) return "accounts.actions.cookie.unchecked";
  return account.cookieValid
    ? "accounts.actions.cookie.valid"
    : "accounts.actions.cookie.invalid";
}

/** Everything that can be done with one account, opened from the row menu or a right click. */
export function AccountActionsDialog({
  account,
  folders,
  trash,
  canLaunch,
  sessionsReady,
  onClose,
  onEdit,
  onLaunch,
  onSession,
  onCopy,
  onTrash,
  onRestore,
}: {
  account: Account | null;
  folders: Folder[];
  trash: boolean;
  canLaunch: boolean;
  sessionsReady: boolean;
  onClose: () => void;
  onEdit: (account: Account) => void;
  onLaunch: (account: Account) => void;
  onSession: (account: Account, action: SessionAction) => void;
  onCopy: (account: Account) => void;
  onTrash: (account: Account) => void;
  onRestore: (account: Account, destination: RestoreDestination) => void;
}) {
  const t = useT();
  const privacy = usePrivacy();
  const format = useFormat();
  const { formats, selected } = useClipboardFormats();
  const [managing, setManaging] = useState(false);
  const [restoreTo, setRestoreTo] = useState<RestoreDestination>("original");
  const folderName = (id: number | null) =>
    folders.find((folder) => folder.id === id)?.name ??
    t("accounts.folders.general");
  // Runs an action and closes the dialog, so the row shows the result.
  const act = (action: () => void) => {
    action();
    onClose();
  };
  return (
    <>
      <Dialog
        open={account !== null && !managing}
        onOpenChange={(open) => {
          if (!open) onClose();
        }}
        title={
          account
            ? privacy.username(account)
            : t("accounts.actions.fallbackTitle")
        }
        description={t(
          trash ? "accounts.actions.deleted" : "accounts.actions.subtitle",
        )}
      >
        {account && (
          <div className="dialog-body account-actions">
            <div className="account-summary">
              <AccountAvatar account={account} />
              <div>
                <strong>{privacy.username(account)}</strong>
                <small>
                  {folderName(account.folderId)} · #{account.id}
                  {account.userId &&
                    !privacy.hidden &&
                    ` · ${t("accounts.actions.robloxId", { id: account.userId })}`}
                </small>
              </div>
              <HealthBadge health={account.health} />
            </div>
            <dl className="account-facts">
              <div>
                <dt>{t("accounts.actions.cookie.label")}</dt>
                <dd>{t(cookieState(account))}</dd>
              </div>
              <div>
                <dt>{t("accounts.table.lastStarted")}</dt>
                <dd>{format.date(account.lastStartedAt)}</dd>
              </div>
              <div>
                <dt>{t("accounts.table.lastChecked")}</dt>
                <dd>{format.date(account.checkedAt)}</dd>
              </div>
              <div>
                <dt>{t("accounts.robux.label")}</dt>
                <dd
                  title={format.dateTime(account.robuxCheckedAt) ?? undefined}
                >
                  {account.robux === null
                    ? t("accounts.robux.unknown")
                    : format.number(account.robux)}
                </dd>
              </div>
              <div>
                <dt>{t("accounts.playtime.label")}</dt>
                <dd>
                  {t("accounts.playtime.value", {
                    hours: Math.floor(account.playtimeSecs / 3600),
                    minutes: Math.floor((account.playtimeSecs % 3600) / 60),
                  })}
                </dd>
              </div>
            </dl>
            {account.note && (
              <p className="account-note-full">{account.note}</p>
            )}
            {account.fields.length > 0 && (
              <dl className="account-facts account-own-fields">
                {account.fields.map((field) => (
                  <div key={field.name}>
                    <dt>{field.name}</dt>
                    <dd>{field.value}</dd>
                  </div>
                ))}
              </dl>
            )}

            {!trash && (
              <section className="action-group">
                <h4>{t("accounts.actions.session")}</h4>
                <div className="action-tiles">
                  <ActionTile
                    icon={<Play />}
                    label={t("quickActions.launch")}
                    hint={t("accounts.actions.hint.launch")}
                    primary
                    disabled={!canLaunch}
                    onClick={() => act(() => onLaunch(account))}
                  />
                  {SESSION_TILES.map(({ action, icon: Icon }) => (
                    <ActionTile
                      key={action}
                      icon={<Icon />}
                      label={t(`quickActions.${action}`)}
                      hint={t(`accounts.actions.hint.${action}`)}
                      disabled={!sessionsReady}
                      onClick={() => act(() => onSession(account, action))}
                    />
                  ))}
                  <ActionTile
                    icon={<Pencil />}
                    label={t("quickActions.edit")}
                    hint={t("accounts.actions.hint.edit")}
                    onClick={() => act(() => onEdit(account))}
                  />
                </div>
              </section>
            )}

            {!trash && account.hasTotp && (
              <section className="action-group">
                <h4>{t("accounts.totp.label")}</h4>
                <TotpPanel accountId={account.id} />
              </section>
            )}

            {!trash && account.userId && account.hasCookie && (
              <section className="action-group">
                <h4>{t("accounts.roblox.title")}</h4>
                <RobloxAccountTools accountId={account.id} />
              </section>
            )}

            <section className="action-group">
              <h4>{t("accounts.actions.copyTitle")}</h4>
              <div className="copy-row">
                <select
                  aria-label={t("accounts.selection.clipboardFormat")}
                  value={selected.id}
                  onChange={(event) =>
                    clipboardFormatStore.select(event.target.value)
                  }
                >
                  {formats.map((item) => (
                    <option key={item.id} value={item.id}>
                      {formatName(item, t)}
                    </option>
                  ))}
                </select>
                <Button onClick={() => onCopy(account)}>
                  <Clipboard />
                  {t("accounts.selection.copy")}
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  title={t("clipboard.manage")}
                  aria-label={t("clipboard.manageLabel")}
                  onClick={() => setManaging(true)}
                >
                  <Settings2 />
                </Button>
              </div>
              <code className="copy-preview">
                {privacy.hidden
                  ? t("privacy.hiddenPreview")
                  : previewTemplate(selected.template, {
                      username: account.username,
                      userId: account.userId,
                      id: account.id,
                      folder: folderName(account.folderId),
                    })}
              </code>
            </section>

            <div className="dialog-footer account-actions-footer">
              {trash ? (
                <>
                  <select
                    aria-label={t("accounts.actions.restoreTo")}
                    value={restoreTo}
                    onChange={(event) =>
                      setRestoreTo(event.target.value as RestoreDestination)
                    }
                  >
                    <option value="original">
                      {t("accounts.folders.original")}
                    </option>
                    <option value="general">
                      {t("accounts.folders.general")}
                    </option>
                    {folders.map((folder) => (
                      <option key={folder.id} value={folder.id}>
                        {folder.name}
                      </option>
                    ))}
                  </select>
                  <Button
                    variant="outline"
                    onClick={() => act(() => onRestore(account, restoreTo))}
                  >
                    <RotateCcw />
                    {t("accounts.selection.restore")}
                  </Button>
                </>
              ) : (
                <span />
              )}
              <Button
                variant="destructive"
                onClick={() => act(() => onTrash(account))}
              >
                <Trash2 />
                {t(
                  trash
                    ? "accounts.selection.deleteForever"
                    : "quickActions.trash",
                )}
              </Button>
            </div>
          </div>
        )}
      </Dialog>
      <Dialog
        open={managing}
        onOpenChange={(open) => {
          if (!open) setManaging(false);
        }}
        title={t("clipboard.title")}
        description={t("clipboard.subtitle")}
      >
        <div className="dialog-body">
          <ClipboardFormatManager />
        </div>
      </Dialog>
    </>
  );
}

function ActionTile({
  icon,
  label,
  hint,
  primary,
  disabled,
  onClick,
}: {
  icon: ReactNode;
  label: string;
  hint: string;
  primary?: boolean;
  disabled?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      className={`action-tile ${primary ? "primary" : ""}`}
      disabled={disabled}
      onClick={onClick}
    >
      <span className="action-tile-icon">{icon}</span>
      <span>
        <strong>{label}</strong>
        <small>{hint}</small>
      </span>
    </button>
  );
}
