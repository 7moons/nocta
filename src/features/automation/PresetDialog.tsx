import { useMemo, useState, type FormEvent } from "react";
import type {
  Account,
  JoinStrategy,
  LaunchPreset,
  PresetTarget,
} from "@/generated/contracts";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { HotkeyInput } from "@/components/HotkeyInput";
import { STRATEGY_KEYS } from "@/features/accounts/launch/target";
import { useT } from "@/i18n";
import { usePrivacy } from "@/lib/privacy";

type Kind = PresetTarget["kind"];
const KINDS: Kind[] = ["place", "pick", "link", "follow"];
const STRATEGIES: JoinStrategy[] = [
  "squad",
  "lowest_ping",
  "hop",
  "fullest",
  "emptiest",
];
/** Accounts listed at once in the picker; search narrows bigger lists. */
const LIST_LIMIT = 300;

/** Creates or edits a launch preset: which accounts, where, and whether AFK. */
export function PresetDialog({
  preset,
  accounts,
  onSave,
  onClose,
}: {
  /** The preset to edit; `undefined` closes the dialog, `null` creates a new one. */
  preset: LaunchPreset | null | undefined;
  accounts: Account[];
  onSave: (preset: LaunchPreset) => void;
  onClose: () => void;
}) {
  const t = useT();
  return (
    <Dialog
      open={preset !== undefined}
      wide
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
      title={t(preset ? "automation.preset.edit" : "automation.preset.new")}
      description={t("automation.preset.detail")}
    >
      {preset !== undefined && (
        <PresetForm
          key={preset?.id ?? "new"}
          preset={preset}
          accounts={accounts}
          onSave={(value) => {
            onSave(value);
            onClose();
          }}
          onCancel={onClose}
        />
      )}
    </Dialog>
  );
}

function PresetForm({
  preset,
  accounts,
  onSave,
  onCancel,
}: {
  preset: LaunchPreset | null;
  accounts: Account[];
  onSave: (preset: LaunchPreset) => void;
  onCancel: () => void;
}) {
  const t = useT();
  const privacy = usePrivacy();
  const target = preset?.target;
  const [name, setName] = useState(preset?.name ?? "");
  const [ids, setIds] = useState(() => new Set(preset?.accountIds ?? []));
  const [kind, setKind] = useState<Kind>(target?.kind ?? "place");
  const [placeId, setPlaceId] = useState(
    target && "placeId" in target ? target.placeId : "",
  );
  const [strategy, setStrategy] = useState<JoinStrategy>(
    target?.kind === "pick" ? target.strategy : "squad",
  );
  const [url, setUrl] = useState(target?.kind === "link" ? target.url : "");
  const [username, setUsername] = useState(
    target?.kind === "follow" ? target.username : "",
  );
  const [afk, setAfk] = useState(preset?.afk ?? false);
  const [hotkey, setHotkey] = useState(preset?.hotkey ?? "");
  const [search, setSearch] = useState("");

  const matching = useMemo(() => {
    const query = search.trim().toLowerCase();
    return accounts.filter(
      (account) =>
        !query ||
        account.username.toLowerCase().includes(query) ||
        account.alias.toLowerCase().includes(query),
    );
  }, [accounts, search]);
  const listed = matching.slice(0, LIST_LIMIT);

  const built: PresetTarget | null =
    kind === "place" && /^\d+$/.test(placeId.trim())
      ? { kind, placeId: placeId.trim() }
      : kind === "pick" && /^\d+$/.test(placeId.trim())
        ? { kind, placeId: placeId.trim(), strategy }
        : kind === "link" && url.trim()
          ? { kind, url: url.trim() }
          : kind === "follow" && username.trim()
            ? { kind, username: username.trim() }
            : null;
  const valid = !!name.trim() && ids.size > 0 && built !== null;

  function submit(event: FormEvent) {
    event.preventDefault();
    if (!valid || !built) return;
    onSave({
      id: preset?.id ?? crypto.randomUUID(),
      name: name.trim(),
      // Keep the accounts' table order.
      accountIds: accounts
        .filter((account) => ids.has(account.id))
        .map((account) => account.id),
      target: built,
      afk,
      hotkey,
    });
  }

  const toggle = (id: number, on: boolean) => {
    const next = new Set(ids);
    if (on) next.add(id);
    else next.delete(id);
    setIds(next);
  };

  return (
    <form className="dialog-body preset-form" onSubmit={submit}>
      <label className="field-label">
        {t("automation.preset.name")}
        <input
          autoFocus
          required
          maxLength={60}
          value={name}
          placeholder={t("automation.preset.namePlaceholder")}
          onChange={(event) => setName(event.target.value)}
        />
      </label>

      <fieldset className="preset-target">
        <legend>{t("automation.target.title")}</legend>
        <div className="segmented" role="radiogroup">
          {KINDS.map((item) => (
            <button
              key={item}
              type="button"
              role="radio"
              aria-checked={kind === item}
              className={kind === item ? "active" : ""}
              onClick={() => setKind(item)}
            >
              {t(`automation.target.${item}`)}
            </button>
          ))}
        </div>
        {(kind === "place" || kind === "pick") && (
          <label className="field-label">
            {t("automation.target.placeId")}
            <input
              inputMode="numeric"
              value={placeId}
              placeholder="1818"
              onChange={(event) => setPlaceId(event.target.value)}
            />
          </label>
        )}
        {kind === "pick" && (
          <label className="field-label">
            {t("automation.target.strategy")}
            <select
              value={strategy}
              onChange={(event) =>
                setStrategy(event.target.value as JoinStrategy)
              }
            >
              {STRATEGIES.map((item) => (
                <option key={item} value={item}>
                  {t(`launch.join.${STRATEGY_KEYS[item]}`)}
                </option>
              ))}
            </select>
          </label>
        )}
        {kind === "link" && (
          <label className="field-label">
            {t("automation.target.link")}
            <input
              value={url}
              placeholder="https://www.roblox.com/share?code=…"
              onChange={(event) => setUrl(event.target.value)}
            />
          </label>
        )}
        {kind === "follow" && (
          <label className="field-label">
            {t("launch.join.followUsername")}
            <input
              value={username}
              maxLength={40}
              placeholder={t("launch.join.followPlaceholder")}
              onChange={(event) => setUsername(event.target.value)}
            />
          </label>
        )}
      </fieldset>

      <fieldset className="preset-accounts">
        <legend>{t("automation.preset.accounts", { count: ids.size })}</legend>
        <div className="table-tools">
          <label className="search-field">
            <input
              aria-label={t("automation.preset.searchAccounts")}
              placeholder={t("automation.preset.searchAccounts")}
              value={search}
              onChange={(event) => setSearch(event.target.value)}
            />
          </label>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={() =>
              setIds(new Set([...ids, ...matching.map((item) => item.id)]))
            }
          >
            {t("automation.preset.selectShown")}
          </Button>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            disabled={!ids.size}
            onClick={() => setIds(new Set())}
          >
            {t("automation.preset.clear")}
          </Button>
        </div>
        <div className="preset-account-list">
          {listed.map((account) => (
            <label key={account.id} className="checkbox-label">
              <input
                type="checkbox"
                checked={ids.has(account.id)}
                onChange={(event) => toggle(account.id, event.target.checked)}
              />
              {privacy.name(account)}
              {privacy.realName(account) && (
                <small>{privacy.realName(account)}</small>
              )}
            </label>
          ))}
          {matching.length > listed.length && (
            <small className="muted">
              {t("automation.preset.more", {
                count: matching.length - listed.length,
              })}
            </small>
          )}
        </div>
      </fieldset>

      <label className="checkbox-label">
        <input
          type="checkbox"
          checked={afk}
          onChange={(event) => setAfk(event.target.checked)}
        />
        {t("automation.preset.afk")}
      </label>

      <label className="field-label preset-hotkey">
        {t("automation.preset.hotkey")}
        <HotkeyInput
          label={t("automation.preset.hotkey")}
          value={hotkey}
          onChange={setHotkey}
        />
      </label>

      <div className="dialog-footer">
        <Button type="button" variant="outline" onClick={onCancel}>
          {t("common.cancel")}
        </Button>
        <Button type="submit" disabled={!valid}>
          {t("common.save")}
        </Button>
      </div>
    </form>
  );
}
