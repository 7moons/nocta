import { useEffect, useState } from "react";
import type {
  Account,
  BefriendMode,
  PrivacySetting,
} from "@/generated/contracts";
import { Dialog } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { useT } from "@/i18n";
import { api } from "@/lib/ipc";
import { usePrivacy } from "@/lib/privacy";
import { errorMessage } from "@/lib/utils";
import { privacyLabel, restriction } from "./RobloxAccountTools";

/** Most friendships one run makes (the core refuses more). */
export const MAX_PAIRS = 300;

export type BulkTool = "privacy" | "befriend";

/** The first option that needs no check of Roblox's own. */
const firstAllowed = (setting: PrivacySetting | undefined) =>
  setting?.options.find((option) => !restriction(setting, option)) ?? "";

/** One privacy setting for every selected account; the options come from the first one. */
export function PrivacyBulkDialog({
  accounts,
  onClose,
  onApply,
}: {
  accounts: Account[];
  onClose: () => void;
  onApply: (key: string, value: string) => void;
}) {
  const t = useT();
  const [settings, setSettings] = useState<PrivacySetting[] | null>(null);
  const [error, setError] = useState("");
  const [key, setKey] = useState("");
  const [value, setValue] = useState("");
  const first = accounts[0]?.id;
  useEffect(() => {
    if (first === undefined) return;
    let current = true;
    api
      .privacySettings(first)
      .then((loaded) => {
        if (!current) return;
        setSettings(loaded);
        setKey(loaded[0]?.key ?? "");
        setValue(firstAllowed(loaded[0]));
      })
      .catch((failure) => current && setError(errorMessage(failure)));
    return () => {
      current = false;
    };
  }, [first]);
  const setting = settings?.find((item) => item.key === key);
  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
      title={t("accounts.bulk.privacy.title")}
      description={t("accounts.bulk.privacy.detail", {
        count: accounts.length,
      })}
    >
      <div className="dialog-body bulk-roblox">
        {error && <p className="form-error">{error}</p>}
        {!settings && !error && (
          <p className="field-hint">{t("common.loading")}</p>
        )}
        {settings && (
          <>
            <label className="field-label">
              {t("accounts.bulk.privacy.setting")}
              <select
                aria-label={t("accounts.bulk.privacy.setting")}
                value={key}
                onChange={(event) => {
                  setKey(event.target.value);
                  setValue(
                    firstAllowed(
                      settings.find((item) => item.key === event.target.value),
                    ),
                  );
                }}
              >
                {settings.map((item) => (
                  <option key={item.key} value={item.key}>
                    {privacyLabel(t, "key", item.key)}
                  </option>
                ))}
              </select>
            </label>
            <label className="field-label">
              {t("accounts.bulk.privacy.value")}
              <select
                aria-label={t("accounts.bulk.privacy.value")}
                value={value}
                onChange={(event) => setValue(event.target.value)}
              >
                {setting?.options.map((option) => (
                  <option
                    key={option}
                    value={option}
                    disabled={!!restriction(setting, option)}
                  >
                    {privacyLabel(t, "value", option)}
                    {restriction(setting, option) &&
                      ` (${t("accounts.roblox.privacy.locked")})`}
                  </option>
                ))}
              </select>
            </label>
            {setting && setting.restricted.length > 0 && (
              <p className="field-hint">{t("accounts.bulk.privacy.oneWay")}</p>
            )}
          </>
        )}
        <div className="dialog-footer">
          <Button variant="outline" onClick={onClose}>
            {t("common.cancel")}
          </Button>
          <Button
            disabled={!key || !value}
            onClick={() => {
              onApply(key, value);
              onClose();
            }}
          >
            {t("accounts.bulk.privacy.apply", { count: accounts.length })}
          </Button>
        </div>
      </div>
    </Dialog>
  );
}

/** Friendships among the selected accounts: everyone with everyone, or with one main account. */
export function BefriendDialog({
  accounts,
  onClose,
  onApply,
}: {
  accounts: Account[];
  onClose: () => void;
  onApply: (mode: BefriendMode) => void;
}) {
  const t = useT();
  const privacy = usePrivacy();
  const [kind, setKind] = useState<BefriendMode["kind"]>("everyone");
  const [main, setMain] = useState(
    (accounts.find((account) => account.isMain) ?? accounts[0])?.id ?? 0,
  );
  const n = accounts.length;
  const pairs = kind === "everyone" ? (n * (n - 1)) / 2 : n - 1;
  const tooMany = pairs > MAX_PAIRS;
  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
      title={t("accounts.bulk.befriend.title")}
      description={t("accounts.bulk.befriend.detail")}
    >
      <div className="dialog-body bulk-roblox">
        <fieldset className="radio-list">
          <label className="checkbox-label">
            <input
              type="radio"
              name="befriend-mode"
              checked={kind === "everyone"}
              onChange={() => setKind("everyone")}
            />
            {t("accounts.bulk.befriend.everyone")}
          </label>
          <label className="checkbox-label">
            <input
              type="radio"
              name="befriend-mode"
              checked={kind === "with_main"}
              onChange={() => setKind("with_main")}
            />
            {t("accounts.bulk.befriend.withMain")}
          </label>
        </fieldset>
        {kind === "with_main" && (
          <label className="field-label">
            {t("accounts.bulk.befriend.main")}
            <select
              aria-label={t("accounts.bulk.befriend.main")}
              value={main}
              onChange={(event) => setMain(Number(event.target.value))}
            >
              {accounts.map((account) => (
                <option key={account.id} value={account.id}>
                  {privacy.name(account)}
                </option>
              ))}
            </select>
          </label>
        )}
        <p className={tooMany ? "form-error" : "field-hint"}>
          {tooMany
            ? t("accounts.bulk.befriend.tooMany", {
                count: pairs,
                max: MAX_PAIRS,
              })
            : t("accounts.bulk.befriend.pairs", { count: pairs })}
        </p>
        <div className="dialog-footer">
          <Button variant="outline" onClick={onClose}>
            {t("common.cancel")}
          </Button>
          <Button
            disabled={tooMany || pairs < 1}
            onClick={() => {
              onApply(
                kind === "everyone"
                  ? { kind: "everyone" }
                  : { kind: "with_main", main },
              );
              onClose();
            }}
          >
            {t("accounts.bulk.befriend.apply")}
          </Button>
        </div>
      </div>
    </Dialog>
  );
}
