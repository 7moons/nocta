import { useEffect, useState, type FormEvent } from "react";
import { LogOut, MonitorSmartphone, ShieldCheck, UserPen } from "lucide-react";
import type { PrivacySetting, QuickLoginDevice } from "@/generated/contracts";
import { Button } from "@/components/ui/button";
import { useT, type MessageKey, type Translate } from "@/i18n";
import { api } from "@/lib/ipc";
import { errorMessage } from "@/lib/utils";

const TOOLS = [
  { id: "sessions", icon: LogOut },
  { id: "displayName", icon: UserPen },
  { id: "quickLogin", icon: MonitorSmartphone },
  { id: "privacy", icon: ShieldCheck },
] as const;
type Tool = (typeof TOOLS)[number]["id"];

/** "whoCanJoinMeInExperiences" → "Who can join me in experiences". */
export function humanize(name: string) {
  const words = name.replace(/([a-z0-9])([A-Z])/g, "$1 $2").toLowerCase();
  return words.charAt(0).toUpperCase() + words.slice(1);
}

/** A Roblox privacy name in the user's language when Nocta knows it, else spelled out. */
export function privacyLabel(
  t: Translate,
  kind: "key" | "value",
  name: string,
) {
  const key = `privacy.${kind}.${name}` as MessageKey;
  const text = t(key);
  return text === key ? humanize(name) : text;
}

/** The check Roblox wants before `option` can be chosen, if any. */
export function restriction(setting: PrivacySetting, option: string) {
  return setting.restricted.find((item) => item.value === option)?.requirement;
}

/** Changes made to the account on Roblox itself, with the account's own session. */
export function RobloxAccountTools({ accountId }: { accountId: number }) {
  const t = useT();
  const [tool, setTool] = useState<Tool | null>(null);
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<{ ok: boolean; text: string } | null>(
    null,
  );
  const run = async (work: () => Promise<unknown>, done: string) => {
    setBusy(true);
    setResult(null);
    try {
      await work();
      setResult({ ok: true, text: done });
    } catch (failure) {
      setResult({ ok: false, text: errorMessage(failure) });
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="roblox-tools">
      <div
        className="roblox-tools-tabs"
        role="tablist"
        aria-label={t("accounts.roblox.title")}
      >
        {TOOLS.map(({ id, icon: Icon }) => (
          <button
            key={id}
            type="button"
            role="tab"
            aria-selected={tool === id}
            className={tool === id ? "active" : ""}
            onClick={() => {
              setTool(tool === id ? null : id);
              setResult(null);
            }}
          >
            <Icon size={14} />
            {t(`accounts.roblox.${id}.title`)}
          </button>
        ))}
      </div>
      {tool && (
        <div className="roblox-tool" role="tabpanel">
          <p className="field-hint">{t(`accounts.roblox.${tool}.detail`)}</p>
          {tool === "sessions" && (
            <Button
              variant="destructive"
              disabled={busy}
              onClick={() => {
                if (window.confirm(t("accounts.roblox.sessions.confirm")))
                  void run(
                    () => api.signOutOtherSessions(accountId),
                    t("accounts.roblox.sessions.done"),
                  );
              }}
            >
              <LogOut />
              {t("accounts.roblox.sessions.action")}
            </Button>
          )}
          {tool === "displayName" && (
            <DisplayNameForm
              busy={busy}
              onSave={(name) =>
                run(
                  () => api.setDisplayName(accountId, name),
                  t("accounts.roblox.displayName.done", { name }),
                )
              }
            />
          )}
          {tool === "quickLogin" && (
            <QuickLoginForm
              accountId={accountId}
              busy={busy}
              onApprove={(code) =>
                run(
                  () => api.quickLoginApprove(accountId, code),
                  t("accounts.roblox.quickLogin.done"),
                )
              }
            />
          )}
          {tool === "privacy" && <PrivacyForm accountId={accountId} />}
          {result && (
            <p
              className={result.ok ? "form-success" : "form-error"}
              role="status"
            >
              {result.text}
            </p>
          )}
        </div>
      )}
    </div>
  );
}

function DisplayNameForm({
  busy,
  onSave,
}: {
  busy: boolean;
  onSave: (name: string) => void;
}) {
  const t = useT();
  const [name, setName] = useState("");
  const submit = (event: FormEvent) => {
    event.preventDefault();
    onSave(name.trim());
  };
  return (
    <form className="roblox-tool-row" onSubmit={submit}>
      <input
        aria-label={t("accounts.roblox.displayName.label")}
        placeholder={t("accounts.roblox.displayName.label")}
        minLength={3}
        maxLength={20}
        value={name}
        onChange={(event) => setName(event.target.value)}
      />
      <Button type="submit" disabled={busy || name.trim().length < 3}>
        {t("accounts.roblox.displayName.action")}
      </Button>
    </form>
  );
}

function QuickLoginForm({
  accountId,
  busy,
  onApprove,
}: {
  accountId: number;
  busy: boolean;
  onApprove: (code: string) => void;
}) {
  const t = useT();
  const [code, setCode] = useState("");
  const [device, setDevice] = useState<QuickLoginDevice | null>(null);
  const [error, setError] = useState("");
  const [checking, setChecking] = useState(false);
  const check = async (event: FormEvent) => {
    event.preventDefault();
    setChecking(true);
    setError("");
    setDevice(null);
    try {
      setDevice(await api.quickLoginCheck(accountId, code));
    } catch (failure) {
      setError(errorMessage(failure));
    } finally {
      setChecking(false);
    }
  };
  return (
    <>
      <form className="roblox-tool-row" onSubmit={check}>
        <input
          aria-label={t("accounts.roblox.quickLogin.label")}
          placeholder="ABC123"
          maxLength={7}
          autoComplete="off"
          value={code}
          onChange={(event) => {
            setCode(event.target.value.toUpperCase());
            setDevice(null);
          }}
        />
        <Button type="submit" disabled={checking || code.trim().length < 6}>
          {t("accounts.roblox.quickLogin.check")}
        </Button>
      </form>
      {error && <p className="form-error">{error}</p>}
      {device && (
        <div className="quick-login-device">
          <p>
            {t("accounts.roblox.quickLogin.device", {
              device: device.device || "?",
              location: device.location || "?",
            })}
          </p>
          <Button disabled={busy} onClick={() => onApprove(code)}>
            {t("accounts.roblox.quickLogin.approve")}
          </Button>
        </div>
      )}
    </>
  );
}

function PrivacyForm({ accountId }: { accountId: number }) {
  const t = useT();
  const [settings, setSettings] = useState<PrivacySetting[] | null>(null);
  const [error, setError] = useState("");
  const [saved, setSaved] = useState("");
  useEffect(() => {
    let current = true;
    api
      .privacySettings(accountId)
      .then((loaded) => current && setSettings(loaded))
      .catch((failure) => current && setError(errorMessage(failure)));
    return () => {
      current = false;
    };
  }, [accountId]);
  const change = async (setting: PrivacySetting, value: string) => {
    const key = setting.key;
    setError("");
    setSaved("");
    // Leaving an option behind Roblox's own check cannot be undone from Nocta.
    if (
      restriction(setting, setting.value) &&
      !window.confirm(t("accounts.roblox.privacy.oneWay"))
    )
      return;
    try {
      const changed = await api.setPrivacy([accountId], key, value);
      if (changed === 0) throw new Error(t("accounts.roblox.privacy.failed"));
      setSettings(
        (list) =>
          list?.map((item) => (item.key === key ? { ...item, value } : item)) ??
          null,
      );
      setSaved(t("accounts.roblox.privacy.done"));
    } catch (failure) {
      setError(errorMessage(failure));
    }
  };
  if (error && !settings) return <p className="form-error">{error}</p>;
  if (!settings) return <p className="field-hint">{t("common.loading")}</p>;
  return (
    <div className="privacy-settings">
      {settings.map((setting) => (
        <label key={setting.key} className="field-label">
          {privacyLabel(t, "key", setting.key)}
          <select
            aria-label={privacyLabel(t, "key", setting.key)}
            value={setting.value}
            onChange={(event) => void change(setting, event.target.value)}
          >
            {setting.options.map((option) => (
              <option
                key={option}
                value={option}
                disabled={
                  option !== setting.value && !!restriction(setting, option)
                }
              >
                {privacyLabel(t, "value", option)}
                {restriction(setting, option) &&
                  ` (${t("accounts.roblox.privacy.locked")})`}
              </option>
            ))}
          </select>
        </label>
      ))}
      {error && <p className="form-error">{error}</p>}
      {saved && (
        <p className="form-success" role="status">
          {saved}
        </p>
      )}
    </div>
  );
}
