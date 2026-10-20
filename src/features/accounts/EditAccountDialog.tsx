import { useState, type FormEvent, type ReactNode } from "react";
import {
  Check,
  Eye,
  EyeOff,
  KeyRound,
  Crown,
  Lock,
  Pin,
  Smartphone,
} from "lucide-react";
import type { Account, Folder } from "@/generated/contracts";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { useT } from "@/i18n";
import { api } from "@/lib/ipc";
import { errorMessage } from "@/lib/utils";
import { ACCOUNT_COLORS } from "./accountDetails";
import { HealthBadge } from "./HealthBadge";
import { usePrivacy } from "@/lib/privacy";
import { ClickPointEditor, FieldsEditor } from "./AccountExtras";

/** Edits one account's name, folder and credentials. */
export function EditAccountDialog({
  account,
  folders,
  onClose,
}: {
  account: Account | null;
  folders: Folder[];
  onClose: () => void;
}) {
  const t = useT();
  const [busy, setBusy] = useState(false);
  return (
    <Dialog
      open={account !== null}
      onOpenChange={(open) => {
        if (!open && !busy) onClose();
      }}
      title={t("accounts.edit.title")}
    >
      {account && (
        // Keyed by account, so every account starts from its own saved values.
        <EditAccountForm
          key={account.id}
          account={account}
          folders={folders}
          busy={busy}
          onBusy={setBusy}
          onClose={onClose}
        />
      )}
    </Dialog>
  );
}

function EditAccountForm({
  account,
  folders,
  busy,
  onBusy,
  onClose,
}: {
  account: Account;
  folders: Folder[];
  busy: boolean;
  onBusy: (busy: boolean) => void;
  onClose: () => void;
}) {
  const t = useT();
  const privacy = usePrivacy();
  const [username, setUsername] = useState(account.username);
  const [folderId, setFolderId] = useState(account.folderId?.toString() ?? "");
  // null keeps the stored secret; a string (even empty while typing) replaces it.
  const [password, setPassword] = useState<string | null>(null);
  const [cookie, setCookie] = useState<string | null>(null);
  const [showPassword, setShowPassword] = useState(false);
  const [alias, setAlias] = useState(account.alias);
  const [note, setNote] = useState(account.note);
  const [color, setColor] = useState(account.color);
  const [pinned, setPinned] = useState(account.pinned);
  const [isMain, setIsMain] = useState(account.isMain);
  const [fields, setFields] = useState(account.fields);
  const [afkPoint, setAfkPoint] = useState(account.afkPoint);
  // null keeps the saved key, "" removes it, anything else replaces it.
  const [totp, setTotp] = useState<string | null>(null);
  const [error, setError] = useState("");
  const accountChanged =
    username.trim() !== account.username ||
    folderId !== (account.folderId?.toString() ?? "") ||
    !!password ||
    !!cookie?.trim();
  const detailsChanged =
    alias.trim() !== account.alias ||
    note.trim() !== account.note ||
    color !== account.color ||
    pinned !== account.pinned ||
    isMain !== account.isMain ||
    JSON.stringify(fields) !== JSON.stringify(account.fields) ||
    JSON.stringify(afkPoint) !== JSON.stringify(account.afkPoint);
  const totpChanged = totp !== null && (totp.trim() !== "" || account.hasTotp);
  const changed = accountChanged || detailsChanged || totpChanged;

  async function submit(event: FormEvent) {
    event.preventDefault();
    onBusy(true);
    try {
      if (totpChanged)
        await api.setTotp(account.id, totp?.trim() ? totp.trim() : null);
      if (detailsChanged)
        await api.updateAccountDetails(account.id, {
          alias: alias.trim(),
          note: note.trim(),
          color,
          pinned,
          isMain,
          fields,
          afkPoint,
        });
      if (accountChanged)
        await api.updateAccount(account.id, {
          username: username.trim(),
          folderId: folderId ? Number(folderId) : null,
          password: password || null,
          cookie: cookie?.trim() || null,
        });
      onClose();
    } catch (failure) {
      setError(errorMessage(failure));
    } finally {
      onBusy(false);
    }
  }

  return (
    <form
      className="dialog-body edit-account"
      onSubmit={(event) => void submit(event)}
    >
      <div className="account-summary">
        <span className="avatar">
          {privacy.username(account).replace("#", "").slice(0, 1).toUpperCase()}
        </span>
        <div>
          <strong>{privacy.username(account)}</strong>
          <small>
            #{account.id}
            {account.userId &&
              !privacy.hidden &&
              ` · ${t("accounts.actions.robloxId", { id: account.userId })}`}
          </small>
        </div>
        <HealthBadge health={account.health} />
      </div>

      <section className="edit-group">
        <h4>{t("accounts.edit.details")}</h4>
        <div className="form-grid">
          <label className="field-label">
            {t("accounts.edit.username")}
            <input
              autoFocus
              // Streamer mode keeps the username off screen while it can still be edited.
              type={privacy.hidden ? "password" : "text"}
              autoComplete="off"
              value={username}
              onChange={(event) => setUsername(event.target.value)}
              required
            />
          </label>
          <label className="field-label">
            {t("accounts.table.folder")}
            <select
              value={folderId}
              onChange={(event) => setFolderId(event.target.value)}
            >
              <option value="">{t("accounts.folders.general")}</option>
              {folders.map((folder) => (
                <option key={folder.id} value={folder.id}>
                  {folder.name}
                </option>
              ))}
            </select>
          </label>
        </div>
      </section>

      <section className="edit-group">
        <h4>{t("accounts.details.title")}</h4>
        <div className="form-grid">
          <label className="field-label">
            {t("accounts.details.alias")}
            <input
              value={alias}
              maxLength={40}
              placeholder={t("accounts.details.aliasPlaceholder")}
              onChange={(event) => setAlias(event.target.value)}
            />
          </label>
          <div className="field-label pin-field">
            {t("accounts.details.pinned")}
            <label className="checkbox-label">
              <input
                type="checkbox"
                checked={pinned}
                onChange={(event) => setPinned(event.target.checked)}
              />
              <Pin size={13} />
              {t("accounts.details.pinnedDetail")}
            </label>
            <label
              className="checkbox-label"
              title={t("accounts.details.mainDetail")}
            >
              <input
                type="checkbox"
                checked={isMain}
                onChange={(event) => setIsMain(event.target.checked)}
              />
              <Crown size={13} />
              {t("accounts.details.main")}
            </label>
          </div>
        </div>
        <div
          className="color-tags"
          role="radiogroup"
          aria-label={t("accounts.details.color")}
        >
          <span>{t("accounts.details.color")}</span>
          <button
            type="button"
            role="radio"
            aria-checked={color === null}
            aria-label={t("accounts.details.noColor")}
            className={`color-tag none ${color === null ? "active" : ""}`}
            onClick={() => setColor(null)}
          />
          {ACCOUNT_COLORS.map((item) => (
            <button
              key={item}
              type="button"
              role="radio"
              aria-checked={color === item}
              aria-label={item}
              className={`color-tag ${color === item ? "active" : ""}`}
              style={{ background: item }}
              onClick={() => setColor(item)}
            />
          ))}
        </div>
        <label className="field-label">
          {t("accounts.details.note")}
          <textarea
            className="note-input"
            rows={2}
            maxLength={1000}
            placeholder={t("accounts.details.notePlaceholder")}
            value={note}
            onChange={(event) => setNote(event.target.value)}
          />
        </label>
        <FieldsEditor fields={fields} onChange={setFields} />
        <ClickPointEditor point={afkPoint} onChange={setAfkPoint} />
      </section>

      <section className="edit-group">
        <h4>{t("accounts.edit.credentials")}</h4>
        <p>{t("accounts.edit.credentialsDetail")}</p>
        <CredentialField
          icon={<Lock size={14} />}
          label={t("accounts.edit.password")}
          stored={account.hasPassword}
          editing={password !== null}
          onEdit={(editing) => {
            setPassword(editing ? "" : null);
            setShowPassword(false);
          }}
        >
          <div className="secret-input">
            <input
              autoFocus
              type={showPassword ? "text" : "password"}
              autoComplete="new-password"
              aria-label={t("accounts.edit.newPassword")}
              placeholder={t("accounts.edit.newPassword")}
              value={password ?? ""}
              onChange={(event) => setPassword(event.target.value)}
            />
            <Button
              type="button"
              variant="ghost"
              size="icon"
              aria-label={t(
                showPassword
                  ? "accounts.edit.hidePassword"
                  : "accounts.edit.showPassword",
              )}
              onClick={() => setShowPassword(!showPassword)}
            >
              {showPassword ? <EyeOff /> : <Eye />}
            </Button>
          </div>
        </CredentialField>
        <CredentialField
          icon={<KeyRound size={14} />}
          label={t("accounts.actions.cookie.label")}
          stored={account.hasCookie}
          detail={
            account.hasCookie && account.cookieValid !== null
              ? t(
                  account.cookieValid
                    ? "accounts.actions.cookie.valid"
                    : "accounts.actions.cookie.invalid",
                )
              : undefined
          }
          editing={cookie !== null}
          onEdit={(editing) => setCookie(editing ? "" : null)}
        >
          <textarea
            autoFocus
            rows={4}
            spellCheck={false}
            aria-label={t("accounts.edit.newCookie")}
            placeholder={t("accounts.edit.cookiePlaceholder")}
            value={cookie ?? ""}
            onChange={(event) => setCookie(event.target.value)}
          />
          <small className="field-hint">{t("accounts.edit.cookieHint")}</small>
        </CredentialField>
        <CredentialField
          icon={<Smartphone size={14} />}
          label={t("accounts.totp.label")}
          stored={account.hasTotp}
          editing={totp !== null}
          onEdit={(editing) => setTotp(editing ? "" : null)}
        >
          <input
            autoFocus
            spellCheck={false}
            autoComplete="off"
            aria-label={t("accounts.totp.key")}
            placeholder={t("accounts.totp.placeholder")}
            value={totp ?? ""}
            onChange={(event) => setTotp(event.target.value)}
          />
          <small className="field-hint">
            {t(
              account.hasTotp
                ? "accounts.totp.replaceHint"
                : "accounts.totp.hint",
            )}
          </small>
        </CredentialField>
      </section>

      {error && (
        <div className="error-banner" role="alert">
          {error}
        </div>
      )}
      <div className="dialog-footer">
        <Button variant="outline" type="button" onClick={onClose}>
          {t("common.cancel")}
        </Button>
        <Button disabled={busy || !username.trim() || !changed}>
          {t(busy ? "common.saving" : "common.saveChanges")}
        </Button>
      </div>
    </form>
  );
}

// A stored secret: shows whether it exists and only opens an input on request.
function CredentialField({
  icon,
  label,
  stored,
  detail,
  editing,
  onEdit,
  children,
}: {
  icon: ReactNode;
  label: string;
  stored: boolean;
  detail?: string;
  editing: boolean;
  onEdit: (editing: boolean) => void;
  children: ReactNode;
}) {
  const t = useT();
  const action = editing
    ? stored
      ? "accounts.edit.keep"
      : "common.cancel"
    : stored
      ? "accounts.edit.replace"
      : "accounts.edit.add";
  return (
    <div className={`credential-field ${editing ? "editing" : ""}`}>
      <div className="credential-row">
        <span className="credential-label">
          {icon}
          {label}
        </span>
        <span className={`credential-state ${stored ? "stored" : ""}`}>
          {stored ? <Check size={12} /> : null}
          {t(stored ? "accounts.edit.saved" : "accounts.edit.notSaved")}
          {detail && <span>· {detail}</span>}
        </span>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          onClick={() => onEdit(!editing)}
        >
          {t(action)}
        </Button>
      </div>
      {editing && <div className="credential-editor">{children}</div>}
    </div>
  );
}
